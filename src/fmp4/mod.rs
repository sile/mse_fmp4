use std::fmt;
use std::io::Write;
use std::str;
use byteorder::{BigEndian, WriteBytesExt};

use {ErrorKind, Result};
use io::{ByteCounter, WriteTo};

macro_rules! write_u8 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::WriteBytesExt;
            track_io!($w.write_u8($n))?;
        }
    }
}
macro_rules! write_u16 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_u16::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_i16 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_i16::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_u24 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_uint::<BigEndian>($n as u64, 3))?;
        }
    }
}
macro_rules! write_u32 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_u32::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_i32 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_i32::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_u64 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_u64::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_all {
    ($w:expr, $n:expr) => { track_io!($w.write_all($n))?; }
}
macro_rules! write_zeroes {
    ($w:expr, $n:expr) => { track_io!($w.write_all(&[0;$n][..]))?; }
}
macro_rules! write_box {
    ($w:expr, $b:expr) => { track!($b.write_box_to(&mut $w))?; }
}
macro_rules! write_boxes {
    ($w:expr, $bs:expr) => {
        for b in $bs {
            track!(b.write_box_to(&mut $w))?;
        }
    }
}

pub mod initialization;
pub mod media;

pub trait WriteBoxTo: WriteTo {
    fn box_type(&self) -> BoxType;
    fn box_size(&self) -> u32 {
        let mut writer = ByteCounter::with_sink();
        track_try_unwrap!(self.write_to(&mut writer));

        let mut size = 8 + writer.count() as u32;
        if self.full_box_header().is_some() {
            size += 4;
        }
        size
    }
    fn box_header(&self) -> BoxHeader {
        BoxHeader {
            kind: self.box_type(),
            size: self.box_size(),
        }
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        None
    }
    fn write_box_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track!(self.box_header().write_to(&mut writer))?;
        if let Some(x) = self.full_box_header() {
            track!(x.write_to(&mut writer))?;
        }
        track!(self.write_to(writer))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoxHeader {
    pub size: u32,
    pub kind: BoxType,
}
impl BoxHeader {
    const SIZE: u32 = 8;

    pub fn data_size(&self) -> u32 {
        self.size - Self::SIZE
    }
}
impl WriteTo for BoxHeader {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert_ne!(self.size, 1, ErrorKind::Unsupported);
        track_assert_ne!(self.size, 0, ErrorKind::Unsupported);
        track_assert!(self.size >= Self::SIZE, ErrorKind::InvalidInput);
        track_assert_ne!(self.kind.0, *b"uuid", ErrorKind::Unsupported);

        track_io!(writer.write_u32::<BigEndian>(self.size))?;
        track_io!(writer.write_all(&self.kind.0))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FullBoxHeader {
    pub version: u8,
    pub flags: u32, // u24
}
impl FullBoxHeader {
    pub fn new(version: u8, flags: u32) -> Self {
        FullBoxHeader { version, flags }
    }
}
impl WriteTo for FullBoxHeader {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_io!(writer.write_u8(self.version))?;
        track_io!(writer.write_uint::<BigEndian>(u64::from(self.flags), 3))?;
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoxType(pub [u8; 4]);
impl fmt::Debug for BoxType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(s) = str::from_utf8(&self.0) {
            write!(f, "BoxType(b{:?})", s)
        } else {
            write!(f, "BoxType({:?})", self.0)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Brand(pub [u8; 4]);
impl fmt::Debug for Brand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(s) = str::from_utf8(&self.0) {
            write!(f, "Brand(b{:?})", s)
        } else {
            write!(f, "Brand({:?})", self.0)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SampleFormat(pub [u8; 4]);
impl fmt::Debug for SampleFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(s) = str::from_utf8(&self.0) {
            write!(f, "SampleFormat(b{:?})", s)
        } else {
            write!(f, "SampleFormat({:?})", self.0)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HandlerType(pub [u8; 4]);
impl fmt::Debug for HandlerType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(s) = str::from_utf8(&self.0) {
            write!(f, "HandlerType(b{:?})", s)
        } else {
            write!(f, "HandlerType({:?})", self.0)
        }
    }
}
