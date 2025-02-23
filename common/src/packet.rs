use log::{debug, trace};
use crate::buffer::BitBuffer;
use std::fmt::Write;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::anyhow;
use zstd::DEFAULT_COMPRESSION_LEVEL;
use zstd::stream::copy_encode;
use crate::unix_timestamp;

pub struct PacketBuilder {
    buf: BitBuffer,
}

impl PacketBuilder {
    pub fn new(packet_type: u16) -> Self {
        let mut buf = BitBuffer::with_capacity(PACKET_HEADER_SIZE);
        buf.write_i32(-1); // length, filled later
        buf.write_i16(packet_type as i16);
        buf.write_u32(unix_timestamp());
        buf.write_u32(0x0); //unused on server side
        Self {
            buf,
        }
    }

    // Sets the auth id (defaults to 0)
    // Note: This field conflicts with sequence_number, auth id is only for outgoing client packets
    pub fn with_auth_id(mut self, auth_id: u32) -> Self {
        self.buf.write_u32_at(0xA, auth_id);
        self
    }

    // Sets the sequence number.
    // Note: This field conflicts with auth_id, seq number is only for outgoing server packets
    pub fn with_sequence_number(mut self, seq_num: u16) -> Self {
        self.buf.write_u16_at(0xA, seq_num);
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
        self.buf.write_u32_at(0x0, len as u32); // write the payload length
        trace!("finalize. buflen={} bufcursor={}", self.buf.len(), self.buf.offset_pos());
        Packet::new(self.buf)
    }
}

pub const PACKET_HEADER_SIZE: usize = 0xE;

#[derive(Clone)]
pub struct Packet {
    buf: BitBuffer,
}

impl Packet {
    pub fn new<B: Into<BitBuffer>>(vec: B) -> Self  where BitBuffer: From<B>  {
        let buf = BitBuffer::from(vec);
        Self {
            buf
        }
    }

    pub fn try_from<B: Into<BitBuffer>>(buf: B) -> Result<Self, anyhow::Error> where BitBuffer: From<B> {
        let buf: BitBuffer = buf.try_into()?;
        if buf.len() <= PACKET_HEADER_SIZE {
            return Err(anyhow!("packet len ({}) is smaller than header size ({})", buf.len(), PACKET_HEADER_SIZE));
        }
        let pk = Self { buf };
        let py_len = pk.payload_len();
        if py_len == 0 {
            return Err(anyhow!("payload length is invalid (0)"))
        } else if pk.buf.len() < py_len as usize {
            return Err(anyhow!("buffer length too small ({}) for payload ({})", pk.buf.len(), py_len));
        }
        Ok(pk)
    }

    pub fn try_decompress_from_slice(slice: &[u8]) -> anyhow::Result<Self> {
        trace!("len = {}", slice.len() * 2);
        let vec = zstd::bulk::decompress(slice, slice.len() * 2)?;
        trace!("vec_len={} cap={} ", vec.len(), vec.capacity());
        Self::try_from(vec)
    }

    pub fn is_valid(&self) -> bool {
        trace!("buf len = {}\tpayload len = {}", self.buf.len(), self.payload_len());
        self.buf.len() > PACKET_HEADER_SIZE && self.payload_len() > 0 && self.buf.len() >= PACKET_HEADER_SIZE + self.payload_len() as usize
        // TODO: check packet_type?
    }

    // The length of the packet
    pub fn payload_len(&self) -> u32 {
        let py_len = self.buf.peek_u32_at(0);
        // assert!(self.buf_len() >= py_len, "payload len exceeds buffer len (invalid value?)");
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

    /// Gets the sequence number. 0 if not a reliable (requiring ACK) packet.
    /// Only for client reading server sent packets.
    pub fn sequence_number(&self) -> u16 {
        self.buf.peek_u16_at(0xA)
    }

    /// Gets the auth id from client. May be 0 if Login event.
    /// Only for server reading client sent packets
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
        self.buf.as_bytes()
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

    pub fn compress(&self) -> std::io::Result<Vec<u8>> {
        zstd::bulk::compress(self.buf.as_bytes(), DEFAULT_COMPRESSION_LEVEL)
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