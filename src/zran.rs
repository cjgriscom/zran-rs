use std::ffi::{CStr, CString};
use std::io::{self, Read, Seek, SeekFrom};
use std::mem;

use once_cell::sync::Lazy;
use crate::pushback::PushbackReader;
use crate::types::{CompressionMode, DeflateIndex, WINSIZE, CHUNK};
use crate::zlib::*;

static ZLIB_VERSION: Lazy<String> = Lazy::new(|| {
    unsafe {
        // Call the FFI function to get the zlib version as a C string
        let c_str = zlibVersion();
        if !c_str.is_null() {
            // Convert the C string to a Rust String
            CStr::from_ptr(c_str).to_string_lossy().into_owned()
        } else {
            // Handle the case where the C string pointer is null
            "unknown".to_string()
        }
    }
});

fn fread<R: Read>(reader: &mut R, buffer: &mut [u8], length: usize) -> io::Result<usize> {
    let mut total_read = 0;
    while total_read < length {
        match reader.read(&mut buffer[total_read..length])? {
            0 => break,
            n => total_read += n,
        }
    }
    Ok(total_read)
}

pub fn build_index<R: Read + Seek>(reader: &mut R, span: u64) -> io::Result<DeflateIndex> {
    let mut in_stream = PushbackReader::new(reader);
    let mut stream = ZStream::new();
    let mut buffer = vec![0; CHUNK];
    let mut win = vec![0; WINSIZE]; // output sliding window
    let mut totin = 0u64;               // total bytes read from input
    let mut totout = 0u64;              // total bytes uncompressed
    let mut mode = 0;         // mode: RAW, ZLIB, or GZIP (0 => not set yet)
    let mut last = 0u64;  // last access point uncompressed offset
    
    // list of access points
    let mut index = DeflateIndex::new();

    unsafe {
		// Decompress from reader, generating access points along the way.
        let mut ret = Z_OK;  // the return value from zlib, or Z_ERRNO
        loop {
			// Assure available input, at least until reaching EOF.
            if stream.avail_in == 0 {
                stream.avail_in = fread(&mut in_stream, &mut buffer, CHUNK)? as u32;
                totin += stream.avail_in as u64;
                stream.next_in = buffer.as_mut_ptr();
            }

            if mode == 0 {
                // At the start of the input -- determine the type. Assume raw
                // if it is neither zlib nor gzip. This could in theory result
                // in a false positive for zlib, but in practice the fill bits
                // after a stored block are always zeros, so a raw stream won't
                // start with an 8 in the low nybble.
                mode = match stream.avail_in {
                    0 => CompressionMode::Raw as i32, // empty -- will fail
                    _ if (*stream.next_in & 0xf) == 8 => CompressionMode::Zlib as i32,
                    _ if *stream.next_in == 0x1f => CompressionMode::Gzip as i32,
                    _ => CompressionMode::Raw as i32,
                };
                
                let version_cstr = CString::new(ZLIB_VERSION.as_str()).expect("CString::new failed");

                ret = inflateInit2_(
                    &mut stream,
                    mode,
                    version_cstr.as_ptr(),
                    mem::size_of::<ZStream>() as i32,
                );
                if ret != Z_OK {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("inflateInit2_ error: {}", zlib_error_description(ret)),
                    ));
                }
            }

			// Assure available output. This rotates the output through, for use as
			// a sliding window on the uncompressed data.
            if stream.avail_out == 0 {
                stream.avail_out = WINSIZE as u32;
                stream.next_out = win.as_mut_ptr();
            }

            if mode == CompressionMode::Raw as i32 && index.list.is_empty() {
				// We skip the inflate() call at the start of raw deflate data in
				// order generate an access point there. Set data_type to imitate
				// the end of a header.
                stream.data_type = 0x80;
            } else {
				// Inflate and update the number of uncompressed bytes.
                let before = stream.avail_out;
                ret = inflate(&mut stream, Z_BLOCK);
                totout += (before - stream.avail_out) as u64;
            }
            
            if (stream.data_type & 0xc0) == 0x80 && (index.list.is_empty() || totout - last >= span) {

				// We are at the end of a header or a non-last deflate block, so we
				// can add an access point here. Furthermore, we are either at the
				// very start for the first access point, or there has been span or
				// more uncompressed bytes since the last access point, so we want
				// to add an access point here.

                index.add_point(
                    stream.data_type as u32 & 7,
                    totin - stream.avail_in as u64,
                    totout,
                    stream.avail_out as usize,
                    &win,
                );
                last = totout;
            }

            if ret == Z_STREAM_END && mode == CompressionMode::Gzip as i32
                && (stream.avail_in != 0 || !is_eof(&mut in_stream)?){
                
				// There is more input after the end of a gzip member. Reset the
				// inflate state to read another gzip member. On success, this will
				// set ret to Z_OK to continue decompressing.
                ret = inflateReset2(&mut stream, CompressionMode::Gzip as i32);
            }

			// Keep going until Z_STREAM_END or error. If the compressed data ends
			// prematurely without a file read error, Z_BUF_ERROR is returned.
            if ret != Z_OK {
                break;
            }
        }

        inflateEnd(&mut stream);

        if ret != Z_STREAM_END {
			// An error was encountered. Discard the index and return a negative
			// error code
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("zlib error: {}", zlib_error_description(ret)),
            ));
        }

        index.mode = mode;
        index.length = totout;
    }

    Ok(index)
}

