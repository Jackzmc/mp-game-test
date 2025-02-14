use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
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
    transmit_out_tx: Sender<Packet>,
    send_thread: thread::JoinHandle<()>,
    recv_thread: thread::JoinHandle<()>
}

type EventQueue = Arc<Mutex<Vec<ServerEvent>>>;

impl NetClient  {
    pub fn new(addr: SocketAddr) -> Self  {
        // socket.set_nonblocking(false);
        let (tx, rx) = channel::<Packet>();
        let event_queue = Arc::new(Mutex::new(vec![]));
        let mut socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.connect(addr).unwrap();
        socket.set_nonblocking(false).unwrap();
        debug!("connect to {:?} from {:?}", addr, socket.local_addr().unwrap());
        let recv_thread = {
            let event_queue = event_queue.clone();
            let socket = socket.try_clone().unwrap();
            thread::spawn(move || network_recv_thread(socket, event_queue.clone()))
        };
        let send_thread = {
            let socket = socket.try_clone().unwrap();
            thread::spawn(move || network_send_thread(socket, rx))
        };

        NetClient {
            transmit_out_tx: tx,
            recv_thread,
            send_thread,
            event_queue,
            socket
        }
    }

    pub fn send(&self, packet: Packet) -> Result<(), String> {
        self.transmit_out_tx.send(packet).map_err(|e| e.to_string())
    }

    pub fn event_queue_len(&self) -> usize {
        self.event_queue.lock().unwrap().len()
    }


    /// Pops the next event off, if any
    pub fn next_event(&mut self) -> Option<ServerEvent> {
        let mut lock = self.event_queue.lock().unwrap();
        lock.pop()
    }
}

pub fn network_recv_thread(socket: UdpSocket, mut event_queue: EventQueue) {
    let mut buf = Vec::with_capacity(2048);
    loop {
        // Check if we received any data, and add it to packet queue
        buf.resize(2048, 0);
        match socket.recv(&mut buf) {
            Ok(n) => {
                if n > 0 {
                    trace!("IN n={} {:?}", n, &buf[0..n]);
                    let mut lock = event_queue.lock().unwrap();
                    let pk = Packet::from(buf.as_slice());
                    match ServerEvent::from_packet(pk) {
                        Ok(ev) => {
                            trace!("received event, pushing to queue");
                            lock.push(ev);
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
}
pub fn network_send_thread(socket: UdpSocket, mut transmit_recv: Receiver<Packet>) {
    loop {
        // Check if there's any data we need to send out
        match transmit_recv.recv() {
            Ok(pk) => {
                trace!("OUT pk_len={} py_len={} {}", pk.buf_len(), pk.payload_len(), pk.as_hex_str());
                socket.send(pk.as_slice()).unwrap();
            }
            Err(err) => {
                error!("[net] recv error: {}", err)
            }
        }
    }
}