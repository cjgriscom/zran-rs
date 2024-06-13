extern crate libc;

use libc::{c_void, c_char, c_int, c_uint, c_ulong};

pub const Z_OK: c_int = 0;
pub const Z_STREAM_END: c_int = 1;
pub const Z_NEED_DICT: c_int = 2;
pub const Z_ERRNO: c_int = -1;
pub const Z_STREAM_ERROR: c_int = -2;
pub const Z_DATA_ERROR: c_int = -3;
pub const Z_MEM_ERROR: c_int = -4;
pub const Z_BUF_ERROR: c_int = -5;
pub const Z_BLOCK: c_int = 5;

pub const Z_NO_FLUSH: c_int = 0;

pub fn zlib_error_description(error_code: i32) -> &'static str {
    match error_code {
        Z_OK => "No error",
        Z_STREAM_END => "End of stream",
        Z_NEED_DICT => "Dictionary needed",
        Z_ERRNO => "File error",
        Z_STREAM_ERROR => "Stream error",
        Z_DATA_ERROR => "Data error",
        Z_MEM_ERROR => "Memory error",
        Z_BUF_ERROR => "Buffer error",
        Z_BLOCK => "Block error",
        _ => "Unknown error",
    }
}

#[link(name = "z")]
extern "C" {
    pub fn zlibVersion() -> *const c_char;

    pub fn inflateInit2_(strm: *mut ZStream, window_bits: c_int, version: *const c_char, stream_size: c_int) -> c_int;
    pub fn inflate(strm: *mut ZStream, flush: c_int) -> c_int;
    pub fn inflateReset2(strm: *mut ZStream, window_bits: c_int) -> c_int;
    pub fn inflateEnd(strm: *mut ZStream) -> c_int;
    pub fn inflateSetDictionary(strm: *mut ZStream, dictionary: *const u8, dict_length: c_int) -> c_int;
    pub fn inflatePrime(strm: *mut ZStream, bits: c_int, value: c_int) -> c_int;
}

#[repr(C)]
pub struct ZStream {
    pub next_in: *mut u8,
    pub avail_in: c_uint,
    pub total_in: c_ulong,

    pub next_out: *mut u8,
    pub avail_out: c_uint,
    pub total_out: c_ulong,

    pub msg: *mut c_char,
    pub state: *mut c_void,

    pub zalloc: Option<extern "C" fn(*mut c_void, c_uint, c_uint) -> *mut c_void>,
    pub zfree: Option<extern "C" fn(*mut c_void, *mut c_void)>,
    pub opaque: *mut c_void,

    pub data_type: c_int,
    pub adler: c_ulong,
    pub reserved: c_ulong,
}

impl ZStream {
    pub fn new() -> Self {
        Self {
            next_in: std::ptr::null_mut(),
            avail_in: 0,
            total_in: 0,
            next_out: std::ptr::null_mut(),
            avail_out: 0,
            total_out: 0,
            msg: std::ptr::null_mut(),
            state: std::ptr::null_mut(),
            zalloc: None,
            zfree: None,
            opaque: std::ptr::null_mut(),
            data_type: 0,
            adler: 0,
            reserved: 0,
        }
    }
}