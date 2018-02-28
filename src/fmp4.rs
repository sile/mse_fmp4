use std::ffi::CString;
use std::fmt;
use std::io::Write;
use std::str;
use byteorder::{BigEndian, WriteBytesExt};

use {ErrorKind, Result};
use aac;
use io::{ByteCounter, WriteTo};

macro_rules! write_u8 {
    ($w:expr, $n:expr) => { track_io!($w.write_u8($n))?; }
}
macro_rules! write_u16 {
    ($w:expr, $n:expr) => { track_io!($w.write_u16::<BigEndian>($n))?; }
}
macro_rules! write_i16 {
    ($w:expr, $n:expr) => { track_io!($w.write_i16::<BigEndian>($n))?; }
}
macro_rules! write_u24 {
    ($w:expr, $n:expr) => { track_io!($w.write_uint::<BigEndian>($n as u64, 3))?; }
}
macro_rules! write_u32 {
    ($w:expr, $n:expr) => { track_io!($w.write_u32::<BigEndian>($n))?; }
}
macro_rules! write_i32 {
    ($w:expr, $n:expr) => { track_io!($w.write_i32::<BigEndian>($n))?; }
}
macro_rules! write_u64 {
    ($w:expr, $n:expr) => { track_io!($w.write_u64::<BigEndian>($n))?; }
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
pub struct InitializationSegment {
    pub ftyp_box: FileTypeBox,
    pub moov_box: MovieBox,
}
impl InitializationSegment {
    pub fn new() -> Self {
        InitializationSegment {
            ftyp_box: FileTypeBox::default(),
            moov_box: MovieBox::new(),
        }
    }
}
impl WriteTo for InitializationSegment {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.ftyp_box);
        write_box!(writer, self.moov_box);
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
pub struct MovieBox {
    pub mvhd_box: MovieHeaderBox,
    pub trak_boxes: Vec<TrackBox>,
    pub mvex_box: MovieExtendsBox,
}
impl MovieBox {
    pub fn new() -> Self {
        MovieBox {
            mvhd_box: MovieHeaderBox::new(),
            trak_boxes: Vec::new(),
            mvex_box: MovieExtendsBox::new(),
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
        track_assert!(!self.trak_boxes.is_empty(), ErrorKind::InvalidInput);

        write_box!(writer, self.mvhd_box);
        write_boxes!(writer, &self.trak_boxes);
        write_box!(writer, self.mvex_box);
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

#[derive(Debug)]
pub struct MovieExtendsBox {
    pub mehd_box: MovieExtendsHeaderBox,
    pub trex_boxes: Vec<TrackExtendsBox>,
}
impl MovieExtendsBox {
    pub fn new() -> Self {
        MovieExtendsBox {
            mehd_box: MovieExtendsHeaderBox::new(),
            trex_boxes: Vec::new(), // FIXME
        }
    }
}
impl WriteBoxTo for MovieExtendsBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mvex")
    }
}
impl WriteTo for MovieExtendsBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.mehd_box);
        write_boxes!(writer, &self.trex_boxes);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrackBox {
    pub tkhd_box: TrackHeaderBox,
    pub mdia_box: MediaBox,
}
impl TrackBox {
    pub fn new(is_video: bool) -> Self {
        TrackBox {
            tkhd_box: TrackHeaderBox::new(is_video),
            mdia_box: MediaBox::new(is_video),
        }
    }
}
impl WriteBoxTo for TrackBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"trak")
    }
}
impl WriteTo for TrackBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.tkhd_box);
        write_box!(writer, self.mdia_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MediaBox {
    pub mdhd_box: MediaHeaderBox,
    pub hdlr_box: HandlerReferenceBox,
    pub minf_box: MediaInformationBox,
}
impl MediaBox {
    pub fn new(is_video: bool) -> Self {
        MediaBox {
            mdhd_box: MediaHeaderBox::new(),
            hdlr_box: HandlerReferenceBox::new(is_video),
            minf_box: MediaInformationBox::new(is_video),
        }
    }
}
impl WriteBoxTo for MediaBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mdia")
    }
}
impl WriteTo for MediaBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.mdhd_box);
        write_box!(writer, self.hdlr_box);
        write_box!(writer, self.minf_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MediaInformationBox {
    pub vmhd_box: Option<VideoMediaHeaderBox>,
    pub smhd_box: Option<SoundMediaHeaderBox>,
    pub dinf_box: DataInformationBox,
    pub stbl_box: SampleTableBox,
}
impl MediaInformationBox {
    pub fn new(is_video: bool) -> Self {
        MediaInformationBox {
            vmhd_box: if is_video {
                Some(VideoMediaHeaderBox::new())
            } else {
                None
            },
            smhd_box: if !is_video {
                Some(SoundMediaHeaderBox::new())
            } else {
                None
            },
            dinf_box: DataInformationBox::new(),
            stbl_box: SampleTableBox::new(),
        }
    }
}
impl WriteBoxTo for MediaInformationBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"minf")
    }
}
impl WriteTo for MediaInformationBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        if let Some(ref x) = self.vmhd_box {
            write_box!(writer, x);
        }
        if let Some(ref x) = self.smhd_box {
            write_box!(writer, x);
        }
        write_box!(writer, self.dinf_box);
        write_box!(writer, self.stbl_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleTableBox {
    pub stsd_box: SampleDescriptionBox,
    pub stts_box: TimeToSampleBox,
    pub stsc_box: SampleToChunkBox,
    pub stsz_box: SampleSizeBox,
    pub stco_box: ChunkOffsetBox,
}
impl SampleTableBox {
    pub fn new() -> Self {
        SampleTableBox {
            stsd_box: SampleDescriptionBox::new(),
            stts_box: TimeToSampleBox,
            stsc_box: SampleToChunkBox,
            stsz_box: SampleSizeBox,
            stco_box: ChunkOffsetBox,
        }
    }
}
impl WriteBoxTo for SampleTableBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stbl")
    }
}
impl WriteTo for SampleTableBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.stsd_box);
        write_box!(writer, self.stts_box);
        write_box!(writer, self.stsc_box);
        write_box!(writer, self.stsz_box);
        write_box!(writer, self.stco_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleSizeBox;
impl WriteBoxTo for SampleSizeBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stsz")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for SampleSizeBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TimeToSampleBox;
impl WriteBoxTo for TimeToSampleBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stts")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for TimeToSampleBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct ChunkOffsetBox;
impl WriteBoxTo for ChunkOffsetBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stco")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for ChunkOffsetBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleToChunkBox;
impl WriteBoxTo for SampleToChunkBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stsc")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for SampleToChunkBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AvcConfigurationBox {
    pub config: ::avc::AvcDecoderConfigurationRecord,
}
impl WriteBoxTo for AvcConfigurationBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"avcC")
    }
}
impl WriteTo for AvcConfigurationBox {
    fn write_to<W: Write>(&self, writer: W) -> Result<()> {
        track!(self.config.write_to(writer))
    }
}

