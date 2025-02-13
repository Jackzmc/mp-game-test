use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use log::{debug, trace, warn};
use mp_game_test_common::def::ClientEvent;
use mp_game_test_common::packet::{Packet, PACKET_HEADER_SIZE};
use mp_game_test_common::PacketSerialize;

pub struct NetClient {
    event_queue: EventQueue,
    // thread_tx: Sender<ClientEvent>,
    transmit_out_tx: Sender<Packet>,
    thread: thread::JoinHandle<()>
}

type EventQueue = Arc<Mutex<Vec<ClientEvent>>>;

impl NetClient  {
    pub fn new(addr: SocketAddr) -> Self  {
        // socket.set_nonblocking(false);
        let (tx, rx) = channel::<Packet>();
        let event_queue = Arc::new(Mutex::new(vec![]));
        let thread = {
            let packet_queue = event_queue.clone();
             thread::spawn(move || network_thread(addr, rx, packet_queue.clone()))
        };
        NetClient {
            transmit_out_tx: tx,
            thread,
            event_queue
        }
    }

    pub fn send(&self, event: &ClientEvent) -> Result<(), String> {
        let pk = event.to_packet();
        trace!("sending {} bytes", pk.payload_len());
        self.transmit_out_tx.send(pk).map_err(|e| e.to_string())
    }

    pub fn start(&self) -> Result<(), String> {
        let event = ClientEvent::ClientConnect {
            name: "Test Client".to_string()
        };
        self.send(&event).map(|_| ())
    }

    /// Pops the next event off, if any
    pub fn next_event(&mut self) -> Option<ClientEvent> {
        let mut lock = self.event_queue.lock().unwrap();
        lock.pop()
    }
}

pub fn network_thread(addr: SocketAddr, mut transmit_recv: Receiver<Packet>, mut event_queue: EventQueue) {
    let mut socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    socket.connect(addr).unwrap();
    debug!("connect to {:?} from {:?}", addr, socket.local_addr());

    let mut buf = [0; 1024];

    loop {
        // Check if there's any data we need to send out
        if let Ok(pk) = transmit_recv.recv() {
            let slice = pk.buf().slice(0, pk.payload_len() as usize + PACKET_HEADER_SIZE);
            trace!("OUT pk_len={} py_len={} {:?}", pk.buf_len(), pk.payload_len(), slice);
            socket.send(pk.as_slice()).unwrap();
        }

        // Check if we received any data, and add it to packet queue
        if let Ok(n) = socket.recv(&mut buf) {
            if n > 0 {
                trace!("IN n={} {:?}", n, &buf[0..n]);
                let mut lock = event_queue.lock().unwrap();
                let pk = Packet::from(buf.as_slice());
                match ClientEvent::from_packet(pk) {
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

        // std::thread::sleep(std::time::Duration::from_millis(30));
    }
}