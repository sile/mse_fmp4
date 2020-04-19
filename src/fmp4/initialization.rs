use crate::aac::{AacProfile, ChannelConfiguration, SamplingFrequency};
use crate::avc::AvcDecoderConfigurationRecord;
use crate::fmp4::{Mp4Box, AUDIO_TRACK_ID, VIDEO_TRACK_ID};
use crate::io::{ByteCounter, WriteTo};
use crate::{ErrorKind, Result};
use std::ffi::CString;
use std::io::Write;

/// [3. Initialization Segments][init_segment] (ISO BMFF Byte Stream Format)
///
/// [init_segment]: https://w3c.github.io/media-source/isobmff-byte-stream-format.html#iso-init-segments
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct InitializationSegment {
    pub ftyp_box: FileTypeBox,
    pub moov_box: MovieBox,
}
impl InitializationSegment {
    /// Returns MIME type.
    pub fn mime_type(&self) -> String {
        // FIXME
        r#"video/mp4; "avc1.640029, mp4a.40.2""#.to_string()
    }
}
impl WriteTo for InitializationSegment {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.ftyp_box);
        write_box!(writer, self.moov_box);
        Ok(())
    }
}

/// 4.3 File Type Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct FileTypeBox;
impl Mp4Box for FileTypeBox {
    const BOX_TYPE: [u8; 4] = *b"ftyp";

    fn box_payload_size(&self) -> Result<u32> {
        Ok(8)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_all!(writer, b"isom"); // major_brand
        write_u32!(writer, 512); // minor_version
        Ok(())
    }
}

/// 8.2.1 Movie Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct MovieBox {
    pub mvhd_box: MovieHeaderBox,
    pub trak_boxes: Vec<TrackBox>,
    pub mvex_box: MovieExtendsBox,
}
impl Mp4Box for MovieBox {
    const BOX_TYPE: [u8; 4] = *b"moov";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += box_size!(self.mvhd_box);
        size += boxes_size!(self.trak_boxes);
        size += box_size!(self.mvex_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.trak_boxes.is_empty(), ErrorKind::InvalidInput);
        write_box!(writer, self.mvhd_box);
        write_boxes!(writer, &self.trak_boxes);
        write_box!(writer, &self.mvex_box);
        Ok(())
    }
}

/// 8.8.1 Movie Extends Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct MovieExtendsBox {
    pub mehd_box: Option<MovieExtendsHeaderBox>,
    pub trex_boxes: Vec<TrackExtendsBox>,
}
impl Mp4Box for MovieExtendsBox {
    const BOX_TYPE: [u8; 4] = *b"mvex";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += optional_box_size!(self.mehd_box);
        size += boxes_size!(self.trex_boxes);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.trex_boxes.is_empty(), ErrorKind::InvalidInput);
        if let Some(mehd_box) = &self.mehd_box {
            write_box!(writer, mehd_box);
        }
        write_boxes!(writer, &self.trex_boxes);
        Ok(())
    }
}

/// 8.8.2 Movie Extends Header Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct MovieExtendsHeaderBox {
    pub fragment_duration: u32,
}
impl Mp4Box for MovieExtendsHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"mehd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.fragment_duration);
        Ok(())
    }
}

/// 8.8.3 Track Extends Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TrackExtendsBox {
    track_id: u32,
    default_sample_description_index: u32,
    pub default_sample_duration: u32,
    pub default_sample_size: u32,
    pub default_sample_flags: u32,
}
impl TrackExtendsBox {
    /// Makes a new `TrackExtendsBox` instance.
    pub fn new(is_video: bool) -> Self {
        TrackExtendsBox {
            track_id: if is_video {
                VIDEO_TRACK_ID
            } else {
                AUDIO_TRACK_ID
            },
            default_sample_description_index: 1,
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}
impl Mp4Box for TrackExtendsBox {
    const BOX_TYPE: [u8; 4] = *b"trex";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4 * 5)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.track_id);
        write_u32!(writer, self.default_sample_description_index);
        write_u32!(writer, self.default_sample_duration);
        write_u32!(writer, self.default_sample_size);
        write_u32!(writer, self.default_sample_flags);
        Ok(())
    }
}

