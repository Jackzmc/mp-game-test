use log::trace;
use crate::def::Position;
use crate::packet::{Packet, PacketBuilder};
use crate::PacketSerialize;

#[derive(Debug)]
pub enum ClientEvent {
    Login { version: u32, name: String }, // 0x0
    Move { position: Position }, // ox1
}
impl ClientEvent {
    pub fn get_packet_type(&self) -> u16 {
        match self {
            ClientEvent::Login { .. } => 0x1,
            ClientEvent::Move { .. } => 0x2
        }
    }
}
impl PacketSerialize<ClientEvent> for ClientEvent {
    // For the client to the server
    fn to_packet_builder(&self) -> PacketBuilder {
        let mut pk = PacketBuilder::new(self.get_packet_type());
        match self {
            ClientEvent::Login { version, name } => {
                let buf = pk.buf_mut();
                buf.write_u32(*version);
                buf.write_string(name);
            },
            ClientEvent::Move { position } => {
                let buf = pk.buf_mut();
                buf.write_f32(position.x);
                buf.write_f32(position.y);
                buf.write_f32(position.z);
            }
        }
        pk
    }
    // For the server to parse
    fn from_packet(mut packet: Packet) -> Result<Self, String> {
        let len = packet.payload_len();
        let pk_type = packet.packet_type();
        let mut buf = packet.payload_buf();
        match pk_type {
            0x1 => {
                trace!("reading 0x1: Client Login");
                Ok(ClientEvent::Login {
                    version: buf.read_u32(),
                    name: buf.read_string().unwrap()
                })
            },
            0x2 => {
                trace!("reading 0x2: Client Move");
                Ok(ClientEvent::Move {
                    position: Position::new(
                        buf.read_f32(),
                        buf.read_f32(),
                        buf.read_f32()
                    )
                })
            },
            _ => {
                // println!("{:?}", packet.buf());
                Err(format!("Invalid packet type ({}). packet len={}", pk_type, len))
            }
        }
    }
}