// ISO/IEC 14496-1
#[derive(Debug, Clone)]
pub struct Mpeg4EsDescriptorBox {
    pub profile: aac::AacProfile,
    pub frequency: aac::SamplingFrequency,
    pub channel_configuration: aac::ChannelConfiguration,
    // the maximum bitrate of this elementary stream in any time window of one second duration.
    // TODO: max_bitrate
}
impl WriteBoxTo for Mpeg4EsDescriptorBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"esds")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for Mpeg4EsDescriptorBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        // es descriptor
        write_u8!(writer, 0x03); // descriptor_tag=es
        write_u8!(writer, 25); // descriptor_len
        write_u16!(writer, 0); // es_id (TODO)
        write_u8!(writer, 0); // stream_priority and flags

        // decoder configuration descriptor
        write_u8!(writer, 0x04); // descriptor_tag=decoder_configuration
        write_u8!(writer, 17); // descriptor_len

        write_u8!(writer, 0x40); // object_type
        write_u8!(writer, (5 << 2) | 1); // stream_type=audio=5, upstream=0, reserved=1
        write_u24!(writer, 0); // buffer_size
        write_u32!(writer, 0); // max_bitrate (TODO)
        write_u32!(writer, 0); // avg_bitrate (TODO)

        // decoder specific info
        write_u8!(writer, 0x05); // descriptor_tag=decoder_specific_info
        write_u8!(writer, 2); // descriptor_len
        write_u16!(
            writer,
            ((self.profile as u16 + 1) << 11) | ((self.frequency as u16) << 7)
                | ((self.channel_configuration as u16) << 3)
        );

        // sl configuration descriptor
        write_u8!(writer, 0x06); // descriptor_tag=es_configuration_descriptor
        write_u8!(writer, 1); // descriptor_len
        write_u8!(writer, 2); // MP4

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AudioSampleEntry {
    pub channels: u16,
    pub sample_rate: u16,
    pub esds_box: Mpeg4EsDescriptorBox,
}
impl WriteTo for AudioSampleEntry {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 8);
        track_assert!(
            self.channels == 1 || self.channels == 2,
            ErrorKind::Unsupported
        );
        write_u16!(writer, self.channels);
        write_u16!(writer, 16);
        write_zeroes!(writer, 4);
        write_u16!(writer, self.sample_rate);
        write_zeroes!(writer, 2);

        write_box!(writer, self.esds_box);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Mpeg4AudioSampleEntry {
    pub audio_sample_entry: AudioSampleEntry,
}
impl Mpeg4AudioSampleEntry {
    pub fn new(audio_sample_entry: AudioSampleEntry) -> Self {
        Mpeg4AudioSampleEntry { audio_sample_entry }
    }
    pub fn to_sample_entry(&self) -> Result<SampleEntry> {
        let mut data = Vec::new();
        track!(self.audio_sample_entry.write_to(&mut data))?;
        Ok(SampleEntry {
            format: SampleFormat(*b"mp4a"),
            data_reference_index: 1,
            data,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AvcSampleEntry {
    pub width: u16,
    pub height: u16,
    pub avcc_box: AvcConfigurationBox,
}
impl AvcSampleEntry {
    pub fn to_sample_entry(&self) -> Result<SampleEntry> {
        let mut data = Vec::new();
        write_zeroes!(&mut data, 16);
        write_u16!(&mut data, self.width);
        write_u16!(&mut data, self.height);
        write_u32!(&mut data, 0x0048_0000);
        write_u32!(&mut data, 0x0048_0000);
        write_zeroes!(&mut data, 4);
        write_u16!(&mut data, 1);
        write_zeroes!(&mut data, 32);
        write_u16!(&mut data, 0x0018);
        write_i16!(&mut data, -1);
        write_box!(&mut data, self.avcc_box);
        Ok(SampleEntry {
            format: SampleFormat(*b"avc1"),
            data_reference_index: 1,
            data,
        })
    }
}

#[derive(Debug)]
pub struct SampleDescriptionBox {
    pub sample_entries: Vec<SampleEntry>,
}
impl SampleDescriptionBox {
    pub fn new() -> Self {
        SampleDescriptionBox {
            sample_entries: Vec::new(), // FIXME
        }
    }
}
impl WriteBoxTo for SampleDescriptionBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stsd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for SampleDescriptionBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.sample_entries.len() as u32);
        write_boxes!(writer, &self.sample_entries);
        Ok(())
    }
}
impl WriteBoxTo for SampleEntry {
    fn box_type(&self) -> BoxType {
        BoxType(self.format.0)
    }
}
impl WriteTo for SampleEntry {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 6);
        write_u16!(writer, self.data_reference_index);
        write_all!(writer, &self.data);
        Ok(())
    }
}

