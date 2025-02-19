use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvError, Sender};
use std::thread;
use std::time::Instant;
use anyhow::anyhow;
use log::{debug, error, info, trace, warn};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::packet::{Packet, PACKET_HEADER_SIZE};
use mp_game_test_common::{NetContainer, NetDirection, NetStat, PacketSerialize, ACK_TIMEOUT_REPLY, PACKET_PROTOCOL_VERSION};
use mp_game_test_common::events_server::ServerEvent;
use mp_game_test_common::network::{Network, ReliableQueue};

pub struct NetServer {
    event_queue: EventQueue,
    socket: UdpSocket,
    // rx: Sender<ClientEvent>,
    transmit_out_tx: Sender<OutPacket>,
    send_thread: thread::JoinHandle<()>,
    recv_thread: thread::JoinHandle<()>,

    reliable_queue: Arc<Mutex<ReliableQueue>>,
    seq_number: u16,
    net_stat: NetStat
}

#[derive(Clone)]
pub struct ReliableEntry {
    pub seq_id: u16,
    pub packet: Packet,
    pub sent_time: Instant
}

pub enum OutPacket {
    Single(Packet, SocketAddr),
    Multiple(Packet, Vec<SocketAddr>),
}

type EventQueue = Arc<Mutex<VecDeque<(Packet, ClientEvent, SocketAddr)>>>;
impl NetServer {
    pub(crate) fn new(addr: SocketAddr) -> Self  {
        // socket.set_nonblocking(false);
        let mut sock = UdpSocket::bind(addr).expect("Failed to bind UDP socket");
        let (tx, rx) = channel::<OutPacket>();
        let event_queue = Arc::new(Mutex::new(VecDeque::new()));
        let reliable_queue = Arc::new(Mutex::new(ReliableQueue::new()));
        let net_stat = NetStat::new();

        let send_thread = {
            let socket = sock.try_clone().unwrap();
            let net_stat = net_stat.clone();
            thread::spawn(move || network_send_thread(socket, rx, net_stat))
        };
        let recv_thread = {
            let event_queue = event_queue.clone();
            let socket = sock.try_clone().unwrap();
            let reliable_queue = reliable_queue.clone();
            let net_stat = net_stat.clone();
            thread::spawn(move || network_recv_thread(socket, event_queue.clone(), reliable_queue, net_stat))
        };

        info!("server listening at UDP {:?}", sock.local_addr().unwrap());

        NetServer {
            transmit_out_tx: tx,
            recv_thread,
            send_thread,
            event_queue,
            socket: sock,
            reliable_queue,
            seq_number: 0,
            net_stat
        }
    }

    pub(crate) fn stat(&self) -> &NetStat {
        &self.net_stat
    }

    /// Returns the number of packets sent and received since last called.
    /// Resets the count
    pub(crate) fn pks_per_interval(&mut self) -> NetContainer<u16> {
        let pk_stat = self.net_stat.pk_count();
        self.net_stat.reset_pk_count();
        pk_stat
    }

    pub fn send_to(&self, event: &ServerEvent, addr: SocketAddr) -> Result<(), String> {
        let pk = event.to_packet();
        let buf = pk.as_slice();;
        debug!("EVENT[{}B] {:?} {:?}", buf.len(), addr, event);
        self.transmit_out_tx.send(OutPacket::Single(pk, addr)).map_err(|e| e.to_string())
    }

    /// Sends an event to a specified addr, returning Ok(sequence_number)
    pub fn send_to_reliable(&self, event: ServerEvent, addr: SocketAddr) -> Result<u16, String> {
        let mut lock = self.reliable_queue.lock().unwrap();
        let entry = lock.add_event(addr, event.clone());
        self.send_to(&event ,addr).map(|_| entry.seq_id)
    }

    fn event_queue_len(&self) -> usize {
        self.event_queue.lock().unwrap().len()
    }


    /// Pops the next incoming event off, if any
    pub(crate) fn next_event(&mut self) -> Option<(Packet, ClientEvent, SocketAddr)> {
        let mut lock = self.event_queue.lock().unwrap();
        lock.pop_front()
    }
}

pub fn network_recv_thread(
    socket: UdpSocket,
    mut event_queue: EventQueue,
    reliable_queue: Arc<Mutex<ReliableQueue>>,
    mut net_stat: NetStat
) {
    let mut buf = Vec::with_capacity(2048);
    loop {
        // Check if we received any data, and add it to packet queue
        buf.resize(2048, 0);
        match socket.recv_from(&mut buf) {
            Ok((n, addr)) => {
                net_stat.mark_activity(NetDirection::In);
                if n > 0 {
                    let pk = Packet::from(buf.as_slice());
                    trace!("IN n={} {}", n, pk.as_hex_str());
                    // TODO: make this process ACK instead?
                    if !pk.is_valid() {
                        continue;
                    }
                    net_stat.inc_pk_count(NetDirection::Out);
                    match ClientEvent::from_packet(&pk) {
                        Ok(ev) => {
                            // If it's ACK packet, handle it here
                            if let ClientEvent::Ack {seq_number} = ev {
                                trace!("got ACK {:?}", seq_number);
                                let mut lock = reliable_queue.lock().unwrap();
                                lock.try_accept_ack(addr, seq_number);
                            } else {
                                trace!("received event, pushing to queue");
                                let mut lock = event_queue.lock().unwrap();
                                lock.push_back((pk, ev, addr));
                            }
                        }
                        Err(err) => {
                            warn!("bad packet: {:?}", err);
                        }
                    }
                }
                // Check ACK state
                {
                    // This code assumes that the server will receive a regular amount of traffic so this is processed
                    // If zero clients are sending data this will stall
                    let mut lock = reliable_queue.lock().unwrap();
                    if let Some(item) = lock.next_resend(addr) {
                        trace!("ACK timeout (seq#{}). resending (original pk {} ms ago)", item.seq_id, item.sent_time.elapsed().as_millis());
                        socket.send_to(item.packet.as_slice(), addr).ok();
                        item.sent_time = Instant::now(); // update timestamp so client has another chance
                    }
                }
            }
            Err(e) => {
                error!("[net] recv error: {}", e)
            }
        }

    }
}
pub fn network_send_thread(
    socket: UdpSocket,
    mut transmit_recv: Receiver<OutPacket>,
    mut net_stat: NetStat
) {
    loop {
        // Check if there's any data we need to send out
        if let Ok(out) = transmit_recv.recv() {
            net_stat.mark_activity(NetDirection::Out);
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
            net_stat.inc_pk_count(NetDirection::Out);
        } else {
            debug!("send_thread: channel closed, exiting");
            break;
        }
    }
}
