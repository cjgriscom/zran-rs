use byteorder::BigEndian;
use byteorder::WriteBytesExt;
use std::io::Write;

pub const WINSIZE: usize = 32768;
pub const CHUNK: usize = 16384;

pub enum CompressionMode {
    Raw = -15,
    Zlib = 15,
    Gzip = 31,
}

#[derive(Debug, Clone, Default)]
pub struct Point {
    pub inn: u64,
    pub out: u64,
    pub bits: u32,
    pub window: Vec<u8>,
}

impl Point {
    pub fn new() -> Self {
        Self {
            inn: 0,
            out: 0,
            bits: 0,
            window: vec![0; WINSIZE],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeflateIndex {
    pub mode: i32,
    pub list: Vec<Point>,
    pub length: u64,
}

impl DeflateIndex {
    pub fn new() -> Self {
        Self {
            mode: 0,
            list: vec![],
            length: 0,
        }
    }

    pub fn add_point(&mut self, bits: u32, inn: u64, out: u64, left: usize, window: &[u8]) {
        let mut point = Point::new();
        point.inn = inn;
        point.out = out;
        point.bits = bits;

        if left > 0 {
            let end = WINSIZE - left;
            point.window[..left].copy_from_slice(&window[end..]);
        }
        if left < WINSIZE {
            point.window[left..].copy_from_slice(&window[..WINSIZE - left]);
        }

        self.list.push(point);
    }

    pub fn serialize(&self, writer: &mut dyn Write) -> std::io::Result<()> {
        writer.write_u64::<BigEndian>(self.length)?;
        writer.write_i32::<BigEndian>(self.mode)?;
        writer.write_i32::<BigEndian>(self.list.len() as i32)?;

        for point in &self.list {
            writer.write_u64::<BigEndian>(point.inn)?;
            writer.write_u64::<BigEndian>(point.out)?;
            writer.write_u32::<BigEndian>(point.bits)?;
            writer.write_all(&point.window)?;
        }

        Ok(())
    }
}