/// 8.2.2 Movie Header Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct MovieHeaderBox {
    pub timescale: u32,
    pub duration: u32,
}
impl Default for MovieHeaderBox {
    fn default() -> Self {
        MovieHeaderBox {
            timescale: 1,
            duration: 1,
        }
    }
}
impl Mp4Box for MovieHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"mvhd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        let size = track!(ByteCounter::calculate(|w| self.write_box_payload(w)))?;
        Ok(size as u32)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0); // creation_time
        write_u32!(writer, 0); // modification_time
        write_u32!(writer, self.timescale);
        write_u32!(writer, self.duration);
        write_i32!(writer, 0x1_0000); // rate
        write_i16!(writer, 256); // volume
        write_zeroes!(writer, 2);
        write_zeroes!(writer, 4 * 2);
        for &x in &[0x1_0000, 0, 0, 0, 0x1_0000, 0, 0, 0, 0x4000_0000] {
            write_i32!(writer, x); // matrix
        }
        write_zeroes!(writer, 4 * 6);
        write_u32!(writer, 0xFFFF_FFFF); // next_track_id
        Ok(())
    }
}

/// 8.3.1 Track Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TrackBox {
    pub tkhd_box: TrackHeaderBox,
    pub edts_box: EditBox,
    pub mdia_box: MediaBox,
}
impl TrackBox {
    /// Makes a new `TrackBox` instance.
    pub fn new(is_video: bool) -> Self {
        TrackBox {
            tkhd_box: TrackHeaderBox::new(is_video),
            edts_box: EditBox::default(),
            mdia_box: MediaBox::new(is_video),
        }
    }
}
impl Mp4Box for TrackBox {
    const BOX_TYPE: [u8; 4] = *b"trak";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += box_size!(self.tkhd_box);
        size += box_size!(self.edts_box);
        size += box_size!(self.mdia_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.tkhd_box);
        write_box!(writer, self.edts_box);
        write_box!(writer, self.mdia_box);
        Ok(())
    }
}

/// 8.3.2 Track Header Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TrackHeaderBox {
    track_id: u32,
    pub duration: u32,
    volume: i16,     // fixed point 8.8
    pub width: u32,  // fixed point 16.16
    pub height: u32, // fixed point 16.16
}
impl TrackHeaderBox {
    fn new(is_video: bool) -> Self {
        TrackHeaderBox {
            track_id: if is_video {
                VIDEO_TRACK_ID
            } else {
                AUDIO_TRACK_ID
            },
            duration: 1,
            volume: if is_video { 0 } else { 256 },
            width: 0,
            height: 0,
        }
    }
}
impl Mp4Box for TrackHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"tkhd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_flags(&self) -> Option<u32> {
        // track_enabled | track_in_movie | track_in_preview
        let flags = 0x00_0001 | 0x00_0002 | 0x00_0004;
        Some(flags)
    }
    fn box_payload_size(&self) -> Result<u32> {
        let size = track!(ByteCounter::calculate(|w| self.write_box_payload(w)))?;
        Ok(size as u32)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0); // creation_time
        write_u32!(writer, 0); // modification_time
        write_u32!(writer, self.track_id);
        write_zeroes!(writer, 4);
        write_u32!(writer, self.duration);
        write_zeroes!(writer, 4 * 2);
        write_i16!(writer, 0); // layer
        write_i16!(writer, 0); // alternate_group
        write_i16!(writer, self.volume);
        write_zeroes!(writer, 2);
        for &x in &[0x1_0000, 0, 0, 0, 0x1_0000, 0, 0, 0, 0x4000_0000] {
            write_i32!(writer, x); // matrix
        }
        write_u32!(writer, self.width);
        write_u32!(writer, self.height);
        Ok(())
    }
}

/// 8.6.5 Edit Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct EditBox {
    pub elst_box: EditListBox,
}
impl Mp4Box for EditBox {
    const BOX_TYPE: [u8; 4] = *b"edts";

    fn box_payload_size(&self) -> Result<u32> {
        Ok(box_size!(self.elst_box))
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.elst_box);
        Ok(())
    }
}

/// 8.6.6 Edit List Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct EditListBox {
    pub media_time: i32,
}
impl Mp4Box for EditListBox {
    const BOX_TYPE: [u8; 4] = *b"elst";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4 + 4 + 4 + 2 + 2)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 1); // entry_count
        write_u32!(writer, 0); // segment_duration ("0" indicating that it spans all subsequent media)
        write_i32!(writer, self.media_time);
        write_i16!(writer, 1); // media_rate_integer
        write_i16!(writer, 0); // media_rate_fraction
        Ok(())
    }
}