#[derive(Debug)]
pub struct DataInformationBox {
    pub dref_box: DataReferenceBox,
}
impl DataInformationBox {
    pub fn new() -> Self {
        DataInformationBox {
            dref_box: DataReferenceBox::new(),
        }
    }
}
impl WriteBoxTo for DataInformationBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"dinf")
    }
}
impl WriteTo for DataInformationBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.dref_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct DataReferenceBox {
    pub url_box: DataEntryUrlBox,
}
impl DataReferenceBox {
    pub fn new() -> Self {
        DataReferenceBox {
            url_box: DataEntryUrlBox::new(),
        }
    }
}
impl WriteBoxTo for DataReferenceBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"dref")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for DataReferenceBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 1);
        write_box!(writer, self.url_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct DataEntryUrlBox {
    pub location: Option<CString>,
}
impl DataEntryUrlBox {
    pub fn new() -> Self {
        DataEntryUrlBox { location: None }
    }
}
impl WriteBoxTo for DataEntryUrlBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"url ")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        let flags = if self.location.is_some() {
            0
        } else {
            0x00_0001
        };
        Some(FullBoxHeader::new(0, flags))
    }
}
impl WriteTo for DataEntryUrlBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        if let Some(ref x) = self.location {
            write_all!(writer, x.as_bytes_with_nul());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct VideoMediaHeaderBox {
    pub graphicsmode: u16,
    pub opcolor: [u16; 3],
}
impl VideoMediaHeaderBox {
    pub fn new() -> Self {
        VideoMediaHeaderBox {
            graphicsmode: 0,
            opcolor: [0, 0, 0],
        }
    }
}
impl WriteBoxTo for VideoMediaHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"vmhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 1))
    }
}
impl WriteTo for VideoMediaHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u16!(writer, self.graphicsmode);
        for &x in &self.opcolor {
            write_u16!(writer, x);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SoundMediaHeaderBox {
    pub balance: i16,
}
impl SoundMediaHeaderBox {
    pub fn new() -> Self {
        SoundMediaHeaderBox { balance: 0 }
    }
}
impl WriteBoxTo for SoundMediaHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"smhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for SoundMediaHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_i16!(writer, self.balance);
        write_zeroes!(writer, 2);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MediaHeaderBox {
    pub creation_time: u64,
    pub modification_time: u64,
    pub timescale: u32,
    pub duration: u64,
    pub language: u16,
}
impl MediaHeaderBox {
    pub fn new() -> Self {
        MediaHeaderBox {
            creation_time: 0,
            modification_time: 0,
            timescale: 0, // FIXME
            duration: 1,  // FIXME
            language: 21956,
        }
    }
}
impl WriteBoxTo for MediaHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mdhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(1, 0))
    }
}
impl WriteTo for MediaHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u64!(writer, self.creation_time);
        write_u64!(writer, self.modification_time);
        write_u32!(writer, self.timescale);
        write_u64!(writer, self.duration);
        write_u16!(writer, self.language);
        write_zeroes!(writer, 2);
        Ok(())
    }
}

