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

#[derive(Debug)]
pub struct MediaSegment {
    pub moof_box: MovieFragmentBox,
    pub mdat_boxes: Vec<MediaDataBox>,
}
impl MediaSegment {
    pub fn new() -> Self {
        MediaSegment {
            moof_box: MovieFragmentBox::new(),
            mdat_boxes: Vec::new(),
        }
    }
}
impl WriteTo for MediaSegment {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.mdat_boxes.is_empty(), ErrorKind::InvalidInput);
        write_box!(writer, self.moof_box);
        write_boxes!(writer, &self.mdat_boxes);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MediaDataBox {
    pub data: Vec<u8>,
}
impl WriteBoxTo for MediaDataBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mdat")
    }
}
impl WriteTo for MediaDataBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_all!(writer, &self.data);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieFragmentBox {
    pub mfhd_box: MovieFragmentHeaderBox,
    pub traf_boxes: Vec<TrackFragmentBox>,
}
impl MovieFragmentBox {
    pub fn new() -> Self {
        MovieFragmentBox {
            mfhd_box: MovieFragmentHeaderBox::new(),
            traf_boxes: Vec::new(),
        }
    }
}
impl WriteBoxTo for MovieFragmentBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"moof")
    }
}
impl WriteTo for MovieFragmentBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.traf_boxes.is_empty(), ErrorKind::InvalidInput);
        write_box!(writer, self.mfhd_box);
        write_boxes!(writer, &self.traf_boxes);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrackFragmentBox {
    pub tfhd_box: TrackFragmentHeaderBox,
    pub tfdt_box: TrackFragmentBaseMediaDecodeTimeBox,
    pub trun_box: TrackRunBox,
}
impl TrackFragmentBox {
    pub fn new(track_id: u32) -> Self {
        TrackFragmentBox {
            tfhd_box: TrackFragmentHeaderBox::new(track_id),
            tfdt_box: TrackFragmentBaseMediaDecodeTimeBox::new(),
            trun_box: TrackRunBox::new(),
        }
    }
}
impl WriteBoxTo for TrackFragmentBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"traf")
    }
}
impl WriteTo for TrackFragmentBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.tfhd_box);
        write_box!(writer, self.tfdt_box);
        write_box!(writer, self.trun_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieFragmentHeaderBox {
    pub sequence_number: u32,
}
impl MovieFragmentHeaderBox {
    pub fn new() -> Self {
        MovieFragmentHeaderBox { sequence_number: 1 }
    }
}
impl WriteBoxTo for MovieFragmentHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mfhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for MovieFragmentHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert_ne!(self.sequence_number, 0, ErrorKind::InvalidInput);
        write_u32!(writer, self.sequence_number);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrackRunBox {
    pub data_offset: Option<i32>,
    pub first_sample_flags: Option<u32>,
    pub entries: Vec<TrunEntry>,
}
impl TrackRunBox {
    pub fn new() -> Self {
        TrackRunBox {
            data_offset: None,
            first_sample_flags: None,
            entries: Vec::new(),
        }
    }
}
impl WriteBoxTo for TrackRunBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"trun")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        let head = self.entries.first().unwrap_or_else(|| &TrunEntry {
            sample_duration: None,
            sample_size: None,
            sample_flags: None,
            sample_composition_time_offset: None,
        });
        let flags = (self.data_offset.is_some() as u32 * 0x00_0001)
            | (self.first_sample_flags.is_some() as u32 * 0x00_0004)
            | (head.sample_duration.is_some() as u32 * 0x00_0100)
            | (head.sample_size.is_some() as u32 * 0x00_0200)
            | (head.sample_flags.is_some() as u32 * 0x00_0400)
            | (head.sample_composition_time_offset.is_some() as u32 * 0x00_0800);
        Some(FullBoxHeader::new(1, flags))
    }
}
impl WriteTo for TrackRunBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.entries.len() as u32);
        if let Some(x) = self.data_offset {
            write_i32!(writer, x);
        }
        if let Some(x) = self.first_sample_flags {
            write_u32!(writer, x);
        }
        for e in &self.entries {
            // TODO: check flags
            if let Some(x) = e.sample_duration {
                write_u32!(writer, x);
            }
            if let Some(x) = e.sample_size {
                write_u32!(writer, x);
            }
            if let Some(x) = e.sample_flags {
                write_u32!(writer, x);
            }
            if let Some(x) = e.sample_composition_time_offset {
                write_i32!(writer, x);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrunEntry {
    pub sample_duration: Option<u32>,
    pub sample_size: Option<u32>,
    pub sample_flags: Option<u32>,
    pub sample_composition_time_offset: Option<i32>,
}

#[derive(Debug)]
pub struct TrackFragmentBaseMediaDecodeTimeBox {
    pub base_media_decode_time: u32,
}
impl TrackFragmentBaseMediaDecodeTimeBox {
    pub fn new() -> Self {
        TrackFragmentBaseMediaDecodeTimeBox {
            base_media_decode_time: 0,
        }
    }
}
impl WriteBoxTo for TrackFragmentBaseMediaDecodeTimeBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"tfdt")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for TrackFragmentBaseMediaDecodeTimeBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.base_media_decode_time);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleFlags {
    // reserved(4)
    pub is_leading: u8,             // u2
    pub sample_depends_on: u8,      // u2
    pub sample_is_depdended_on: u8, // u2
    pub sample_has_redundancy: u8,  // u2
    pub sample_padding_value: u8,   // u3
    pub sample_is_non_sync_sample: bool,
    pub sample_degradation_priority: u16,
}
impl SampleFlags {
    pub fn to_u32(&self) -> u32 {
        (u32::from(self.is_leading) << 26) | (u32::from(self.sample_depends_on) << 24)
            | (u32::from(self.sample_is_depdended_on) << 22)
            | (u32::from(self.sample_has_redundancy) << 20)
            | (u32::from(self.sample_padding_value) << 17)
            | ((self.sample_is_non_sync_sample as u32) << 16)
            | u32::from(self.sample_degradation_priority)
    }
}