/// 8.4.1 Media Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct MediaBox {
    pub mdhd_box: MediaHeaderBox,
    pub hdlr_box: HandlerReferenceBox,
    pub minf_box: MediaInformationBox,
}
impl MediaBox {
    fn new(is_video: bool) -> Self {
        MediaBox {
            mdhd_box: MediaHeaderBox::default(),
            hdlr_box: HandlerReferenceBox::new(is_video),
            minf_box: MediaInformationBox::new(is_video),
        }
    }
}
impl Mp4Box for MediaBox {
    const BOX_TYPE: [u8; 4] = *b"mdia";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += box_size!(self.mdhd_box);
        size += box_size!(self.hdlr_box);
        size += box_size!(self.minf_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.mdhd_box);
        write_box!(writer, self.hdlr_box);
        write_box!(writer, self.minf_box);
        Ok(())
    }
}

/// 8.4.2 Media Header Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct MediaHeaderBox {
    pub timescale: u32,
    pub duration: u32,
}
impl Default for MediaHeaderBox {
    fn default() -> Self {
        MediaHeaderBox {
            timescale: 0,
            duration: 1,
        }
    }
}
impl Mp4Box for MediaHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"mdhd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4 + 4 + 4 + 4 + 2 + 2)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0); // creation_time
        write_u32!(writer, 0); // modification_time
        write_u32!(writer, self.timescale);
        write_u32!(writer, self.duration);
        write_u16!(writer, 0x55c4); // language
        write_zeroes!(writer, 2);
        Ok(())
    }
}

/// 8.4.3 Handler Reference Box (ISO/IEC 14496-12).
#[derive(Debug)]
pub struct HandlerReferenceBox {
    handler_type: [u8; 4],
    name: CString,
}
impl HandlerReferenceBox {
    fn new(is_video: bool) -> Self {
        let name = if is_video {
            "Video Handler"
        } else {
            "Sound Handler"
        };
        HandlerReferenceBox {
            handler_type: if is_video { *b"vide" } else { *b"soun" },
            name: CString::new(name).expect("Never fails"),
        }
    }
}
impl Mp4Box for HandlerReferenceBox {
    const BOX_TYPE: [u8; 4] = *b"hdlr";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        let size = track!(ByteCounter::calculate(|w| self.write_box_payload(w)))?;
        Ok(size as u32)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 4);
        write_all!(writer, &self.handler_type);
        write_zeroes!(writer, 4 * 3);
        write_all!(writer, self.name.as_bytes_with_nul());
        Ok(())
    }
}

/// 8.4.4 Media Information Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct MediaInformationBox {
    pub vmhd_box: Option<VideoMediaHeaderBox>,
    pub smhd_box: Option<SoundMediaHeaderBox>,
    pub dinf_box: DataInformationBox,
    pub stbl_box: SampleTableBox,
}
impl MediaInformationBox {
    fn new(is_video: bool) -> Self {
        MediaInformationBox {
            vmhd_box: if is_video {
                Some(VideoMediaHeaderBox)
            } else {
                None
            },
            smhd_box: if !is_video {
                Some(SoundMediaHeaderBox)
            } else {
                None
            },
            dinf_box: DataInformationBox::default(),
            stbl_box: SampleTableBox::default(),
        }
    }
}
impl Mp4Box for MediaInformationBox {
    const BOX_TYPE: [u8; 4] = *b"minf";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += optional_box_size!(self.vmhd_box);
        size += optional_box_size!(self.smhd_box);
        size += box_size!(self.dinf_box);
        size += box_size!(self.stbl_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
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

/// 12.1.2 Video media header (ISO/IEC 14496-12).
#[derive(Debug)]
pub struct VideoMediaHeaderBox;
impl Mp4Box for VideoMediaHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"vmhd";

    fn box_flags(&self) -> Option<u32> {
        Some(1)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(2 + 2 * 3)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u16!(writer, 0); // graphicsmode
        write_zeroes!(writer, 2 * 3); // opcolor
        Ok(())
    }
}

/// 12.2.2 Sound media header (ISO/IEC 14496-12).
#[derive(Debug)]
pub struct SoundMediaHeaderBox;
impl Mp4Box for SoundMediaHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"smhd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(2 + 2)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_i16!(writer, 0); // balance
        write_zeroes!(writer, 2);
        Ok(())
    }
}

/// 8.7.1 Data Information Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct DataInformationBox {
    pub dref_box: DataReferenceBox,
}
impl Mp4Box for DataInformationBox {
    const BOX_TYPE: [u8; 4] = *b"dinf";

    fn box_payload_size(&self) -> Result<u32> {
        Ok(box_size!(self.dref_box))
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.dref_box);
        Ok(())
    }
}

