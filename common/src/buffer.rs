use std::fmt::Write;
use std::ffi::{CStr, FromBytesUntilNulError};
use std::fmt::{Debug, Formatter};
use std::io;
use std::io::{Cursor, Read, Write as OtherWrite};
use std::ops::Range;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use log::trace;
use zstd::DEFAULT_COMPRESSION_LEVEL;
use zstd::zstd_safe::CParameter::CompressionLevel;
use crate::packet::Packet;

#[derive(Clone)]
pub struct BitBuffer {
    current_offset: usize,
    // offset: usize, // prob can just use vec.length but for now it works
    vec: Vec<u8>
}

impl BitBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        let mut vec = Vec::with_capacity(capacity);
        Self {
            current_offset: 0,
            vec
        }
    }

    fn _buf_cursor(&self, offset: usize) -> Cursor<&Vec<u8>> {
        let mut c = Cursor::new(&self.vec);
        c.set_position(offset as u64);
        c
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.vec[0..self.current_offset]
    }

    pub fn set_offset_pos(&mut self, offset: usize) -> Result<(), String>{
        if offset > self.vec.capacity() {
            return Err(format!("offset {}, is greater than capacity {}", offset, self.vec.capacity()).to_string())
        }
        self.current_offset = offset;
        Ok(())
    }

    /// Reserves more space if given offset and length exceeds vec
    fn try_expand(&mut self, offset: usize, len: usize)  {
        if offset + len > self.vec.len() {
            self.vec.resize(offset + len, 0);
        }
    }

    pub fn write_i8(&mut self, value: i8) {
        self.write_i8_at(self.current_offset, value);
        self.current_offset += 1;
    }

    pub fn write_i8_at(&mut self, offset: usize, value: i8) {
        self.try_expand(offset, 1);
        self.vec[offset] = value as u8;
    }

    pub fn write_u8(&mut self, value: u8) {
        self.write_u8_at(self.current_offset, value);
        self.current_offset += 1;
    }

    /// Does not call try_expand
    pub fn write_u8_at_unchecked(&mut self, offset: usize, value: u8) {
        self.vec[offset] = value;
    }

    pub fn write_u8_at(&mut self, offset: usize, value: u8) {
        self.try_expand(offset, 1);
        self.write_u8_at_unchecked(offset, value);
    }

    pub fn write_i16(&mut self, value: i16) {
        self.write_i16_at(self.current_offset, value);
        self.current_offset += 2;
    }

    pub fn write_i16_at(&mut self, mut offset: usize, value: i16) {
        self.try_expand(offset, 2);
        for b in value.to_le_bytes() {
            self.write_u8_at_unchecked(offset, b);
            offset += 1;
        }
    }

    pub fn write_u16(&mut self, value: u16) {
        self.write_u16_at(self.current_offset, value);
        self.current_offset += 2;
    }

    pub fn write_u16_at(&mut self, mut offset: usize, value: u16) {
        self.try_expand(offset, 2);
        for b in value.to_le_bytes() {
            self.write_u8_at_unchecked(offset, b);
            offset += 1;
        }
    }

    pub fn write_i32(&mut self, value: i32) {
        self.write_i32_at(self.current_offset, value);
        self.current_offset += 4;
    }

    pub fn write_i32_at(&mut self, mut offset: usize, value: i32) {
        self.try_expand(offset, 4);
        for b in value.to_le_bytes() {
            self.write_u8_at_unchecked(offset, b);
            offset += 1;
        }
    }

    pub fn write_u32(&mut self, value: u32) {
        self.write_u32_at(self.current_offset, value);
        self.current_offset += 4;
    }

    pub fn write_u32_at(&mut self, mut offset: usize, value: u32) {
        self.try_expand(offset, 4);
        for b in value.to_le_bytes() {
            self.write_u8_at_unchecked(offset, b);
            offset += 1;
        }
    }

    pub fn write_f32(&mut self, value: f32) {
        self.write_f32_at(self.current_offset, value);
        self.current_offset += 4;
    }

    pub fn write_f32_at(&mut self, mut offset: usize, value: f32) {
        self.try_expand(offset, 4);
        for b in value.to_le_bytes() {
            self.write_u8_at_unchecked(offset, b);
            offset += 1;
        }
    }

    pub fn write_f32_vec(&mut self, slice: Vec<f32>) {
        for val in slice {
            self.write_f32(val);
        }
    }

    pub fn write_string(&mut self, str: &str) {
        self.write_string_at(self.current_offset, str);
        self.current_offset += str.len() + 1;
    }

    pub fn write_string_at(&mut self, mut offset: usize, str: &str) {
        self.try_expand(offset, str.len() + 1);
        for b in str.bytes() {
            self.write_u8_at_unchecked(offset, b);
            offset += 1;
        }
        self.write_u8_at_unchecked(offset, 0x0);
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn offset_pos(&self) -> usize {
        self.current_offset
    }

    pub fn max_size(&self) -> usize {
        self.vec.capacity()
    }

    pub fn can_read(&self) -> bool {
        self.current_offset < self.vec.len()
    }

    pub fn read_i8(&mut self) -> i8 {
        let v = self.peek_i8_at(self.current_offset);
        self.current_offset += 1;
        v
    }

    pub fn peek_i8_at(&self, offset: usize) -> i8 {
        self._buf_cursor(offset).read_i8().unwrap()
    }

    pub fn read_u8(&mut self) -> u8 {
        let v = self.peek_u8_at(self.current_offset);
        self.current_offset += 1;
        v
    }

    pub fn peek_u8_at(&self, offset: usize) -> u8 {
        self._buf_cursor(offset).read_u8().unwrap()
    }

    pub fn read_i16(&mut self) -> i16 {
        let val = self.peek_i16_at(self.current_offset);
        self.current_offset += 2;
        val
    }

    pub fn peek_i16_at(&self, offset: usize) -> i16 {
        self._buf_cursor(offset).read_i16::<LittleEndian>().unwrap()
    }

    pub fn peek_u16_at(&self, offset: usize) -> u16 {
        self._buf_cursor(offset).read_u16::<LittleEndian>().unwrap()
    }

    pub fn read_u16(&mut self) -> u16 {
        let val = self.peek_u16_at(self.current_offset);
        self.current_offset += 2;
        val
    }

    pub fn read_i32(&mut self) -> i32 {
        let val = self.peek_i32_at(self.current_offset);
        self.current_offset += 4;
        val
    }

    pub fn peek_i32_at(&self, offset: usize) -> i32 {
        self._buf_cursor(offset).read_i32::<LittleEndian>().unwrap()
    }

    pub fn read_u32(&mut self) -> u32 {
        let val = self.peek_u32_at(self.current_offset);
        self.current_offset += 4;
        val
    }

    pub fn peek_u32_at(&self, offset: usize) -> u32 {
        self._buf_cursor(offset).read_u32::<LittleEndian>().unwrap()
    }

    pub fn read_f32(&mut self) -> f32 {
        let val = self.peek_f32_at(self.current_offset);
        self.current_offset += 4;
        val
    }

    pub fn peek_f32_at(&self, offset: usize) -> f32 {
        self._buf_cursor(offset).read_f32::<LittleEndian>().unwrap()
    }

    pub fn read_f32_vec(&mut self, count: usize) -> Vec<f32> {
        let mut vec = Vec::with_capacity(count);
        for _ in 0..count {
            vec.push(self.read_f32());
        }
        vec
    }

    pub fn read_string(&mut self) -> Result<String, FromBytesUntilNulError> {
        self.peek_string_at(self.current_offset)
    }

    pub fn peek_string_at(&mut self, offset: usize) -> Result<String, FromBytesUntilNulError> {
        let cstr = CStr::from_bytes_until_nul(&self.vec[offset..self.len()])?;
        Ok(String::from_utf8_lossy(cstr.to_bytes()).to_string())
    }

    pub fn get_vec_slice(&self, offset: usize, len: usize) -> &[u8] {
        &self.vec[offset..len]
    }

    pub fn slice(&self, offset: usize, len: usize) -> BitBuffer {
        let v_s = &self.vec[offset..len];
        BitBuffer::from(v_s.to_vec())
    }

    pub fn as_hex_str(&self) -> String {
        let mut s = String::with_capacity(self.len() + 4);
        write!(s,"[{}]0x", self.vec.len()).unwrap();
        for i in 0..self.vec.len() {
            write!(s, "{:02X}", self.vec[i]).unwrap();
        }
        s
    }

    pub fn as_dec_str(&self) -> String {
        let mut s = String::with_capacity(self.len() + 4);
        write!(s,"[{}]", self.vec.len()).unwrap();
        for i in 0..self.vec.len() {
            write!(s, "{} ", self.vec[i]).unwrap();
        }
        s
    }
}

impl Read for BitBuffer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.vec.write(buf)
    }
}

impl Debug for BitBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,"[{}]0x", self.vec.len())?;
        for i in 0..self.vec.len() {
            write!(f, "{:02X}", self.vec[i])?;
        }
        Ok(())
    }
}

impl From<Vec<u8>> for BitBuffer {
    fn from(vec: Vec<u8>) -> Self {
        Self {
            current_offset: 0,
            vec
        }
    }
}
