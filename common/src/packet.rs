use log::{debug, trace};
use crate::buffer::BitBuffer;
use std::fmt::Write;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, format_err};
use int_enum::IntEnum;
use zstd::DEFAULT_COMPRESSION_LEVEL;
use zstd::stream::copy_encode;
use crate::unix_timestamp;

pub struct PacketBuilder {
    buf: BitBuffer,
}

#[repr(usize)]
#[derive(IntEnum)]
enum PacketHeaderOffset {
    Length = 0x0,           // u16
    PacketType = 0x2,       // u8
    Timestamp = 0x3,        // u32
    AuthId = 0x7,           // u32
    SeqNum = 0xB,           // u16

    Payload = 0xD
}

impl PacketBuilder {
    pub fn new(packet_type: u8) -> Self {
        let buf = BitBuffer::with_capacity(PacketHeaderOffset::PacketType.into());
        let mut builder = Self {
            buf
        }
            .with_length(u16::MAX)
            .with_type(packet_type)
            .with_timestamp(unix_timestamp())
            .with_auth_id(0) // unused on server side
            .with_sequence_number(0); // only set for reliable
        // Set cursor to end of header - prevent payload overwriting
        builder.buf.set_offset_pos(PacketHeaderOffset::Payload.into()).unwrap();
        builder
    }

    fn with_type(mut self, pk_type: u8) -> Self {
        self.buf.write_u8_at(PacketHeaderOffset::PacketType.into(), pk_type);
        self
    }

    fn with_length(mut self, len: u16) -> Self {
        self.buf.write_u16_at(PacketHeaderOffset::Length.into(), len);
        self
    }

    // Sets the auth id (defaults to 0)
    // Note: This field conflicts with sequence_number, auth id is only for outgoing client packets
    pub fn with_auth_id(mut self, auth_id: u32) -> Self {
        self.buf.write_u32_at(PacketHeaderOffset::AuthId.into(), auth_id);
        self
    }

    // Sets the sequence number.
    // Note: This field conflicts with auth_id, seq number is only for outgoing server packets
    pub fn with_sequence_number(mut self, seq_num: u16) -> Self {
        self.buf.write_u16_at(PacketHeaderOffset::SeqNum.into(), seq_num);
        self
    }

    /// Replaces default timestamp (of when new() called), with a specific timestamp
    pub fn with_timestamp(mut self, timestamp: u32) -> Self {
        self.buf.write_u32_at(PacketHeaderOffset::Timestamp.into(), timestamp);
        self
    }

    pub fn buf_mut(&mut self) -> &mut BitBuffer {
        &mut self.buf
    }

    pub fn finalize(mut self) -> Packet {
        let len: usize = self.buf.len() - PacketHeaderOffset::Payload as usize; // subtract the payload length + payload type fields
        self = self.with_length(len as u16);
        Packet::new(self.buf)
    }
}

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
        if buf.len() <= PacketHeaderOffset::Payload.into() {
            return Err(anyhow!("packet len ({}) is smaller than header size ({})", buf.len(), PacketHeaderOffset::Payload as usize));
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
        trace!("{:x?}", slice);
        let vec = zstd::bulk::decompress(slice, slice.len() * 2)
            .map_err(|e| format_err!("decompress failed: {}", e))?;
        Self::try_from(vec)
    }

    // The length of the packet
    pub fn payload_len(&self) -> u16 {
        let py_len = self.buf.peek_u16_at(PacketHeaderOffset::Length.into());
        // assert!(self.buf_len() >= py_len, "payload len exceeds buffer len (invalid value?)");
        // assert_eq!(pk_len + 4, self.buf.len() as u32, "packet len record != buffer len");
        py_len
    }

    /// Returns the length of the buffer - this may have trailing 0's at the end
    pub fn buf_len(&self) -> u32 {
        self.buf.len() as u32
    }

    pub fn packet_type(&self) -> u8 {
        self.buf.peek_u8_at(PacketHeaderOffset::PacketType.into())
    }

    pub fn timestamp(&self) -> u32 {
        self.buf.peek_u32_at(PacketHeaderOffset::Timestamp.into())
    }

    /// Gets the sequence number. 0 if not a reliable (requiring ACK) packet.
    pub fn sequence_number(&self) -> u16 {
        self.buf.peek_u16_at(PacketHeaderOffset::SeqNum.into())
    }

    /// Gets the auth id from client. May be 0 if Login event.
    /// Only for server reading client sent packets
    pub fn auth_id(&self) -> u32 {
        self.buf.peek_u32_at(PacketHeaderOffset::AuthId.into())
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
        let start: usize = PacketHeaderOffset::Payload.into();
        let end = start + self.payload_len() as usize;
        self.buf.slice(start, end)
    }

    pub fn as_hex_str(&self) -> String {
        let mut s = String::with_capacity((self.buf_len() + 4) as usize);

        let header_buf = self.buf.slice(0, PacketHeaderOffset::Payload.into());
        let payload_buf = self.payload_buf();
        write!(s,"[{}]0x", self.buf_len()).unwrap();
        for i in 0..PacketHeaderOffset::Payload.into() {
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