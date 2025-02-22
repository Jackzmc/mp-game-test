use int_enum::IntEnum;
use log::trace;
use crate::def::Position;
use crate::packet::{Packet, PacketBuilder};
use crate::PacketSerialize;

#[derive(Debug, Clone)]
pub enum ServerEvent {
    Login { client_index: u32, auth_id: u32 }, // 0x1
    Move { client_index: u32, position: Position }, // 0x2
    PlayerSpawn { client_index: u32, name: String, position: Position }, //0x3,
    Disconnect { client_index: u32, reason: String },
    CommandResult { id: u32, result: bool }
}
impl ServerEvent {
    pub fn get_packet_type(&self) -> u16 {
        match self {
            ServerEvent::Login { .. } => 0x1,
            ServerEvent::Move { .. } => 0x2,
            ServerEvent::PlayerSpawn { .. } => 0x3,
            ServerEvent::Disconnect { .. } => 0x4,
            ServerEvent::CommandResult { .. } => 0x5,
        }
    }
}
impl PacketSerialize for ServerEvent {
    // Serializing to send to client
    fn to_packet_builder(&self) -> PacketBuilder {
        let mut pk = PacketBuilder::new(self.get_packet_type());
        match self {
            ServerEvent::Login { client_index, auth_id } => {
                let buf = pk.buf_mut();
                buf.write_u32(*client_index);
                buf.write_u32(*auth_id);
            },
            ServerEvent::Move { client_index, position } => {
                let buf = pk.buf_mut();
                buf.write_u32(*client_index);
                buf.write_f32(position.x);
                buf.write_f32(position.y);
                buf.write_f32(position.z);
            }
            ServerEvent::PlayerSpawn { client_index, name, position } => {
                let buf = pk.buf_mut();
                buf.write_u32(*client_index);
                buf.write_f32(position.x);
                buf.write_f32(position.y);
                buf.write_f32(position.z);
                buf.write_string(name);
            },
            ServerEvent::Disconnect { client_index, reason } => {
                let buf = pk.buf_mut();
                buf.write_u32(*client_index);
                buf.write_string(reason)
            },
            ServerEvent::CommandResult { id, result } => {
                let buf = pk.buf_mut();
                buf.write_u32(*id);
                buf.write_u8(result.then(|| 1).unwrap_or(0));
            }
        }
        pk
    }

    // For client parsing
    fn from_packet(mut packet: &Packet) -> Result<Self, String> {
        let len = packet.payload_len();
        let pk_type = packet.packet_type();
        let mut buf = packet.payload_buf();
        match pk_type {
            0x1 => {
                trace!("reading 0x1: Server Connect");
                Ok(ServerEvent::Login {
                    client_index: buf.read_u32(),
                    auth_id: buf.read_u32(),
                })
            },
            0x2 => {
                trace!("reading 0x2: Server Move");
                Ok(ServerEvent::Move {
                    client_index: buf.read_u32(),
                    position: Position::new(
                        buf.read_f32(),
                        buf.read_f32(),
                        buf.read_f32()
                    )
                })
            },
            0x3 => {
                trace!("reading 0x3: Server Player Spawn");
                Ok(ServerEvent::PlayerSpawn {
                    client_index: buf.read_u32(),
                    position: Position::new(
                        buf.read_f32(),
                        buf.read_f32(),
                        buf.read_f32()
                    ),
                    name: buf.read_string().unwrap(),

                })
            },
            0x4 => {
                trace!("reading 0x4: Server Disconnect");
                Ok(ServerEvent::Disconnect {
                    client_index: buf.read_u32(),
                    reason: buf.read_string().unwrap()
                })
            },
            0x5 => {
                trace!("reading 0x5: Server CommandResult");
                Ok(ServerEvent::CommandResult {
                    id: buf.read_u32(),
                    result: buf.read_u8() != 0,
                })
            }
            _ => {
                // println!("{:?}", packet.buf());
                Err(format!("Invalid packet type ({}). packet len={}", pk_type, len))
            }
        }
    }
}