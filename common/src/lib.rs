use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::packet::{Packet, PacketBuilder};

pub mod def;
mod buffer;
pub mod packet;
pub mod events_client;
pub mod events_server;
pub mod game;

pub const PACKET_PROTOCOL_VERSION: u32 = 0;

pub trait PacketSerialize<T> {
    fn to_packet(&self) -> Packet {
        self.to_packet_builder().finalize()
    }

    fn to_packet_builder(&self) -> PacketBuilder;

    fn from_packet(bytes: &Packet) -> Result<T, String>;
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