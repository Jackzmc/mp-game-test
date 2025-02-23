use int_enum::IntEnum;
use log::trace;
use crate::def::Vector3;
use crate::packet::{Packet, PacketBuilder};
use crate::PacketSerialize;

#[derive(Debug, Clone)]
pub enum ServerEvent {
    Login { client_index: u32, auth_id: u32 }, // 0x1
    Move { client_index: u32, position: Vector3, angles: Vector3, velocity: Vector3 }, // 0x2
    PlayerSpawn { client_index: u32, name: String, position: Vector3, angles: Vector3 }, //0x3,
    Disconnect { client_index: u32, reason: String },
}
impl ServerEvent {
    pub fn get_packet_type(&self) -> u16 {
        match self {
            ServerEvent::Login { .. } => 0x1,
            ServerEvent::Move { .. } => 0x2,
            ServerEvent::PlayerSpawn { .. } => 0x3,
            ServerEvent::Disconnect { .. } => 0x4,
        }
    }
    pub fn is_reliable(&self) -> bool {
        match self {
            ServerEvent::Login { .. } => true,
            ServerEvent::PlayerSpawn { .. } => true,
            ServerEvent::Move { .. } => false,
            ServerEvent::Disconnect { .. } => true,
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
            ServerEvent::Move { client_index, position, angles, velocity } => {
                let buf = pk.buf_mut();
                buf.write_u32(*client_index);
                buf.write_f32(position.x);
                buf.write_f32(position.y);
                buf.write_f32(position.z);
                buf.write_f32(angles.x);
                buf.write_f32(angles.y);
                buf.write_f32(angles.z);
                buf.write_f32(velocity.x);
                buf.write_f32(velocity.y);
                buf.write_f32(velocity.z);
            }
            ServerEvent::PlayerSpawn { client_index, name, position, angles } => {
                let buf = pk.buf_mut();
                buf.write_u32(*client_index);
                buf.write_f32(position.x);
                buf.write_f32(position.y);
                buf.write_f32(position.z);
                buf.write_f32(angles.x);
                buf.write_f32(angles.y);
                buf.write_f32(angles.z);
                buf.write_string(name);
            },
            ServerEvent::Disconnect { client_index, reason } => {
                let buf = pk.buf_mut();
                buf.write_u32(*client_index);
                buf.write_string(reason)
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
                    position: Vector3::new(
                        buf.read_f32(),
                        buf.read_f32(),
                        buf.read_f32()
                    ),
                    angles: Vector3::new(
                        buf.read_f32(),
                        buf.read_f32(),
                        buf.read_f32()
                    ),
                    velocity: Vector3::new(
                        buf.read_f32(),
                        buf.read_f32(),
                        buf.read_f32()
                    ),
                })
            },
            0x3 => {
                trace!("reading 0x3: Server Player Spawn");
                Ok(ServerEvent::PlayerSpawn {
                    client_index: buf.read_u32(),
                    position: Vector3::new(
                        buf.read_f32(),
                        buf.read_f32(),
                        buf.read_f32()
                    ),
                    angles: Vector3::new(
                        buf.read_f32(),
                        buf.read_f32(),
                        buf.read_f32()
                    ),
                    name: buf.read_string().unwrap(),

                })
            },
            0x4 => {
                trace!("reading 0x4: Disconnect");
                Ok(ServerEvent::Disconnect {
                    client_index: buf.read_u32(),
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