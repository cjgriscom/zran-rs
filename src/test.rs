use std::io::{self, Cursor, Read, Seek, SeekFrom};
use zlib_rs::deflate::compress_slice;
use zlib_rs::deflate::DeflateConfig;
use zlib_rs::ReturnCode;

use crate::reader::SeekableZLibReader;
use crate::types::CompressionMode::*;
use crate::types::CHUNK;
use crate::zran::build_index;

// Fills the provided buffer with pseudorandom bytes based on the given seed
// Duplicates bytes by `step` in a row
fn prng_bytes(seed: u64, bytes: &mut [u8], step: usize) {
    const M: u64 = 2u64.pow(32);
    const A: u64 = 1664525;
    const C: u64 = 1013904223;
    let mut state = seed;
    for chunk in bytes.chunks_mut(4 * step) {
        state = (A * state + C) % M;
        let rand_bytes = state.to_le_bytes();
        for (i, byte) in chunk.iter_mut().enumerate() {
            *byte = rand_bytes[i / step];
        }
    }
}

fn create_data(seed: u64) -> io::Result<Vec<u8>> {
    // Create a compressed vector of random data that's bigger then the zlib block size
    let mut data = vec![0u8; 160000];
    prng_bytes(seed, &mut data, 4);

    Ok(data)
}

fn compress(data: &[u8], window_bits: i32) -> io::Result<Vec<u8>> {
    let config = DeflateConfig {
        window_bits,
        ..DeflateConfig::default()
    };

    let mut output = vec![0u8; 80000];
    // Compress the data
    let (compressed_data, return_code) = compress_slice(&mut output, &data, config);
    assert_eq!(return_code, ReturnCode::Ok);
    let len = compressed_data.len();

    // 0..compressed_data.len() is the compressed data
    output.truncate(len);

    Ok(output)
}

#[test]
fn test_seekable_raw_reader() -> io::Result<()> {
    let data = create_data(12345)?;

    // Create a clone of the last 10 bytes of the data
    let off = data.len() - 10;
    let end = data.len();
    let data_clone_end = data[off..end].to_vec();

    let mut buffer = vec![0; end - off];
    let mut reader = Cursor::new(data);

    reader.seek(SeekFrom::Start(off as u64))?;
    reader.read_exact(&mut buffer)?;

    assert_eq!(buffer, data_clone_end);

    Ok(())
}

fn test_seekable_zlib_reader(span: u64, window_bits: i32) -> io::Result<()> {
    let data = create_data(12345)?;
    let compressed_data = compress(&data, window_bits)?;

    // Create a clone of the last 10 bytes of the data
    let off = data.len() - 10;
    let end = data.len();
    let data_clone_end = data[off..end].to_vec();

    let mut reader = Cursor::new(compressed_data.clone());

    let index = build_index(&mut reader, span)?;

    for point in &index.list {
        println!(
            "inn: {}, out: {}, bits: {}",
            point.inn, point.out, point.bits
        );
        assert_eq!(point.window.len(), 32768);
    }

    let mut seekable_reader = SeekableZLibReader::new(Cursor::new(reader.into_inner()), index);

    seekable_reader.seek(SeekFrom::Start(off as u64))?;

    let mut buffer = vec![0; end - off];

    seekable_reader.read_exact(&mut buffer)?;

    assert_eq!(buffer, data_clone_end);

    Ok(())
}

#[test]
pub fn test_seekable_zlib_reader_raw() -> io::Result<()> {
    let span = CHUNK as u64;
    let window_bits = Raw as i32;
    test_seekable_zlib_reader(span, window_bits)
}

#[test]
pub fn test_seekable_zlib_reader_zlib() -> io::Result<()> {
    let span = CHUNK as u64;
    let window_bits = Zlib as i32;
    test_seekable_zlib_reader(span, window_bits)
}

#[test]
pub fn test_seekable_zlib_reader_gz() -> io::Result<()> {
    let span = CHUNK as u64;
    let window_bits = Gzip as i32;
    test_seekable_zlib_reader(span, window_bits)
}
