use std::io::{sink, Result as IoResult, Sink, Write};

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
