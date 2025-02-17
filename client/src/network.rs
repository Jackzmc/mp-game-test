use std::collections::VecDeque;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvError, Sender, TryRecvError};
use std::thread;
use log::{debug, error, trace, warn};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::packet::{Packet, PACKET_HEADER_SIZE};
use mp_game_test_common::{PacketSerialize, PACKET_PROTOCOL_VERSION};
use mp_game_test_common::events_server::ServerEvent;

pub struct NetClient {
    event_queue: EventQueue,
    socket: UdpSocket,
    // rx: Sender<ClientEvent>,
    transmit_out_tx: Option<Sender<Packet>>,
    recv_end_signal: Option<Sender<()>>,
    send_thread: thread::JoinHandle<()>,
    recv_thread: thread::JoinHandle<()>,

    packet_counter: (Arc<AtomicU16>, Arc<AtomicU16>), // (tx, rx)
}



type EventQueue = Arc<Mutex<VecDeque<ServerEvent>>>;

impl NetClient  {
    pub fn new(addr: SocketAddr) -> Self  {
        // socket.set_nonblocking(false);
        let (tx, rx) = channel::<Packet>();
        let event_queue = Arc::new(Mutex::new(VecDeque::new()));
        let mut socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.connect(addr).unwrap();
        let end_signal = channel::<()>();
        // socket.set_nonblocking(false).unwrap();
        let packet_counter = (Arc::new(AtomicU16::new(0)), Arc::new(AtomicU16::new(0)));
        debug!("connect to {:?} from {:?}", addr, socket.local_addr().unwrap());
        let recv_thread = {
            let event_queue = event_queue.clone();
            let socket = socket.try_clone().unwrap();
            let counter = packet_counter.0.clone();
            thread::spawn(move || network_recv_thread(end_signal.1, socket, event_queue.clone(), counter))
        };
        let send_thread = {
            let socket = socket.try_clone().unwrap();
            let counter = packet_counter.1.clone();
            thread::spawn(move || network_send_thread(socket, rx, counter))
        };

        NetClient {
            transmit_out_tx: Some(tx),
            recv_end_signal: Some(end_signal.0),
            recv_thread,
            send_thread,
            event_queue,
            socket,
            packet_counter,
        }
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

pub fn network_recv_thread(end_signal: Receiver<()>, socket: UdpSocket, mut event_queue: EventQueue, counter: Arc<AtomicU16>) {
    let mut buf = Vec::with_capacity(2048);
    while end_signal.try_recv() != Err(TryRecvError::Disconnected) { // Check if we are good
        // Check if we received any data, and add it to packet queue
        buf.resize(2048, 0);
        match socket.recv(&mut buf) {
            Ok(n) => {
                if n > 0 {
                    counter.fetch_add(1, Ordering::Relaxed);
                    trace!("IN n={} {:?}", n, &buf[0..n]);
                    let mut lock = event_queue.lock().unwrap();
                    let pk = Packet::from(buf.as_slice());
                    match ServerEvent::from_packet(&pk) {
                        Ok(ev) => {
                            trace!("received event, pushing to queue");
                            lock.push_back(ev);
                        }
                        Err(err) => {
                            warn!("bad packet: {:?}", err);
                        }
                    };
                }
            }
            Err(e) => {
                error!("[net] recv error: {}", e)
            }
        }
    }
    debug!("recv thread: EXITED");
}
pub fn network_send_thread(socket: UdpSocket, mut transmit_recv: Receiver<Packet>,counter: Arc<AtomicU16>) {
    loop {
        // Check if there's any data we need to send out
        if let Ok(pk) = transmit_recv.recv() {
            trace!("OUT pk_len={} py_len={} {}", pk.buf_len(), pk.payload_len(), pk.as_hex_str());
            socket.send(pk.as_slice()).unwrap();
            counter.fetch_add(1, Ordering::Relaxed);
        } else {
            debug!("send_thread: channel closed, exiting");
            break;
        }
    }
    debug!("send thread: EXITED");
}