/// 8.7.2 Data Reference Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct DataReferenceBox {
    pub url_box: DataEntryUrlBox,
}
impl Mp4Box for DataReferenceBox {
    const BOX_TYPE: [u8; 4] = *b"dref";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 4;
        size += box_size!(self.url_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 1); // entry_count
        write_box!(writer, self.url_box);
        Ok(())
    }
}

/// 8.7.2.2 Data Entry Url Box (ISO/IEC 14496-12).
#[derive(Debug, Default)]
pub struct DataEntryUrlBox;
impl Mp4Box for DataEntryUrlBox {
    const BOX_TYPE: [u8; 4] = *b"url ";

    fn box_flags(&self) -> Option<u32> {
        Some(0x00_0001)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(0)
    }
    fn write_box_payload<W: Write>(&self, _writer: W) -> Result<()> {
        // NOTE: null location
        Ok(())
    }
}

/// 8.5.1 Sample Table Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct SampleTableBox {
    pub stsd_box: SampleDescriptionBox,
    pub stts_box: TimeToSampleBox,
    pub stsc_box: SampleToChunkBox,
    pub stsz_box: SampleSizeBox,
    pub stco_box: ChunkOffsetBox,
}
impl Mp4Box for SampleTableBox {
    const BOX_TYPE: [u8; 4] = *b"stbl";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += box_size!(self.stsd_box);
        size += box_size!(self.stts_box);
        size += box_size!(self.stsc_box);
        size += box_size!(self.stsz_box);
        size += box_size!(self.stco_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.stsd_box);
        write_box!(writer, self.stts_box);
        write_box!(writer, self.stsc_box);
        write_box!(writer, self.stsz_box);
        write_box!(writer, self.stco_box);
        Ok(())
    }
}

/// 8.5.2 Sample Description Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct SampleDescriptionBox {
    pub sample_entries: Vec<SampleEntry>,
}
impl Mp4Box for SampleDescriptionBox {
    const BOX_TYPE: [u8; 4] = *b"stsd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 4;
        size += boxes_size!(self.sample_entries);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.sample_entries.is_empty(), ErrorKind::InvalidInput);
        write_u32!(writer, self.sample_entries.len() as u32);
        write_boxes!(writer, &self.sample_entries);
        Ok(())
    }
}

/// 8.5.3 Sample Size Boxes (ISO/IEC 14496-12).
#[derive(Debug, Default)]
pub struct SampleSizeBox;
impl Mp4Box for SampleSizeBox {
    const BOX_TYPE: [u8; 4] = *b"stsz";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4 + 4)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        write_u32!(writer, 0);
        Ok(())
    }
}

/// 8.6.1.2 Decoding Time To Sample Box (ISO/IEC 14496-12).
#[derive(Debug, Default)]
pub struct TimeToSampleBox;
impl Mp4Box for TimeToSampleBox {
    const BOX_TYPE: [u8; 4] = *b"stts";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

/// 8.7.5 Chunk Offset Box (ISO/IEC 14496-12).
#[derive(Debug, Default)]
pub struct ChunkOffsetBox;
impl Mp4Box for ChunkOffsetBox {
    const BOX_TYPE: [u8; 4] = *b"stco";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

/// 8.7.4 Sample To Chunk Box (ISO/IEC 14496-12).
#[derive(Debug, Default)]
pub struct SampleToChunkBox;
impl Mp4Box for SampleToChunkBox {
    const BOX_TYPE: [u8; 4] = *b"stsc";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

/// 8.5.2.2 Sample Entry (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub enum SampleEntry {
    Avc(AvcSampleEntry),
    Aac(AacSampleEntry),
}
impl SampleEntry {
    fn box_size(&self) -> Result<u32> {
        match *self {
            SampleEntry::Avc(ref x) => track!(x.box_size()),
            SampleEntry::Aac(ref x) => track!(x.box_size()),
        }
    }
    fn write_box<W: Write>(&self, writer: W) -> Result<()> {
        match *self {
            SampleEntry::Avc(ref x) => track!(x.write_box(writer)),
            SampleEntry::Aac(ref x) => track!(x.write_box(writer)),
        }
    }
}

/// Sample Entry for AVC.
#[allow(missing_docs)]
#[derive(Debug)]
pub struct AvcSampleEntry {
    pub width: u16,
    pub height: u16,
    pub avcc_box: AvcConfigurationBox,
}
impl AvcSampleEntry {
    fn write_box_payload_without_avcc<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 6);
        write_u16!(writer, 1); // data_reference_index

