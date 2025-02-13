use std::ffi::{CStr, FromBytesUntilNulError};
use std::fmt::{Debug, Formatter};
use std::io::{Cursor};
use std::ops::Range;
use byteorder::{LittleEndian, ReadBytesExt};
use crate::packet::Packet;
use std::fmt::Write;

pub struct BitBuffer {
    cursor: usize,
    // offset: usize, // prob can just use vec.length but for now it works
    vec: Vec<u8>
}

impl BitBuffer {
    pub fn new(size: usize, init_len: Option<usize>) -> Self {
        let mut vec = Vec::with_capacity(size);
        if let Some(len) = init_len {
            unsafe { vec.set_len(len); }
        }
        Self {
            cursor: 0,
            vec
        }
    }


    fn _buf_cursor(&self, offset: usize) -> Cursor<&Vec<u8>> {
        let mut c = Cursor::new(&self.vec);
        c.set_position(offset as u64);
        c
    }

    pub fn as_slice(&self) -> &[u8] {
        self.vec.as_slice()
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.vec
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.vec.as_mut_slice()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.vec[0..self.cursor]
    }

    pub fn set_cursor(&mut self, offset: usize) -> Result<(), String>{
        if offset > self.vec.capacity() {
            return Err(format!("offset {}, is greater than capacity {}", offset, self.vec.capacity()).to_string())
        }
        self.cursor = offset;
        Ok(())
    }

    pub fn set_len(&mut self, len: usize) -> Result<(), String> {
        if len > self.vec.capacity() {
            return Err(format!("len {}, is greater than capacity {}", len, self.vec.capacity()).to_string())
        }
        unsafe { self.vec.set_len(len); }
        Ok(())
    }

    pub fn write_byte(&mut self, value: i8) {
        self.vec.push(value as u8);
        self.cursor += 1;
    }

    pub fn write_byte_at(&mut self, offset: usize, value: i8) {
        self.vec[offset] = value as u8;
    }

    pub fn write_short(&mut self, value: i16) {
        for b in value.to_le_bytes() {
            self.write_byte(b as i8);
        }
    }

    pub fn write_short_at(&mut self, offset: usize, value: i16) {
        let mut offset = offset;
        for b in value.to_le_bytes() {
            self.write_byte_at(offset, b as i8);
            offset += 1;
        }
    }

    pub fn write_int(&mut self, value: i32) {
        for b in value.to_le_bytes() {
            self.write_byte(b as i8);
        }
    }

    pub fn write_int_at(&mut self, offset: usize, value: i32) {
        let mut offset = offset;
        for b in value.to_le_bytes() {
            self.write_byte_at(offset, b as i8);
            offset += 1;
        }
    }

    pub fn write_float(&mut self, value: f32) {
        for b in value.to_le_bytes() {
            self.write_byte(b as i8);
        }
    }

    pub fn write_float_at(&mut self, offset: usize, value: f32) {
        let mut offset = offset;
        for b in value.to_le_bytes() {
            self.write_byte_at(offset, b as i8);
            offset += 1;
        }
    }

    pub fn write_string(&mut self, str: &str) {
        for b in str.bytes() {
            self.write_byte(b as i8);
        }
        self.write_byte(0x0);
    }

    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn max_size(&self) -> usize {
        self.vec.capacity()
    }

    pub fn can_read(&self) -> bool {
        self.cursor < self.vec.len()
    }

    pub fn read_byte(&mut self) -> i8 {
        let v = self.vec[self.cursor] as i8;
        self.cursor += 1;
        v
    }

    pub fn peek_byte_at(&self, offset: usize) -> i8 {
        self.vec[offset] as i8
    }

    pub fn read_short(&mut self) -> i16 {
        let val = self.peek_short_at(self.cursor);
        self.cursor += 2;
        val
    }

    pub fn peek_short_at(&self, offset: usize) -> i16 {
        self._buf_cursor(offset).read_i16::<LittleEndian>().unwrap()
    }

    pub fn read_int(&mut self) -> i32 {
        let val = self.peek_int_at(self.cursor);
        self.cursor += 4;
        val
    }

    pub fn peek_int_at(&self, offset: usize) -> i32 {
        self._buf_cursor(offset).read_i32::<LittleEndian>().unwrap()
    }

    pub fn read_float(&mut self) -> f32 {
        let val = self.peek_float_at(self.cursor);
        self.cursor += 4;
        val
    }

    pub fn peek_float_at(&self, offset: usize) -> f32 {
        self._buf_cursor(offset).read_f32::<LittleEndian>().unwrap()
    }

    pub fn read_string(&mut self) -> Result<String, FromBytesUntilNulError> {
        self.peek_string_at(self.cursor)
    }

    pub fn peek_string_at(&mut self, offset: usize) -> Result<String, FromBytesUntilNulError> {
        println!("{:?}", &self.vec[offset..self.len()]);
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
            cursor: 0,
            vec
        }
    }
}
