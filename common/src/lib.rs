use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use simple_moving_average::{NoSumSMA, SMA};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::packet::{Packet, PacketBuilder};

pub mod def;
mod buffer;
pub mod packet;
pub mod events_client;
pub mod events_server;
pub mod game;
pub mod network;

pub const PACKET_PROTOCOL_VERSION: u32 = 0;
/// How long to wait until we consider packet was lost and resend?
pub static ACK_TIMEOUT_REPLY: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub struct NetStat {
    packet_count: Arc<NetContainer<AtomicU16>>,
    activity_time: Arc<Mutex<NetContainer<Option<Instant>>>>,
    ping_time: Arc<Mutex<NoSumSMA<u16, u16, 10>>>
}

pub enum NetDirection {
    In,
    Out
}
pub struct NetContainer<T> {
    pub tx: T,
    pub rx: T
}
impl<T> NetContainer<T> {
    pub fn new(tx: T, rx: T) -> Self {
        Self { tx, rx }
    }
}

impl NetStat {
    pub fn new() -> Self {
        Self {
            packet_count: Arc::new(NetContainer::new(AtomicU16::new(0), AtomicU16::new(0))),
            activity_time: Arc::new(Mutex::new(NetContainer::new(None, None))),
            ping_time: Arc::new(Mutex::new(NoSumSMA::new())),
        }
    }
    pub fn mark_activity(&mut self, dir: NetDirection) {
        let mut lock = self.activity_time.lock().unwrap();
        match dir {
            NetDirection::Out => lock.tx = Some(Instant::now()),
            NetDirection::In => lock.rx = Some(Instant::now()),
        }
    }
    pub fn activity_time(&self) -> NetContainer<Option<Instant>> {
        let lock = self.activity_time.lock().unwrap();
        NetContainer::new(lock.tx, lock.rx)
    }
    pub fn activity_time_as_secs_f32(&self) -> NetContainer<Option<String>> {
        let lock = self.activity_time.lock().unwrap();
        NetContainer::new(
            lock.tx.map(|a| a.elapsed().as_secs_f32().to_string()),
            lock.rx.map(|a| a.elapsed().as_secs_f32().to_string()),
        )
    }
    /// Returns if there has been any activity (tx or rx) within time frame
    pub fn has_activity_within(&self, duration: Duration) -> bool {
        let lock = self.activity_time.lock().unwrap();
        if let Some(tx_in) = lock.tx {
            if tx_in.elapsed() < duration {
                return true
            }
        }
        if let Some(rx_in) = lock.rx {
            if rx_in.elapsed() < duration {
                return true
            }
        }
        false
    }
    pub fn reset_pk_count(&mut self) {
        self.packet_count.tx.store(0, Ordering::Relaxed);
        self.packet_count.tx.store(0, Ordering::Relaxed);
    }

    pub fn pk_count(&self) -> NetContainer<u16> {
        let (pk_in, pk_out) = (
            self.packet_count.tx.load(Ordering::Relaxed),
            self.packet_count.rx.load(Ordering::Relaxed)
        );
        NetContainer::new(pk_in, pk_out)
    }

    pub fn inc_pk_count(&mut self, dir: NetDirection) {
        match dir {
            NetDirection::In => {
                self.packet_count.rx.fetch_add(1, Ordering::Relaxed);
            },
            NetDirection::Out => {
                self.packet_count.tx.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn add_ping(&mut self, ping: u16) {
        let mut lock = self.ping_time.lock().unwrap();
        lock.add_sample(ping);
    }

    pub fn avg_ping(&self) -> u16 {
        let lock = self.ping_time.lock().unwrap();
        lock.get_average()
    }
}

pub trait PacketSerialize {
    fn to_packet(&self) -> Packet {
        self.to_packet_builder().finalize()
    }

    fn to_packet_builder(&self) -> PacketBuilder;

    fn from_packet(bytes: &Packet) -> Result<Self, String> where Self: Sized;
}

// pub enum ClientId {
//     AuthId(AuthId),
//     Addr(SocketAddr),
//     ClientIndex(ClientIndex)
// }
// impl ClientId {
//     pub fn from_index(index: u32) -> Self {
//         ClientId::ClientIndex(ClientIndex(index))
//     }
//     pub fn from_addr(addr: SocketAddr) -> Self {
//         ClientId::Addr(addr)
//     }
//     pub fn from_auth_id(auth_id: u32) -> Self {
//         ClientId::AuthId(AuthId(auth_id))
//     }
// }

pub struct AuthId(u32);
pub struct ClientIndex(u32);

pub fn unix_timestamp() -> u32 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32
}

pub fn setup_logger() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::filter::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace,mp-game-test-common=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}