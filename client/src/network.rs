use std::collections::VecDeque;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvError, Sender, TryRecvError};
use std::thread;
use std::time::Duration;
use log::{debug, error, trace, warn};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::packet::{Packet};
use mp_game_test_common::{NetDirection, NetStat, PacketSerialize, PACKET_PROTOCOL_VERSION};
use mp_game_test_common::events_server::ServerEvent;

pub struct NetClient {
    event_queue: EventQueue,
    socket: UdpSocket,
    // rx: Sender<ClientEvent>,
    transmit_out_tx: Option<Sender<Packet>>,
    recv_end_signal: Option<Sender<()>>,
    send_thread: thread::JoinHandle<()>,
    recv_thread: thread::JoinHandle<()>,

    last_error: Arc<Mutex<Option<String>>>,

    packet_counter: (Arc<AtomicU16>, Arc<AtomicU16>), // (tx, rx)

    net_stat: NetStat
}



type EventQueue = Arc<Mutex<VecDeque<ServerEvent>>>;

impl NetClient  {
    pub fn new(addr: SocketAddr) -> Self  {
        let last_error = Arc::new(Mutex::new(None));
        // socket.set_nonblocking(false);
        let (tx, rx) = channel::<Packet>();
        let event_queue = Arc::new(Mutex::new(VecDeque::new()));
        let mut socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        let net_stat = NetStat::new();
        socket.connect(addr).unwrap();
        let end_signal = channel::<()>();
        // socket.set_nonblocking(false).unwrap();
        let packet_counter = (Arc::new(AtomicU16::new(0)), Arc::new(AtomicU16::new(0)));
        debug!("connect to {:?} from {:?}", addr, socket.local_addr().unwrap());
        let recv_thread = {
            let event_queue = event_queue.clone();
            let socket = socket.try_clone().unwrap();
            let net_stat = net_stat.clone();
            let last_error = last_error.clone();

            thread::spawn(move || network_recv_thread(end_signal.1, socket, event_queue.clone(), net_stat, last_error))
        };
        let send_thread = {
            let socket = socket.try_clone().unwrap();
            let net_stat = net_stat.clone();
            thread::spawn(move || network_send_thread(socket, rx, net_stat))
        };

        NetClient {
            transmit_out_tx: Some(tx),
            recv_end_signal: Some(end_signal.0),
            recv_thread,
            send_thread,
            event_queue,
            socket,
            packet_counter,
            last_error,
            net_stat
        }
    }

    pub fn stat(&self) -> &NetStat {
        &self.net_stat
    }

    pub fn stats(&self) -> (u16, u16) {
        let val = (
            self.packet_counter.0.load(Ordering::Relaxed),
            self.packet_counter.1.load(Ordering::Relaxed),
        );
        self.packet_counter.0.store(0, Ordering::Relaxed);
        self.packet_counter.1.store(0, Ordering::Relaxed);
        val
    }

    pub fn last_err(&self) -> Option<String> {
        let lock = self.last_error.lock().unwrap();
        lock.as_ref().map(|s| s.clone())
    }

    pub fn clear_last_err(&mut self) {
        let mut lock = self.last_error.lock().unwrap();
        *lock = None;
    }

    pub fn end(mut self) {
        // Drop senders, which the threads will then end after noticing its dropped
        self.transmit_out_tx = None;
        self.recv_end_signal = None;

        self.send_thread.join().unwrap();
        self.recv_thread.join().unwrap();
    }

    pub fn send(&self, packet: Packet) -> Result<(), String> {
        self.transmit_out_tx.as_ref().expect("shutting down").send(packet).map_err(|e| e.to_string())
    }

    pub fn process_queue_len(&self) -> usize {
        self.event_queue.lock().unwrap().len()
    }


    /// Pops the next event off, if any
    pub fn next_event(&mut self) -> Option<ServerEvent> {
        let mut lock = self.event_queue.lock().unwrap();
        lock.pop_front()
    }
}

pub fn network_recv_thread(end_signal: Receiver<()>, socket: UdpSocket, mut event_queue: EventQueue, mut net_stat: NetStat, last_error: Arc<Mutex<Option<String>>>) {
    let mut buf = Vec::with_capacity(2048);
    let mut current_auth_id = 0;
    // We need a time out so we can check end_signal
    // Otherwise, we cannot shut down (so then we can't stop the program), until we received some data
    socket.set_read_timeout(Some(Duration::from_secs(1))).expect("set_read_timeout failed");
    while end_signal.try_recv() != Err(TryRecvError::Disconnected) { // Check if we are good
        // Check if we received any data, and add it to packet queue
        buf.resize(2048, 0);
        match socket.recv(&mut buf) {
            Ok(n) => {
                buf.truncate(n);
                net_stat.mark_activity(NetDirection::In);
                if n > 0 {
                    let pk = match Packet::try_decompress_from_slice(buf.as_slice()) {
                        Ok(pk) => pk,
                        Err(e) => {
                            warn!("[net] dropping bad packet: {}", e);
                            continue;
                        }
                    };
                    trace!("[net] IN n={} {}", n, pk.as_hex_str());
                    net_stat.inc_pk_count(NetDirection::In);

                    // ACK any seq numbers
                    let seq_num = pk.sequence_number();
                    if seq_num > 0 {
                        let event = ClientEvent::Ack {
                            seq_number: seq_num,
                        };
                        trace!("[net] sending ACK seq#{}", seq_num);
                        let out_pk = event.to_packet_builder()
                            .with_auth_id(current_auth_id)
                            .finalize();
                        // Send out a burst of 3 - hopefully at least one gets sent
                        for _ in 0..3 {
                            socket.send(out_pk.as_slice()).ok();
                            // Add a delay just to ensure they don't all get caught at once
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }
                    }

                    match ServerEvent::from_packet(&pk) {
                        Ok(ev) => {
                            // A little hacky, but we need the auth id for ACK
                            if let ServerEvent::Login {auth_id, ..} = ev {
                                trace!("[net] new auth id = {}", auth_id);
                                current_auth_id = auth_id;
                            }

                            let mut lock = event_queue.lock().unwrap();
                            lock.push_back(ev);
                        }
                        Err(err) => {
                            warn!("[net] bad packet: {:?}", err);
                        }
                    };
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    error!("[net] recv error: {}", e);
                    let mut lock = last_error.lock().unwrap();
                    *lock = Some(e.to_string());
                }

            }
        }
    }
    debug!("[net] recv thread: EXITED");
}
pub fn network_send_thread(socket: UdpSocket, mut transmit_recv: Receiver<Packet>, mut net_stat: NetStat) {
    loop {
        // Check if there's any data we need to send out
        // Unlike recv_thread, when we quit, we send a disconnect packet, so this shouldn't block
        if let Ok(pk) = transmit_recv.recv() {
            let bytes = pk.compress().unwrap();
            trace!("[net] OUT len=({}uncomp,{}comp) py_len={} {}", pk.buf_len(), bytes.len(), pk.payload_len(), pk.as_hex_str());
            socket.send(&bytes).unwrap();
            net_stat.inc_pk_count(NetDirection::Out);
            net_stat.mark_activity(NetDirection::Out);
        } else {
            debug!("[net] send_thread: channel closed, exiting");
            break;
        }
    }
    debug!("[net] send thread: EXITED");
}