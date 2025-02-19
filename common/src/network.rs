use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::Instant;
use log::trace;
use crate::{NetContainer, NetStat, PacketSerialize, ACK_TIMEOUT_REPLY};
use crate::events_client::ClientEvent;
use crate::events_server::ServerEvent;
use crate::packet::Packet;
#[derive(Clone)]
pub struct ReliableEntry {
    pub seq_id: u16,
    pub packet: Packet,
    pub sent_time: Instant
}
pub struct ReliableQueue {
    client_queue: HashMap<SocketAddr, VecDeque<ReliableEntry>>,
    seq_number: u16
}

impl ReliableQueue {
    pub fn new() -> Self {
        Self { client_queue: HashMap::new(), seq_number: 0 }
    }

    pub fn count(&self, addr: SocketAddr) -> Option<usize> {
        self.client_queue.get(&addr).map(|queue| queue.len())
    }

    pub fn delete_all(&mut self, addr: SocketAddr) {
        self.client_queue.remove(&addr);
    }

    pub fn current_seq_number(&self) -> u16 {
        self.seq_number
    }

    pub fn add_event(&mut self, addr: SocketAddr, event: impl PacketSerialize) -> ReliableEntry {
        // increment first - seq number has to be > 0 (wrap to 1 on overflow)
        let seq = self.seq_number.checked_add(1).unwrap_or(1);
        self.seq_number = seq;
        let seq = self.seq_number;
        let packet = event.to_packet_builder()
            .with_sequence_number(seq)
            .finalize();
        let entry = ReliableEntry {
            seq_id: seq,
            packet: packet,
            sent_time: Instant::now()
        };
        let queue = self.client_queue.entry(addr)
            .or_insert(VecDeque::new());
        queue.push_back(entry.clone());
        entry
    }

    pub fn front(&self, addr: SocketAddr) -> Option<&ReliableEntry> {
        self.client_queue.get(&addr).map(|queue| queue.front()).flatten()
    }

    pub fn next_resend(&mut self, addr: SocketAddr) -> Option<&mut ReliableEntry> {
        self.client_queue.get_mut(&addr).map(|queue| queue.front_mut()).flatten()
            .filter(|item| item.sent_time.elapsed() > ACK_TIMEOUT_REPLY)
    }

    pub fn try_accept_ack(&mut self, addr: SocketAddr, seq_number: u16) -> bool {
        if let Some(queue) = self.client_queue.get_mut(&addr) {
            if let Some(item) = queue.front_mut() {
                if item.seq_id == seq_number {
                    queue.pop_front();
                    trace!("accepting ACK {} for {:?}", seq_number, addr);
                    return true;
                }
            }
        }
        false
    }
}
pub trait Network<EV> {
    fn new(addr: SocketAddr) -> Self;

    fn stat(&self) -> &NetStat;

    /// Returns the number of packets sent and received since last called.
    /// Resets the count
    fn pks_per_interval(&mut self) -> NetContainer<u16>;

    fn add_reliable_packet(&mut self, addr: SocketAddr, event: EV) -> ReliableEntry;

    fn send(&self, packet: Packet, addr: SocketAddr) -> Result<(), String>;

    fn send_multiple(&self, packet: Packet, addr_list: Vec<SocketAddr>) -> Result<(), String>;

    fn event_queue_len(&self) -> usize;


    /// Pops the next event off, if any
    fn next_event(&mut self) -> Option<(Packet, EV, SocketAddr)>;
}