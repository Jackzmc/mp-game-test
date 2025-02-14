use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use log::trace;
use rand::random;
use crate::{PacketSerialize};
use crate::buffer::BitBuffer;
use crate::packet::{Packet, PacketBuilder};

pub const MAX_PLAYERS: usize = 32;

#[derive( Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32
}

impl Debug for Position {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{})", self.x, self.y, self.z)
    }
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Position {
        Position { x, y, z }
    }

    pub fn zero() -> Position {
        Position { x: 0.0, y: 0.0, z: 0.0 }
    }
}
