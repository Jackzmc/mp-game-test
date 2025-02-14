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

    fn from_packet(bytes: Packet) -> Result<T, String>;
}