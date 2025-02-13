use crate::packet::Packet;

pub mod def;
mod buffer;
pub mod packet;

pub trait PacketSerialize<T> {
    fn to_packet(&self) -> Packet;

    fn from_packet(bytes: Packet) -> Result<T, String>;
}