#[derive(Debug)]
pub struct HandlerReferenceBox {
    pub handler_type: HandlerType,
    pub name: CString,
}
impl HandlerReferenceBox {
    pub fn new(is_video: bool) -> Self {
        HandlerReferenceBox {
            handler_type: HandlerType(if is_video { *b"vide" } else { *b"soun" }),
            name: CString::new("A handler").expect("Never fails"),
        }
    }
}
impl WriteBoxTo for HandlerReferenceBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"hdlr")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for HandlerReferenceBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 4);
        write_all!(writer, &self.handler_type.0);
        write_zeroes!(writer, 4 * 3);
        write_all!(writer, self.name.as_bytes_with_nul());
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrackHeaderBox {
    pub track_enabled: bool,
    pub track_in_movie: bool,
    pub track_in_preview: bool,
    pub track_size_is_aspect_ratio: bool,
    pub creation_time: u64,
    pub modification_time: u64,
    pub track_id: u32,
    pub duration: u64,
    pub layer: i16,
    pub alternate_group: i16,
    pub volume: i16, // fixed point 8.8
    pub matrix: [i32; 9],
    pub width: u32,  // fixed point 16.16
    pub height: u32, // fixed point 16.16
}
impl TrackHeaderBox {
    pub fn new(is_video: bool) -> Self {
        TrackHeaderBox {
            track_enabled: true,
            track_in_movie: true,
            track_in_preview: true,
            track_size_is_aspect_ratio: false,
            creation_time: 0,
            modification_time: 0,
            track_id: if is_video { 1 } else { 2 },
            duration: 1, // FIXME
            layer: 0,
            alternate_group: 0,
            volume: if is_video { 0 } else { 256 },
            matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
            width: 0,
            height: 0,
        }
    }
}
impl WriteBoxTo for TrackHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"tkhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        let flags = (self.track_enabled as u32 * 0x00_0001)
            | (self.track_in_movie as u32 * 0x00_0002)
            | (self.track_in_preview as u32 * 0x00_0004)
            | (self.track_size_is_aspect_ratio as u32 * 0x00_0008);
        Some(FullBoxHeader::new(1, flags))
    }
}
impl WriteTo for TrackHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u64!(writer, self.creation_time);
        write_u64!(writer, self.modification_time);
        write_u32!(writer, self.track_id);
        write_zeroes!(writer, 4);
        write_u64!(writer, self.duration);
        write_zeroes!(writer, 4 * 2);
        write_i16!(writer, self.layer);
        write_i16!(writer, self.alternate_group);
        write_i16!(writer, self.volume);
        write_zeroes!(writer, 2);
        for &x in &self.matrix {
            write_i32!(writer, x);
        }
        write_u32!(writer, self.width);
        write_u32!(writer, self.height);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieHeaderBox {
    pub creation_time: u64,
    pub modification_time: u64,
    pub timescale: u32,
    pub duration: u64,
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
            timescale: 1, // FIXME
            duration: 1,  // FIXME
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
        Some(FullBoxHeader::new(1, 0))
    }
}
impl WriteTo for MovieHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u64!(writer, self.creation_time);
        write_u64!(writer, self.modification_time);
        write_u32!(writer, self.timescale);
        write_u64!(writer, self.duration);
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

