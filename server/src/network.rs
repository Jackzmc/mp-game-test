use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvError, Sender};
use std::thread;
use std::time::Instant;
use log::{debug, error, info, trace, warn};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::packet::{Packet, PACKET_HEADER_SIZE};
use mp_game_test_common::{PacketSerialize, PACKET_PROTOCOL_VERSION};
use mp_game_test_common::events_server::ServerEvent;

pub struct NetServer {
    event_queue: EventQueue,
    socket: UdpSocket,
    // rx: Sender<ClientEvent>,
    transmit_out_tx: Sender<OutPacket>,
    send_thread: thread::JoinHandle<()>,
    recv_thread: thread::JoinHandle<()>,

    packet_counter: (Arc<AtomicU16>, Arc<AtomicU16>), // (tx, rx),

    reliable_queue: ReliableQueue,
    seq_number: u16,
}

#[derive(Clone)]
struct ReliableEntry {
    pub seq_id: u16,
    pub packet: Packet,
    pub sent_time: Instant
}

pub enum OutPacket {
    Single(Packet, SocketAddr),
    Multiple(Packet, Vec<SocketAddr>),
}

type EventQueue = Arc<Mutex<VecDeque<(Packet, SocketAddr)>>>;
type ReliableQueue = Arc<Mutex<HashMap<SocketAddr, VecDeque<ReliableEntry>>>>;
impl NetServer {
    pub fn new() -> Self  {
        // socket.set_nonblocking(false);
        let mut sock = UdpSocket::bind("0.0.0.0:3566").expect("Failed to bind UDP socket");
        let (tx, rx) = channel::<OutPacket>();
        let event_queue = Arc::new(Mutex::new(VecDeque::new()));
        let packet_counter = (Arc::new(AtomicU16::new(0)), Arc::new(AtomicU16::new(0)));
        let reliable_queue = Arc::new(Mutex::new(HashMap::new()));

        let send_thread = {
            let socket = sock.try_clone().unwrap();
            let counter = packet_counter.0.clone();
            thread::spawn(move || network_send_thread(socket, rx, counter))
        };
        let recv_thread = {
            let event_queue = event_queue.clone();
            let socket = sock.try_clone().unwrap();
            let counter = packet_counter.1.clone();
            thread::spawn(move || network_recv_thread(socket, event_queue.clone(), reliable_queue.clone(), counter))
        };

        info!("server listening at UDP {:?}", sock.local_addr().unwrap());

        NetServer {
            transmit_out_tx: tx,
            recv_thread,
            send_thread,
            event_queue,
            socket: sock,
            packet_counter: packet_counter,
            reliable_queue,
            seq_number: 0
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

    pub fn add_reliable_packet(&mut self, addr: SocketAddr, event: ServerEvent) -> ReliableEntry {
        let seq = self.seq_number;
        let packet = event.to_packet_builder()
            .with_sequence_number(seq)
            .finalize();
        let entry = ReliableEntry {
            seq_id: seq,
            packet: packet,
            sent_time: Instant::now()
        };
        let mut lock = self.reliable_queue.lock().unwrap();
        let queue = lock.entry(addr)
            .or_insert(VecDeque::new());
        queue.push_back(entry.clone());
        self.seq_number += 1;
        entry
    }

    pub fn send(&self, packet: Packet, addr: SocketAddr) -> Result<(), String> {
        self.transmit_out_tx.send(OutPacket::Single(packet, addr)).map_err(|e| e.to_string())
    }

    pub fn send_multiple(&self, packet: Packet, addr_list: Vec<SocketAddr>) -> Result<(), String> {
        self.transmit_out_tx.send(OutPacket::Multiple(packet, addr_list)).map_err(|e| e.to_string())
    }

    pub fn event_queue_len(&self) -> usize {
        self.event_queue.lock().unwrap().len()
    }


    /// Pops the next event off, if any
    pub fn next_event(&mut self) -> Option<(Packet, ClientEvent, SocketAddr)> {
        let mut lock = self.event_queue.lock().unwrap();
        lock.pop_front().and_then(|(pk, addr)| {
            match ClientEvent::from_packet(&pk) {
                Ok(ev) => {
                    trace!("received event, pushing to queue");
                    Some((pk, ev, addr))
                }
                Err(err) => {
                    warn!("bad packet: {:?}", err);
                    None
                }
            }
        })
    }
}

pub fn network_recv_thread(socket: UdpSocket, mut event_queue: EventQueue, reliable_queue: ReliableQueue, counter: Arc<AtomicU16>) {
    let mut buf = Vec::with_capacity(2048);
    loop {
        // Check if we received any data, and add it to packet queue
        buf.resize(2048, 0);
        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => {
                // Check ACK state
                {
                    let lock = reliable_queue.lock().unwrap();
                    if let Some(queue) = lock.get(&addr) {
                        if let Some(item) = queue.back() {

                        }
                    }
                }

                if n > 0 {
                    counter.fetch_add(1, Ordering::Relaxed);
                    trace!("IN n={} {:?}", n, &buf[0..n]);
                    let mut lock = event_queue.lock().unwrap();
                    let pk = Packet::from(buf.as_slice());
                    // TODO: make this process ACK instead?
                    if pk.is_valid() {
                        lock.push_back((pk, addr));
                    }
                }
            }
            Err(e) => {
                error!("[net] recv error: {}", e)
            }
        }

    }
}
pub fn network_send_thread(socket: UdpSocket, mut transmit_recv: Receiver<OutPacket>, counter: Arc<AtomicU16>) {
    loop {
        // Check if there's any data we need to send out
        if let Ok(out) = transmit_recv.recv() {
            trace!("got packet to send, processing");
            match out {
                OutPacket::Multiple(pk, addr_list) => {
                    trace!("OUT addr_list={:?} pk_len={} py_len={} {}", addr_list, pk.buf_len(), pk.payload_len(), pk.as_hex_str());
                    for addr in addr_list {
                        socket.send_to(pk.as_slice(), addr).unwrap();
                    }
                },
                OutPacket::Single(pk, addr) => {
                    trace!("OUT addr={} pk_len={} py_len={} {}", addr, pk.buf_len(), pk.payload_len(), pk.as_hex_str());
                    socket.send_to(pk.as_slice(), addr).unwrap();
                }
            }
            counter.fetch_add(1, Ordering::Relaxed);
        } else {
            debug!("send_thread: channel closed, exiting");
            break;
        }
    }
}
