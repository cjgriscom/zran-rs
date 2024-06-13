use std::io::{self, Read, Seek, SeekFrom};
use crate::pushback::PushbackReader;
use crate::types::*;
use crate::zran::extract_data;

pub struct SeekableZLibReader<R: Read + Seek> {
    reader: PushbackReader<R>,
    index: DeflateIndex,
    current_offset: u64,
    buffer: Vec<u8>,
    buffer_pos: usize,
    buffer_size: usize,
}

impl<R: Read + Seek> SeekableZLibReader<R> {
    pub fn new(reader: R, index: DeflateIndex) -> Self {
        Self {
            reader: PushbackReader::new(reader),
            index,
            current_offset: 0,
            buffer: vec![0; CHUNK],
            buffer_pos: 0,
            buffer_size: 0,
        }
    }

    fn fill_buffer(&mut self) -> io::Result<()> {
        self.buffer_pos = 0;
        self.buffer_size = extract_data(
            &mut self.reader,
            &self.index,
            self.current_offset,
            &mut self.buffer,
        )?;
        Ok(())
    }
}

impl<R: Read + Seek> Read for SeekableZLibReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.buffer_pos >= self.buffer_size {
            self.fill_buffer()?;
        }

        let available = self.buffer_size - self.buffer_pos;
        let to_copy = std::cmp::min(buf.len(), available);
        buf[..to_copy].copy_from_slice(&self.buffer[self.buffer_pos..self.buffer_pos + to_copy]);
        self.buffer_pos += to_copy;
        self.current_offset += to_copy as u64;

        Ok(to_copy)
    }
}

impl<R: Read + Seek> Seek for SeekableZLibReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.current_offset = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                if offset >= 0 {
                    self.index.length
                } else {
                    self.index.length - (-offset) as u64
                }
            }
            SeekFrom::Current(offset) => {
                if offset >= 0 {
                    self.current_offset + offset as u64
                } else {
                    self.current_offset - (-offset) as u64
                }
            }
        };
        self.buffer_pos = 0;
        self.buffer_size = 0; // Invalidate the buffer
        Ok(self.current_offset)
    }
}