        write_zeroes!(writer, 16);
        write_u16!(writer, self.width);
        write_u16!(writer, self.height);
        write_u32!(writer, 0x0048_0000);
        write_u32!(writer, 0x0048_0000);
        write_zeroes!(writer, 4);
        write_u16!(writer, 1);
        write_zeroes!(writer, 32);
        write_u16!(writer, 0x0018);
        write_i16!(writer, -1);
        Ok(())
    }
}
impl Mp4Box for AvcSampleEntry {
    const BOX_TYPE: [u8; 4] = *b"avc1";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += track!(ByteCounter::calculate(
            |w| self.write_box_payload_without_avcc(w)
        ))? as u32;
        size += box_size!(self.avcc_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        track!(self.write_box_payload_without_avcc(&mut writer))?;
        write_box!(writer, self.avcc_box);
        Ok(())
    }
}

/// Box that contains AVC Decoder Configuration Record.
#[allow(missing_docs)]
#[derive(Debug)]
pub struct AvcConfigurationBox {
    pub configuration: AvcDecoderConfigurationRecord,
}
impl Mp4Box for AvcConfigurationBox {
    const BOX_TYPE: [u8; 4] = *b"avcC";

    fn box_payload_size(&self) -> Result<u32> {
        let size = track!(ByteCounter::calculate(|w| self.configuration.write_to(w)))?;
        Ok(size as u32)
    }
    fn write_box_payload<W: Write>(&self, writer: W) -> Result<()> {
        track!(self.configuration.write_to(writer))
    }
}

/// Sample Entry for AAC.
#[allow(missing_docs)]
#[derive(Debug)]
pub struct AacSampleEntry {
    pub esds_box: Mpeg4EsDescriptorBox,
}
impl AacSampleEntry {
    fn write_box_payload_without_esds<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 6);
        write_u16!(writer, 1); // data_reference_index

        let channels = self.esds_box.channel_configuration as u16;
        let sample_rate = self.esds_box.frequency.as_u32();
        write_zeroes!(writer, 8);
        track_assert!(channels == 1 || channels == 2, ErrorKind::Unsupported);
        track_assert!(sample_rate <= 0xFFFF, ErrorKind::InvalidInput);

        write_u16!(writer, channels);
        write_u16!(writer, 16);
        write_zeroes!(writer, 4);
        write_u16!(writer, sample_rate as u16);
        write_zeroes!(writer, 2);
        Ok(())
    }
}
impl Mp4Box for AacSampleEntry {
    const BOX_TYPE: [u8; 4] = *b"mp4a";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += track!(ByteCounter::calculate(
            |w| self.write_box_payload_without_esds(w)
        ))? as u32;
        size += box_size!(self.esds_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        track!(self.write_box_payload_without_esds(&mut writer))?;
        write_box!(writer, self.esds_box);
        Ok(())
    }
}

/// MPEG-4 ES Description Box (ISO/IEC 14496-1).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct Mpeg4EsDescriptorBox {
    pub profile: AacProfile,
    pub frequency: SamplingFrequency,
    pub channel_configuration: ChannelConfiguration,
}
impl Mp4Box for Mpeg4EsDescriptorBox {
    const BOX_TYPE: [u8; 4] = *b"esds";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        let size = track!(ByteCounter::calculate(|w| self.write_box_payload(w)))?;
        Ok(size as u32)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        // es descriptor
        write_u8!(writer, 0x03); // descriptor_tag=es
        write_u8!(writer, 25); // descriptor_len
        write_u16!(writer, 0); // es_id
        write_u8!(writer, 0); // stream_priority and flags

        // decoder configuration descriptor
        write_u8!(writer, 0x04); // descriptor_tag=decoder_configuration
        write_u8!(writer, 17); // descriptor_len

        write_u8!(writer, 0x40); // object_type
        write_u8!(writer, (5 << 2) | 1); // stream_type=audio=5, upstream=0, reserved=1
        write_u24!(writer, 0); // buffer_size
        write_u32!(writer, 0); // max_bitrate
        write_u32!(writer, 0); // avg_bitrate

        // decoder specific info
        write_u8!(writer, 0x05); // descriptor_tag=decoder_specific_info
        write_u8!(writer, 2); // descriptor_len
        write_u16!(
            writer,
            ((self.profile as u16 + 1) << 11)
                | (u16::from(self.frequency.as_index()) << 7)
                | ((self.channel_configuration as u16) << 3)
        );

        // sl configuration descriptor
        write_u8!(writer, 0x06); // descriptor_tag=es_configuration_descriptor
        write_u8!(writer, 1); // descriptor_len
        write_u8!(writer, 2); // MP4

        Ok(())
    }
}
