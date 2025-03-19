use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::ops::{Add, AddAssign};
use log::{debug, trace};
use crate::{PacketSerialize};
use crate::buffer::BitBuffer;
use crate::packet::{Packet, PacketBuilder};

pub const MAX_PLAYERS: usize = 32;

#[derive( Clone, Copy)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32
}

impl Debug for Vector3 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{})", self.x, self.y, self.z)
    }
}

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Vector3 {
        Vector3 { x, y, z }
    }

    pub fn zero() -> Vector3 {
        Vector3 { x: 0.0, y: 0.0, z: 0.0 }
    }

    pub fn to_vec(&self) -> Vec<f32> {
        vec![self.x, self.y, self.z]
    }
}

pub fn get_direction_vector(direction: &Vector3, ang: &Vector3) -> Vector3 {
    let s = Vector3 {
        x: direction.x * ang.z.cos() - direction.z * ang.x.cos() * ang.z.sin() - direction.y * ang.x.sin() * ang.z.sin(),
        y: direction.x * ang.z.sin() + direction.z * ang.x.cos() * ang.z.cos() + direction.y * ang.x.sin() * ang.z.cos(),
        z: direction.z * ang.x.sin() - direction.y * ang.x.cos(),
    };
    debug!("{:?}", s);
    s
}

impl Vector3 {
    pub fn add(self, other: &Vector3) -> Vector3 {
        Vector3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

