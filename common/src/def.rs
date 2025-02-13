use log::trace;
use crate::{PacketSerialize};
use crate::buffer::BitBuffer;
use crate::packet::{Packet, PacketBuilder};

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Position {
        Position { x, y, z }
    }
}

#[derive(Debug)]
pub enum ClientEvent {
    ClientConnect { name: String }, // 0x0
    Move { position: Position }, // ox1
}
impl ClientEvent {
    pub fn get_packet_type(&self) -> u16 {
        match self {
            ClientEvent::ClientConnect { .. } => 0x1,
            ClientEvent::Move { .. } => 0x2
        }
    }
}

pub fn start_packet_header(packet_type: u16) -> Vec<u8> {
    packet_type.to_be_bytes().to_vec()
}

impl PacketSerialize<ClientEvent> for ClientEvent {
    fn to_packet(&self) -> Packet {
        let mut pk = PacketBuilder::new(self.get_packet_type());
        match self {
            ClientEvent::ClientConnect { name } => {
                let buf = pk.buf_mut();
                buf.write_string(name);
            },
            ClientEvent::Move { position } => {
                let buf = pk.buf_mut();
                buf.write_float(position.x);
                buf.write_float(position.y);
                buf.write_float(position.z);
            }
        }
        pk.finalize()
    }

    fn from_packet(mut packet: Packet) -> Result<Self, String> {
        let len = packet.payload_len();
        let pk_type = packet.packet_type();
        let mut buf = packet.payload_buf();
        match pk_type {
            0x1 => {
                trace!("reading 0x1: Client Connect");
                Ok(ClientEvent::ClientConnect {
                    name: buf.read_string().unwrap()
                })
            },
            0x2 => {
                trace!("reading 0x2: Client Move");
                Ok(ClientEvent::Move {
                    position: Position::new(
                        buf.read_float(),
                        buf.read_float(),
                        buf.read_float()
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