use std::io::{self, Read, Seek, SeekFrom};

pub struct PushbackReader<R: Read + Seek> {
    inner: R,
    buffer: Option<u8>,
}

impl<R: Read + Seek> PushbackReader<R> {
    pub fn new(inner: R) -> Self {
        PushbackReader { inner, buffer: None }
    }

    pub fn unread(&mut self, byte: u8) -> io::Result<()> {
        if self.buffer.is_some() {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Pushback buffer already full",
            ))
        } else {
            self.buffer = Some(byte);
            Ok(())
        }
    }
}

impl<R: Read + Seek> Read for PushbackReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut total_read = 0;

        // If there's a byte in the buffer, use it first
        if let Some(byte) = self.buffer.take() {
            buf[0] = byte;
            total_read += 1;
        }

        // Read the remaining bytes from the inner reader
        if buf.len() > total_read {
            match self.inner.read(&mut buf[total_read..]) {
                Ok(n) => total_read += n,
                Err(e) => return Err(e),
            }
        }

        Ok(total_read)
    }
}

impl<R: Read + Seek> Seek for PushbackReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.buffer = None;
        self.inner.seek(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read() {
        let data = b"Hello, world!";
        let mut reader = PushbackReader::new(Cursor::new(data));

        let mut buffer = [0; 5];
        reader.read(&mut buffer).unwrap();
        assert_eq!(&buffer, b"Hello");
    }

    #[test]
    fn test_unread_and_read() {
        let data = b"Hello, world!";
        let mut reader = PushbackReader::new(Cursor::new(data));

        let mut buffer = [0; 5];
        reader.read(&mut buffer).unwrap();
        assert_eq!(&buffer, b"Hello");

        reader.unread(b'H').unwrap();
        let mut buffer = [0; 5];
        reader.read(&mut buffer).unwrap();
        assert_eq!(&buffer, b"H, wo");
    }

    #[test]
    fn test_seek() {
        let data = b"Hello, world!";
        let mut reader = PushbackReader::new(Cursor::new(data));

        reader.seek(SeekFrom::Start(7)).unwrap();
        let mut buffer = [0; 5];
        reader.read(&mut buffer).unwrap();
        assert_eq!(&buffer, b"world");
    }

    #[test]
    fn test_eof() {
        let data = b"Hi";
        let mut reader = PushbackReader::new(Cursor::new(data));

        let mut buffer = [0; 2];
        reader.read(&mut buffer).unwrap();
        assert_eq!(&buffer, b"Hi");

        let mut buffer = [0; 1];
        assert_eq!(reader.read(&mut buffer).unwrap(), 0); // EOF
    }

    #[test]
    fn test_unread_and_eof() {
        let data = b"Hi";
        let mut reader = PushbackReader::new(Cursor::new(data));

        let mut buffer = [0; 2];
        reader.read(&mut buffer).unwrap();
        assert_eq!(&buffer, b"Hi");

        reader.unread(b'i').unwrap();
        let mut buffer = [0; 1];
        reader.read(&mut buffer).unwrap();
        assert_eq!(&buffer, b"i");

        let mut buffer = [0; 1];
        assert_eq!(reader.read(&mut buffer).unwrap(), 0); // EOF
    }
}