pub fn extract_data<R: Read + Seek>(
    reader: &mut PushbackReader<R>,
    index: &DeflateIndex,
    offset: u64,
    buffer: &mut [u8],
) -> io::Result<usize> {

    // Do a quick check on the index
    if index.list.is_empty() || index.list[0].out != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid index"));
    }

	// If nothing to extract, return zero bytes extracted
    if offset >= index.length {
        return Ok(0);
    }

	// Find the access point closest to but not after offset
    let mut lo = -1;
    let mut hi = index.list.len() as isize;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if offset < index.list[mid as usize].out {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let point = &index.list[lo as usize];
    let mut stream = ZStream::new();

    unsafe {
        let seek_offset = point.inn - (if point.bits != 0 { 1 } else { 0 }) as u64;
        reader.seek(SeekFrom::Start(seek_offset))?;

        let ch = if point.bits != 0 {
            let mut byte = [0u8];
            reader.read_exact(&mut byte)?;
            byte[0] as i32
        } else {
            0
        };

        let version_cstr = CString::new(ZLIB_VERSION.as_str()).expect("CString::new failed");

        let ret = inflateInit2_(
            &mut stream,
            CompressionMode::Raw as i32,
            version_cstr.as_ptr(),
            mem::size_of::<ZStream>() as i32,
        );

        if ret != Z_OK {
            return Err(io::Error::new(io::ErrorKind::Other, format!("inflateInit2_ error: {}", zlib_error_description(ret))));
        }

        if point.bits != 0 {
            inflatePrime(&mut stream, point.bits as i32, ch >> (8 - point.bits as i32));
            inflateSetDictionary(&mut stream, point.window.as_ptr(), WINSIZE as i32);
        }

		// Skip uncompressed bytes until offset reached, then satisfy request.
        let mut input_buffer = vec![0; CHUNK];
        let mut discard_buffer = vec![0; WINSIZE];

        let mut offset = offset - point.out; // number of bytes to skip to get to offset
        let mut left = buffer.len();       // number of bytes left to read after offset

        loop {
            if offset != 0 {
				// Discard up to offset uncompressed bytes
                stream.avail_out = (if offset < WINSIZE as u64 { offset } else { WINSIZE as u64 }) as u32;
                stream.next_out = discard_buffer.as_mut_ptr();
            } else {
				// Uncompress up to left bytes into buf
                stream.avail_out = (if left < u32::MAX as usize { left } else { u32::MAX as usize }) as u32;
                stream.next_out = buffer.as_mut_ptr().add(buffer.len() - left);
            }

			// Uncompress, setting got to the number of bytes uncompressed
            if stream.avail_in == 0 {
				// Assure available input.
                stream.avail_in = fread(reader, &mut input_buffer, CHUNK)? as u32;
                stream.next_in = input_buffer.as_mut_ptr();
            }

            let before = stream.avail_out;
            let ret = inflate(&mut stream, Z_NO_FLUSH);
            let got = before - stream.avail_out;

			// Update the appropriate count
            if offset != 0 {
                offset -= got as u64;
            } else {
                left -= got as usize;
            }

			// If we're at the end of a gzip member and there's more to read,
			// continue to the next gzip member.
            if ret == Z_STREAM_END && index.mode == CompressionMode::Gzip as i32 {
                // Discard the gzip trailer
                let mut drop = 8; // length of gzip trailer
                if stream.avail_in >= drop as u32 {
                    stream.avail_in -= drop as u32;
                    stream.next_in = stream.next_in.add(drop);
                } else {
                    drop -= stream.avail_in as usize;
                    stream.avail_in = 0;
                    let mut discard = vec![0; drop];
                    reader.read_exact(&mut discard)?;
                }

                if stream.avail_in != 0 || !is_eof(reader)? {
					// There's more after the gzip trailer. Use inflate to skip the
					// gzip header and resume the raw inflate there.
                    inflateReset2(&mut stream, CompressionMode::Gzip as i32);
                    loop {
                        if stream.avail_in == 0 {
                            stream.avail_in = fread(reader, &mut input_buffer, CHUNK)? as u32;
                            stream.next_in = input_buffer.as_mut_ptr();
                        }
                        stream.avail_out = WINSIZE as u32;
                        stream.next_out = discard_buffer.as_mut_ptr();
                        let ret = inflate(&mut stream, Z_BLOCK);
                        if ret != Z_OK || (stream.data_type & 0x80) != 0 {
                            break;
                        }
                    }
                    if ret != Z_OK {
                        break;
                    }
                    inflateReset2(&mut stream, CompressionMode::Raw as i32);
                }
            }

			// Continue until we have the requested data, the deflate data has
			// ended, or an error is encountered.
            if !(ret == Z_OK && left != 0) {
                break;
            }
        }
        inflateEnd(&mut stream);
        
		// Return the number of uncompressed bytes read into buf, or the error.
        match ret {
            Z_OK | Z_STREAM_END => Ok(buffer.len() - left),
            _ => Err(io::Error::new(io::ErrorKind::Other, format!("inflate error: {}", zlib_error_description(ret)))),
        }
    }
}

fn is_eof<R: Read + Seek>(reader: &mut PushbackReader<R>) -> io::Result<bool> {
    let mut buf = [0; 1];
    match reader.read(&mut buf) {
        Ok(0) => Ok(true), // EOF reached
        Ok(1) => {
            reader.unread(buf[0])?; // Push the byte back into the buffer
            Ok(false)
        }
        Ok(_) => unreachable!(), // We only read 1 byte, so this should never be hit
        Err(e) => Err(e),
    }
}