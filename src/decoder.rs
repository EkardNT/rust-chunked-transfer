// Copyright 2015 The tiny-http Contributors
// Copyright 2015 The rust-chunked-transfer Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use std::io::Result as IoResult;
use std::io::Read;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::fmt;
use std::error::Error;

/// Reads HTTP chunks and sends back real data.
///
/// # Example
///
/// ```
/// use chunked_transfer::Decoder;
/// use std::io::Read;
///
/// let encoded = b"3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n\r\n";
/// let mut decoded = String::new();
///
/// let mut decoder = Decoder::new(encoded as &[u8]);
/// decoder.read_to_string(&mut decoded);
///
/// assert_eq!(decoded, "hello world!!!");
/// ```
pub struct Decoder<R> {
    // where the chunks come from
    source: R,

    // remaining size of the chunk being read
    // none if we are not in a chunk
    remaining_chunks_size: Option<usize>,
}

impl<R> Decoder<R> where R: Read {
    pub fn new(source: R) -> Decoder<R> {
        Decoder {
            source: source,
            remaining_chunks_size: None,
        }
    }
}

impl<R> Read for Decoder<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        // first possibility: we are not in a chunk
        if self.remaining_chunks_size.is_none() {
            // trying the read the chunk size
            let mut chunk_size = Vec::new();

            loop {
                let byte = match self.source.by_ref().bytes().next() {
                    Some(b) => try!(b),
                    None => return Err(IoError::new(ErrorKind::InvalidInput, DecoderError)),
                };

                if byte == b'\r' {
                    break;
                }

                chunk_size.push(byte);
            }

            match self.source.by_ref().bytes().next() {
                Some(Ok(b'\n')) => (),
                _ => return Err(IoError::new(ErrorKind::InvalidInput, DecoderError)),
            }

            let chunk_size = match String::from_utf8(chunk_size) {
                Ok(c) => c,
                Err(_) => return Err(IoError::new(ErrorKind::InvalidInput, DecoderError))
            };

            let chunk_size = match usize::from_str_radix(&chunk_size, 16) {
                Ok(c) => c,
                Err(_) => return Err(IoError::new(ErrorKind::InvalidInput, DecoderError))
            };

            // if the chunk size is 0, we are at EOF
            if chunk_size == 0 {
                if try!(self.source.by_ref().bytes().next().unwrap_or(Ok(0))) != b'\r' {
                    return Err(IoError::new(ErrorKind::InvalidInput, DecoderError));
                }
                if try!(self.source.by_ref().bytes().next().unwrap_or(Ok(0))) != b'\n' {
                    return Err(IoError::new(ErrorKind::InvalidInput, DecoderError));
                }
                return Ok(0);
            }

            // now that we now the current chunk size, calling ourselves recursively
            self.remaining_chunks_size = Some(chunk_size);
            return self.read(buf);
        }

        assert!(self.remaining_chunks_size.is_some());

        // second possibility: we continue reading from a chunk
        if buf.len() < *self.remaining_chunks_size.as_ref().unwrap() {
            let read = try!(self.source.read(buf));
            *self.remaining_chunks_size.as_mut().unwrap() -= read;
            return Ok(read);
        }

        // third possibility: the read request goes further than the current chunk
        // we simply read until the end of the chunk and return
        assert!(buf.len() >= *self.remaining_chunks_size.as_ref().unwrap());

        let remaining_chunks_size = *self.remaining_chunks_size.as_ref().unwrap();

        let buf = &mut buf[.. remaining_chunks_size];
        let read = try!(self.source.read(buf));
        *self.remaining_chunks_size.as_mut().unwrap() -= read;

        if read == remaining_chunks_size {
            self.remaining_chunks_size = None;

            if try!(self.source.by_ref().bytes().next().unwrap_or(Ok(0))) != b'\r' {
                return Err(IoError::new(ErrorKind::InvalidInput, DecoderError));
            }
            if try!(self.source.by_ref().bytes().next().unwrap_or(Ok(0))) != b'\n' {
                return Err(IoError::new(ErrorKind::InvalidInput, DecoderError));
            }
        }

        return Ok(read);
    }
}

#[derive(Debug, Copy, Clone)]
struct DecoderError;

impl fmt::Display for DecoderError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "Error while decoding chunks")
    }
}

impl Error for DecoderError {
    fn description(&self) -> &str {
        "Error while decoding chunks"
    }
}


#[cfg(test)]
mod test {
    use super::Decoder;
    use std::io;
    use std::io::Read;

    #[test]
    fn test_valid_chunk_decode() {
        let source = io::Cursor::new("3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n\r\n".to_string().into_bytes());
        let mut decoded = Decoder::new(source);

        let mut string = String::new();
        decoded.read_to_string(&mut string).unwrap();

        assert_eq!(string, "hello world!!!");
    }

    #[test]
    #[should_panic]
    fn invalid_input1() {
        let source = io::Cursor::new("2\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n".to_string().into_bytes());
        let mut decoded = Decoder::new(source);

        let mut string = String::new();
        decoded.read_to_string(&mut string).unwrap();
    }

    #[test]
    #[should_panic]
    fn invalid_input2() {
        let source = io::Cursor::new("3\rhel\r\nb\r\nlo world!!!\r\n0\r\n".to_string().into_bytes());
        let mut decoded = Decoder::new(source);

        let mut string = String::new();
        decoded.read_to_string(&mut string).unwrap();
    }
}
