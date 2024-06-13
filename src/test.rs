use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use flate2::write::DeflateEncoder;
use flate2::Compression;
use flate2::write::GzEncoder;

use crate::pushback::PushbackReader;
use crate::zran::build_index;
use crate::zran::extract_data;
use crate::reader::SeekableZLibReader;

// const 

const REPEAT_ME: &[u8] = b"Some large data to compress. This will be repeated many times to create a large file. ";
const FINAL_STR: &[u8] = b"Some more stuff to compress. This will terminate after many repetitions a large file. ";

fn create_large_raw_data() -> io::Result<Vec<u8>> {
    
    let mut vec = Vec::new();

    for _ in 0..199999 {
		vec.extend(REPEAT_ME);
    }
	vec.extend(FINAL_STR);

    Ok(vec)
}

fn create_large_gz_data() -> io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());

    for _ in 0..199999 {
		encoder.write_all(REPEAT_ME).unwrap();
    }
	encoder.write_all(FINAL_STR).unwrap();

    let compressed_data = encoder.finish()?;
    Ok(compressed_data)
}


fn create_large_deflate_data() -> io::Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());

    for _ in 0..199999 {
		encoder.write_all(REPEAT_ME).unwrap();
    }
	encoder.write_all(FINAL_STR).unwrap();

    let compressed_data = encoder.finish()?;
    Ok(compressed_data)
}



#[test]
pub fn test_seekable_raw_reader() -> io::Result<()> {
    let data = create_large_raw_data()?;

    let offset = 199999 * 86;

	let mut buffer = vec![0; 86];
	let mut reader = Cursor::new(data);
	
	reader.seek(SeekFrom::Start(offset))?;
	reader.read_exact(&mut buffer)?;

	assert_eq!(buffer, FINAL_STR);

    Ok(())
}

#[test]
pub fn test_seekable_zlib_reader_deflate() -> io::Result<()> {
    let span = 1048576;

    let compressed_data = create_large_deflate_data()?;
	
    let mut reader = Cursor::new(compressed_data.clone());

	let index = build_index(&mut reader, span)?;

    let offset = 199999 * 86;
	
    let mut seekable_reader = SeekableZLibReader::new(Cursor::new(reader.into_inner()), index);

    seekable_reader.seek(SeekFrom::Start(offset))?;
	
	let mut buffer = vec![0; 86];

    seekable_reader.read_exact(&mut buffer)?;

	assert_eq!(buffer, FINAL_STR);

    Ok(())
}

#[test]
pub fn test_seekable_zlib_reader_gz() -> io::Result<()> {
    let span = 1048576;

    let compressed_data = create_large_gz_data()?;
	
    let mut reader = Cursor::new(compressed_data.clone());

	let index = build_index(&mut reader, span)?;

    let offset = 199999 * 86;
	
    let mut seekable_reader = SeekableZLibReader::new(Cursor::new(reader.into_inner()), index);

    seekable_reader.seek(SeekFrom::Start(offset))?;
	
	let mut buffer = vec![0; 86];

    seekable_reader.read_exact(&mut buffer)?;

	assert_eq!(buffer, FINAL_STR);

    Ok(())
}


#[test]
pub fn test_extract_data_zlib_reader() -> io::Result<()> {
    let span = 1048576;

    let compressed_data = create_large_gz_data()?;

    let offset = 199999 * 86;

	let mut buffer = vec![0; 86];
	let mut reader = PushbackReader::new(Cursor::new(compressed_data));
	let index = build_index(&mut reader, span)?;

    // Print the index
   // eprintln!("{:?}", index);
    for point in &index.list {
        eprintln!("{} {} {} {}", point.inn, point.out, point.bits, point.window.len());
        
    }
	let bytes_read = extract_data(&mut reader, &index, offset, &mut buffer).unwrap();

	assert_eq!(bytes_read, FINAL_STR.len());
	assert_eq!(buffer, FINAL_STR);

    Ok(())
}