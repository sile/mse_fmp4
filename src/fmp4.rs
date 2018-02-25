use std::io::{self, Write};
use byteorder::{BigEndian, WriteBytesExt};

use Result;
use isobmff::{BoxHeader, BoxType, Brand, FullBoxHeader};

// macro_rules! write_u8 {
//     ($w:expr, $n:expr) => { track_io!($w.write_u8($n))?; }
// }
// macro_rules! write_u16 {
//     ($w:expr, $n:expr) => { track_io!($w.write_u16::<BigEndian>($n))?; }
// }
macro_rules! write_i16 {
    ($w:expr, $n:expr) => { track_io!($w.write_i16::<BigEndian>($n))?; }
}
macro_rules! write_u32 {
    ($w:expr, $n:expr) => { track_io!($w.write_u32::<BigEndian>($n))?; }
}
macro_rules! write_i32 {
    ($w:expr, $n:expr) => { track_io!($w.write_i32::<BigEndian>($n))?; }
}
// macro_rules! write_u64 {
//     ($w:expr, $n:expr) => { track_io!($w.write_u64::<BigEndian>($n))?; }
// }
macro_rules! write_all {
    ($w:expr, $n:expr) => { track_io!($w.write_all($n))?; }
}
macro_rules! write_zeroes {
    ($w:expr, $n:expr) => { track_io!($w.write_all(&[0;$n][..]))?; }
}
macro_rules! write_box {
    ($w:expr, $b:expr) => { track!($b.write_box_to(&mut $w))?; }
}

#[derive(Debug)]
pub struct WriteBytesCounter(u64);
impl WriteBytesCounter {
    pub fn new() -> Self {
        WriteBytesCounter(0)
    }
    pub fn count(&self) -> u64 {
        self.0
    }
}
impl Write for WriteBytesCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub trait WriteTo {
    fn write_to<W: Write>(&self, writer: W) -> Result<()>;
}

pub trait WriteBoxTo: WriteTo {
    fn box_type(&self) -> BoxType;
    fn box_header(&self) -> BoxHeader {
        let mut writer = WriteBytesCounter::new();
        track_try_unwrap!(self.write_to(&mut writer));

        let mut size = 8 + writer.count() as u32;
        if self.full_box_header().is_some() {
            size += 4;
        }
        BoxHeader {
            kind: self.box_type(),
            size,
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

#[derive(Debug)]
pub struct File {
    pub ftyp_box: FileTypeBox,
    pub moov_box: MovieBox,
    // pub mdat_boxes: Vec<MediaDataBox>,
    // pub moof_boxes: Vec<MoofBox>,
    // pub mfra_box: Option<MfraBox>,
}
impl File {
    pub fn new() -> File {
        File {
            ftyp_box: FileTypeBox::default(),
            moov_box: MovieBox::new(),
        }
    }
}
impl WriteTo for File {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track!(self.ftyp_box.write_box_to(&mut writer))?;
        track!(self.moov_box.write_box_to(&mut writer))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieBox {
    pub mvhd_box: MovieHeaderBox,
}
impl MovieBox {
    pub fn new() -> Self {
        MovieBox {
            mvhd_box: MovieHeaderBox::new(),
        }
    }
}
impl WriteBoxTo for MovieBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"moov")
    }
}
impl WriteTo for MovieBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.mvhd_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieHeaderBox {
    pub creation_time: u32,
    pub modification_time: u32,
    pub timescale: u32,
    pub duration: u32,
    pub rate: i32,   // fixed point 16.16
    pub volume: i16, // fixed point 8.8
    pub matrix: [i32; 9],
    pub next_track_id: u32, // 0xFFFF_FFFF means ...
}
impl MovieHeaderBox {
    pub fn new() -> Self {
        MovieHeaderBox {
            creation_time: 0,
            modification_time: 0,
            timescale: 1, // TODO
            duration: 1,  // TODO
            rate: 65536,
            volume: 256,
            matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
            next_track_id: 0xFFFF_FFFF,
        }
    }
}
impl WriteBoxTo for MovieHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mvhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for MovieHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.creation_time);
        write_u32!(writer, self.modification_time);
        write_u32!(writer, self.timescale);
        write_u32!(writer, self.duration);
        write_i32!(writer, self.rate);
        write_i16!(writer, self.volume);
        write_zeroes!(writer, 2);
        write_zeroes!(writer, 4 * 2);
        for &x in &self.matrix {
            write_i32!(writer, x);
        }
        write_zeroes!(writer, 4 * 6);
        write_u32!(writer, self.next_track_id);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileTypeBox {
    pub major_brand: Brand,
    pub minor_version: u32,
    pub compatible_brands: Vec<Brand>,
}
impl WriteBoxTo for FileTypeBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"ftyp")
    }
}
impl WriteTo for FileTypeBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_all!(writer, &self.major_brand.0);
        write_u32!(writer, self.minor_version);
        for brand in &self.compatible_brands {
            write_all!(writer, &brand.0);
        }
        Ok(())
    }
}
impl Default for FileTypeBox {
    fn default() -> Self {
        FileTypeBox {
            major_brand: Brand(*b"isom"),
            minor_version: 512,
            compatible_brands: Vec::new(),
        }
    }
}
