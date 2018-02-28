use std::io::{sink, Read, Result as IoResult, Sink, Write};
use byteorder::ReadBytesExt;

use Result;

pub trait WriteTo {
    fn write_to<W: Write>(&self, writer: W) -> Result<()>;
}

#[derive(Debug)]
pub struct ByteCounter<T> {
    inner: T,
    count: u64,
}
impl<T> ByteCounter<T> {
    pub fn new(inner: T) -> Self {
        ByteCounter { inner, count: 0 }
    }
    pub fn into_inner(self) -> T {
        self.inner
    }
    pub fn count(&self) -> u64 {
        self.count
    }
}
impl ByteCounter<Sink> {
    pub fn with_sink() -> Self {
        Self::new(sink())
    }
}
impl<T: Write> Write for ByteCounter<T> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let size = self.inner.write(buf)?;
        self.count += size as u64;
        Ok(size)
    }
    fn flush(&mut self) -> IoResult<()> {
        self.inner.flush()
    }
}

#[derive(Debug)]
pub(crate) struct AvcBitReader<R> {
    stream: R,
    byte: u8,
    bit_offset: usize,
}
impl<R: Read> AvcBitReader<R> {
    pub fn new(stream: R) -> Self {
        AvcBitReader {
            stream,
            byte: 0,
            bit_offset: 8,
        }
    }

    pub fn read_bit(&mut self) -> Result<u8> {
        if self.bit_offset == 8 {
            self.byte = track_io!(self.stream.read_u8())?;
            self.bit_offset = 0;
        }
        let bit = (self.byte >> (7 - self.bit_offset)) & 0b1;
        self.bit_offset += 1;
        Ok(bit)
    }

    pub fn read_ue(&mut self) -> Result<u64> {
        track!(self.read_exp_golomb_code())
    }

    fn read_exp_golomb_code(&mut self) -> Result<u64> {
        let mut leading_zeros = 0;
        while 0 == track!(self.read_bit())? {
            leading_zeros += 1;
        }
        let mut n = 0;
        for _ in 0..leading_zeros {
            let bit = track!(self.read_bit())?;
            n = (n << 1) | u64::from(bit);
        }
        n += 2u64.pow(leading_zeros) - 1;
        Ok(n)
    }
}
