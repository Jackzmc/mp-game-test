use log::trace;
use crate::buffer::BitBuffer;
use crate::def::ClientEvent;

pub struct PacketBuilder {
    buf: BitBuffer
}

impl PacketBuilder {
    pub fn new(packet_type: u16) -> Self {
        let mut buf = BitBuffer::new(PACKET_HEADER_SIZE, None);
        trace!("{:?}", buf);
        buf.write_int(-1); // length, filled later
        trace!("{:?}", buf);
        buf.write_short(packet_type as i16);
        trace!("{:?}", buf);
        Self {
            buf
        }
    }

    pub fn buf_mut(&mut self) -> &mut BitBuffer {
        &mut self.buf
    }

    pub fn new_event(client_event: ClientEvent) {

    }

    pub fn finalize(mut self) -> Packet {
        trace!("{:?}", self.buf);
        let len = self.buf.len() - PACKET_HEADER_SIZE; // subtract the payload length + payload type fields
        trace!("vec len={}, payload len field = {}", self.buf.len(), len);
        self.buf.write_int_at(0x0, len as i32); // write the payload length
        trace!("{:?}", self.buf);
        Packet::new(self.buf)
    }
}

pub const PACKET_HEADER_SIZE: usize = 6;

pub struct Packet {
    buf: BitBuffer,
}

impl Packet {
    pub fn new<B: Into<BitBuffer>>(vec: B) -> Self  where BitBuffer: From<B>  {
        let mut buf = BitBuffer::from(vec);
        // Jump cursor to payload
        buf.set_cursor(PACKET_HEADER_SIZE).unwrap();
        Self {
            buf: buf
        }
    }

    // The length of the packet
    pub fn payload_len(&self) -> u32 {
        let py_len = self.buf.peek_int_at(0) as u32;
        // assert_eq!(pk_len + 4, self.buf.len() as u32, "packet len record != buffer len");
        py_len
    }

    /// Returns the length of the buffer - this may have trailing 0's at the end
    pub fn buf_len(&self) -> u32 {
        self.buf.len() as u32
    }

    pub fn packet_type(&self) -> u16 {
        self.buf.peek_short_at(0x4) as u16
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
        self.buf.slice(PACKET_HEADER_SIZE as usize, end)
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