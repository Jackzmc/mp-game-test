use log::trace;
use crate::def::Position;
use crate::game::Action;
use crate::packet::{Packet, PacketBuilder};
use crate::PacketSerialize;

#[derive(Debug)]
pub enum ClientEvent {
    Ack { seq_number: u16 },
    Login { version: u32, name: String }, // 0x0
    PerformAction { actions: Action }, // ox1
    Disconnect { reason: String},
}
impl ClientEvent {
    pub fn get_packet_type(&self) -> u16 {
        match self {
            ClientEvent::Ack { .. }  => 0x0,
            ClientEvent::Login { .. } => 0x1,
            ClientEvent::PerformAction { .. } => 0x2,
            ClientEvent::Disconnect { .. } => 0x3
        }
    }
}
impl PacketSerialize<ClientEvent> for ClientEvent {
    // For the client to the server
    fn to_packet_builder(&self) -> PacketBuilder {
        let mut pk = PacketBuilder::new(self.get_packet_type());
        match self {
            ClientEvent::Ack { seq_number } => {
                let buf = pk.buf_mut();
                buf.write_u16(*seq_number);
            }
            ClientEvent::Login { version, name } => {
                let buf = pk.buf_mut();
                buf.write_u32(*version);
                buf.write_string(name);
            },
            ClientEvent::PerformAction { actions } => {
                let buf = pk.buf_mut();
                buf.write_u32(actions.bits());
            },
            ClientEvent::Disconnect { reason } => {
                let buf = pk.buf_mut();
                buf.write_string(reason);
            }
        }
        pk
    }
    // For the server to parse
    fn from_packet(mut packet: &Packet) -> Result<Self, String> {
        let len = packet.payload_len();
        let pk_type = packet.packet_type();
        let mut buf = packet.payload_buf();
        match pk_type {
            0x0 => {
                trace!("reading 0x0: Ack");
                Ok(ClientEvent::Ack {
                    seq_number: buf.read_u16()
                })
            },
            0x1 => {
                trace!("reading 0x1: Client Login");
                Ok(ClientEvent::Login {
                    version: buf.read_u32(),
                    name: buf.read_string().unwrap()
                })
            },
            0x2 => {
                trace!("reading 0x2: Client Move");
                Ok(ClientEvent::PerformAction {
                    actions: Action::from_bits_retain(buf.read_u32()),
                })
            },
            0x3 => {
                trace!("reading 0x3: Client Disconnect");
                Ok(ClientEvent::Disconnect {
                    reason: buf.read_string().unwrap()
                })
            },
            _ => {
                // println!("{:?}", packet.buf());
                Err(format!("Invalid packet type ({}). packet len={}", pk_type, len))
            }
        }
    }
}