#[derive(Debug)]
pub struct TrackFragmentHeaderBox {
    pub track_id: u32,
    pub duration_is_empty: bool,
    pub default_base_is_moof: bool,
    pub base_data_offset: Option<u64>,
    pub sample_description_index: Option<u32>,
    pub default_sample_duration: Option<u32>,
    pub default_sample_size: Option<u32>,
    pub default_sample_flags: Option<u32>,
}
impl TrackFragmentHeaderBox {
    pub fn new(track_id: u32) -> Self {
        TrackFragmentHeaderBox {
            track_id,
            duration_is_empty: false,
            default_base_is_moof: true,
            base_data_offset: None,
            sample_description_index: None,
            default_sample_duration: None,
            default_sample_size: None,
            default_sample_flags: None,
        }
    }
}
impl WriteBoxTo for TrackFragmentHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"tfhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        let flags = (self.base_data_offset.is_some() as u32 * 0x00_0001)
            | (self.sample_description_index.is_some() as u32 * 0x00_0002)
            | (self.default_sample_duration.is_some() as u32 * 0x00_0008)
            | (self.default_sample_size.is_some() as u32 * 0x00_0010)
            | (self.default_sample_flags.is_some() as u32 * 0x00_0020)
            | (self.duration_is_empty as u32 * 0x01_0000)
            | (self.default_base_is_moof as u32 * 0x02_0000);
        Some(FullBoxHeader::new(0, flags))
    }
}
impl WriteTo for TrackFragmentHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.track_id);
        if let Some(x) = self.base_data_offset {
            write_u64!(writer, x);
        }
        if let Some(x) = self.sample_description_index {
            write_u32!(writer, x);
        }
        if let Some(x) = self.default_sample_duration {
            write_u32!(writer, x);
        }
        if let Some(x) = self.default_sample_size {
            write_u32!(writer, x);
        }
        if let Some(x) = self.default_sample_flags {
            write_u32!(writer, x);
        }
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
