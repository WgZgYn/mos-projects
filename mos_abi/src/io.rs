#![allow(unused)]

#[cfg(feature = "alloc")]
use alloc::string::String;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use core::ops::Deref;

use log::info;
use string_num::string_enum;

string_enum! {
    #[derive(Debug)]
    pub enum IOErrorEnum {
        WriteZero,
        UnexpectedEof,
        InvalidInput,
        NotFound,
    }
}

#[derive(Debug)]
pub struct IOError {
    repr: IOErrorEnum,
}

impl IOError {
    pub const WRITE_ALL_EOF: Self = IOError {
        repr: IOErrorEnum::WriteZero,
    };
    pub const READ_EXACT_EOF: Self = IOError {
        repr: IOErrorEnum::UnexpectedEof,
    };
    pub const UNKNOWN: Self = IOError {
        repr: IOErrorEnum::NotFound,
    };
    pub const ILLEGAL_UTF8: Self = IOError {
        repr: IOErrorEnum::InvalidInput,
    };
    fn is_interrupted(&self) -> bool {
        false
    }
    pub fn as_str(&self) -> &'static str {
        self.repr.as_str()
    }
}

pub type IOResult<T> = Result<T, IOError>;

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize>;
    fn write_all(&mut self, mut buf: &[u8]) -> IOResult<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    return Err(IOError::WRITE_ALL_EOF);
                }
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize>;
    fn read_exact(&mut self, mut buf: &mut [u8]) -> IOResult<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => {
                    buf = &mut buf[n..];
                }
                Err(e) => return Err(e),
            }
        }
        if !buf.is_empty() {
            Err(IOError::WRITE_ALL_EOF)
        } else {
            Ok(())
        }
    }

    #[cfg(feature = "alloc")]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> IOResult<usize> {
        let mut batch = [0u8; 32];
        let mut total = 0;

        loop {
            match self.read(&mut batch) {
                Ok(0) => return Ok(total), // EOF
                Ok(n) => {
                    buf.extend_from_slice(&batch[..n]);
                    total += n;
                }
                e => return e,
            }
        }
    }

    #[cfg(feature = "alloc")]
    fn read_to_string(&mut self) -> IOResult<String> {
        let mut buf = Vec::new();
        self.read_to_end(&mut buf)?;
        String::from_utf8(buf).map_err(|_| IOError::ILLEGAL_UTF8)
    }
}

#[cfg(feature = "alloc")]
pub fn read_to_end(mut reader: impl Read) -> IOResult<Vec<u8>> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

pub trait ReadAt {
    fn read_at(&mut self, offset: usize, buf: &mut [u8]) -> IOResult<usize>;
}

pub trait WriteAt {
    fn write_at(&mut self, offset: usize, buf: &[u8]) -> IOResult<usize>;
}

impl<R: Read + ?Sized> Read for &mut R {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        (**self).read(buf)
    }
}