#[derive(Debug)]
pub struct MovieExtendsHeaderBox {
    pub fragment_duration: u64,
}
impl MovieExtendsHeaderBox {
    pub fn new() -> Self {
        MovieExtendsHeaderBox {
            fragment_duration: 1, // FIXME
        }
    }
}
impl WriteBoxTo for MovieExtendsHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mehd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(1, 0))
    }
}
impl WriteTo for MovieExtendsHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u64!(writer, self.fragment_duration);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrackExtendsBox {
    pub track_id: u32,
    pub default_sample_description_index: u32,
    pub default_sample_duration: u32,
    pub default_sample_size: u32,
    pub default_sample_flags: u32,
}
impl TrackExtendsBox {
    pub fn new(track_id: u32) -> Self {
        TrackExtendsBox {
            track_id,
            default_sample_description_index: 1,
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}
impl WriteBoxTo for TrackExtendsBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"trex")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for TrackExtendsBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.track_id);
        write_u32!(writer, self.default_sample_description_index);
        write_u32!(writer, self.default_sample_duration);
        write_u32!(writer, self.default_sample_size);
        write_u32!(writer, self.default_sample_flags);
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SampleEntry {
    pub format: SampleFormat,
    pub data_reference_index: u16,
    pub data: Vec<u8>,
}
