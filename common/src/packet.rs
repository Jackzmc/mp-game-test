use log::trace;
use crate::buffer::BitBuffer;
use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::unix_timestamp;

pub struct PacketBuilder {
    buf: BitBuffer,
}

impl PacketBuilder {
    pub fn new(packet_type: u16) -> Self {
        let mut buf = BitBuffer::new(PACKET_HEADER_SIZE, None);
        buf.write_i32(-1); // length, filled later
        buf.write_i16(packet_type as i16);
        buf.write_u32(unix_timestamp());
        buf.write_u32(0x0); //unused on server side
        Self {
            buf,
        }
    }

    // Sets the auth id (defaults to 0)
    pub fn with_auth_id(mut self, auth_id: u32) -> Self {
        self.buf.write_u32_at(0xA, auth_id);
        self
    }

    /// Replaces default timestamp (of when new() called), with a specific timestamp
    pub fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.buf.write_u32_at(0x6, timestamp as u32);
        self
    }

    pub fn buf_mut(&mut self) -> &mut BitBuffer {
        &mut self.buf
    }

    pub fn finalize(mut self) -> Packet {
        let len = self.buf.len() - PACKET_HEADER_SIZE; // subtract the payload length + payload type fields
        self.buf.write_i32_at(0x0, len as i32); // write the payload length
        Packet::new(self.buf)
    }
}

pub const PACKET_HEADER_SIZE: usize = 0xE;

pub struct Packet {
    buf: BitBuffer,
}

impl Packet {
    pub fn new<B: Into<BitBuffer>>(vec: B) -> Self  where BitBuffer: From<B>  {
        let mut buf = BitBuffer::from(vec);
        Self {
            buf: buf
        }
    }

    pub fn is_valid(&self) -> bool {
        self.buf.len() > PACKET_HEADER_SIZE && self.payload_len() > 0 && self.buf.len() >= PACKET_HEADER_SIZE + self.payload_len() as usize
        // TODO: check packet_type?
    }

    // The length of the packet
    pub fn payload_len(&self) -> u32 {
        let py_len = self.buf.peek_i32_at(0) as u32;
        // assert_eq!(pk_len + 4, self.buf.len() as u32, "packet len record != buffer len");
        py_len
    }

    /// Returns the length of the buffer - this may have trailing 0's at the end
    pub fn buf_len(&self) -> u32 {
        self.buf.len() as u32
    }

    pub fn packet_type(&self) -> u16 {
        self.buf.peek_u16_at(0x4)
    }

    pub fn timestamp(&self) -> u32 {
        self.buf.peek_u32_at(0x6)
    }

    pub fn auth_id(&self) -> u32 {
        self.buf.peek_u32_at(0xA)
    }

    pub fn buf_mut(&mut self) -> &mut BitBuffer {
        &mut self.buf
    }

    pub fn buf(&self) -> &BitBuffer {
        &self.buf
    }

    pub fn as_slice(&self) -> &[u8] {
        self.buf.as_slice()
    }

    pub fn payload_buf(&self) -> BitBuffer {
        let end = PACKET_HEADER_SIZE + self.payload_len() as usize;
        self.buf.slice(PACKET_HEADER_SIZE, end)
    }

    pub fn as_hex_str(&self) -> String {
        let mut s = String::with_capacity((self.buf_len() + 4) as usize);

        let header_buf = self.buf.slice(0, PACKET_HEADER_SIZE);
        let payload_buf = self.payload_buf();
        write!(s,"[{}]0x", self.buf_len()).unwrap();
        for i in 0..PACKET_HEADER_SIZE {
            write!(s, "{:02X}", header_buf.peek_u8_at(i)).unwrap();
        }
        write!(s, " ").unwrap();
        for i in 0..self.payload_len() as usize {
            write!(s, "{:02X}", payload_buf.peek_u8_at(i)).unwrap();
        }
        s
    }
}

impl From<&[u8]> for Packet {
    fn from(slice: &[u8]) -> Self {
        Packet {
            buf: BitBuffer::from(slice.to_vec())
        }
    }
}


impl From<Vec<u8>> for Packet {
    fn from(vec: Vec<u8>) -> Self {
        Packet {
            buf: BitBuffer::from(vec)
        }
    }
}