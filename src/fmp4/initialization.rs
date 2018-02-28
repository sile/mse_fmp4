use std::ffi::CString;
use std::io::Write;

use {ErrorKind, Result};
use aac::{AacProfile, ChannelConfiguration, SamplingFrequency};
use avc::AvcDecoderConfigurationRecord;
use fmp4::Mp4Box;
use io::WriteTo;

#[derive(Debug)]
pub struct InitializationSegment {
    pub ftyp_box: FileTypeBox,
    pub moov_box: MovieBox,
}
impl InitializationSegment {
    pub fn new() -> Self {
        InitializationSegment {
            ftyp_box: FileTypeBox,
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
pub struct FileTypeBox;
impl Mp4Box for FileTypeBox {
    const BOX_TYPE: [u8; 4] = *b"ftyp";

    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_all!(writer, b"isom"); // major_brand
        write_u32!(writer, 512); // minor_version
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieBox {
    pub mvhd_box: MovieHeaderBox,
    pub trak_boxes: Vec<TrackBox>,
}
impl MovieBox {
    pub fn new() -> Self {
        MovieBox {
            mvhd_box: MovieHeaderBox::new(),
            trak_boxes: Vec::new(),
        }
    }
}
impl Mp4Box for MovieBox {
    const BOX_TYPE: [u8; 4] = *b"moov";

    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.trak_boxes.is_empty(), ErrorKind::InvalidInput);

        write_box!(writer, self.mvhd_box);
        write_boxes!(writer, &self.trak_boxes);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieHeaderBox {
    pub creation_time: u64, // TODO: u32(?)
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
impl Mp4Box for MovieHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"mvhd";

    fn box_version(&self) -> Option<u8> {
        Some(1)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
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
pub struct TrackBox {
    pub tkhd_box: TrackHeaderBox,
    // TODO: pub edts_box: Option<...>,
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
impl Mp4Box for TrackBox {
    const BOX_TYPE: [u8; 4] = *b"trak";

    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.tkhd_box);
        write_box!(writer, self.mdia_box);
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
impl Mp4Box for TrackHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"tkhd";

    fn box_version(&self) -> Option<u8> {
        Some(1)
    }
    fn box_flags(&self) -> Option<u32> {
        let flags = (self.track_enabled as u32 * 0x00_0001)
            | (self.track_in_movie as u32 * 0x00_0002)
            | (self.track_in_preview as u32 * 0x00_0004)
            | (self.track_size_is_aspect_ratio as u32 * 0x00_0008);
        Some(flags)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
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
impl Mp4Box for MediaBox {
    const BOX_TYPE: [u8; 4] = *b"mdia";

    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.mdhd_box);
        write_box!(writer, self.hdlr_box);
        write_box!(writer, self.minf_box);
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
impl Mp4Box for MediaHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"mdhd";

    fn box_version(&self) -> Option<u8> {
        Some(1)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
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
    pub handler_type: [u8; 4],
    pub name: CString,
}
impl HandlerReferenceBox {
    pub fn new(is_video: bool) -> Self {
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
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 4);
        write_all!(writer, &self.handler_type);
        write_zeroes!(writer, 4 * 3);
        write_all!(writer, self.name.as_bytes_with_nul());
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
                Some(VideoMediaHeaderBox)
            } else {
                None
            },
            smhd_box: if !is_video {
                Some(SoundMediaHeaderBox)
            } else {
                None
            },
            dinf_box: DataInformationBox::new(),
            stbl_box: SampleTableBox::new(),
        }
    }
}
impl Mp4Box for MediaInformationBox {
    const BOX_TYPE: [u8; 4] = *b"minf";

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

#[derive(Debug)]
pub struct VideoMediaHeaderBox;
impl Mp4Box for VideoMediaHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"vmhd";

    fn box_flags(&self) -> Option<u32> {
        Some(1)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u16!(writer, 0); // graphicsmode
        write_zeroes!(writer, 2 * 3); // opcolor
        Ok(())
    }
}

#[derive(Debug)]
pub struct SoundMediaHeaderBox;
impl Mp4Box for SoundMediaHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"smhd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_i16!(writer, 0); // balance
        write_zeroes!(writer, 2);
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
impl Mp4Box for DataInformationBox {
    const BOX_TYPE: [u8; 4] = *b"dinf";

    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
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
            url_box: DataEntryUrlBox,
        }
    }
}
impl Mp4Box for DataReferenceBox {
    const BOX_TYPE: [u8; 4] = *b"dref";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 1); // entry_count
        write_box!(writer, self.url_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct DataEntryUrlBox;
impl Mp4Box for DataEntryUrlBox {
    const BOX_TYPE: [u8; 4] = *b"url ";

    fn box_flags(&self) -> Option<u32> {
        Some(0x00_0001)
    }
    fn write_box_payload<W: Write>(&self, _writer: W) -> Result<()> {
        // NOTE: null location
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
impl Mp4Box for SampleTableBox {
    const BOX_TYPE: [u8; 4] = *b"stbl";

    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.stsd_box);
        write_box!(writer, self.stts_box);
        write_box!(writer, self.stsc_box);
        write_box!(writer, self.stsz_box);
        write_box!(writer, self.stco_box);
        Ok(())
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
impl Mp4Box for SampleDescriptionBox {
    const BOX_TYPE: [u8; 4] = *b"stsd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.sample_entries.len() as u32);
        write_boxes!(writer, &self.sample_entries);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleSizeBox;
impl Mp4Box for SampleSizeBox {
    const BOX_TYPE: [u8; 4] = *b"stsz";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TimeToSampleBox;
impl Mp4Box for TimeToSampleBox {
    const BOX_TYPE: [u8; 4] = *b"stts";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct ChunkOffsetBox;
impl Mp4Box for ChunkOffsetBox {
    const BOX_TYPE: [u8; 4] = *b"stco";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleToChunkBox;
impl Mp4Box for SampleToChunkBox {
    const BOX_TYPE: [u8; 4] = *b"stsc";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub enum SampleEntry {
    Avc(AvcSampleEntry),
    Aac(AacSampleEntry),
}
impl SampleEntry {
    fn write_box<W: Write>(&self, writer: W) -> Result<()> {
        match *self {
            SampleEntry::Avc(ref x) => track!(x.write_box(writer)),
            SampleEntry::Aac(ref x) => track!(x.write_box(writer)),
        }
    }
}

#[derive(Debug)]
pub struct AvcSampleEntry {
    pub width: u16,
    pub height: u16,
    pub avcc_box: AvcConfigurationBox,
}
impl Mp4Box for AvcSampleEntry {
    const BOX_TYPE: [u8; 4] = *b"avc1";

    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
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
        write_box!(writer, self.avcc_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct AvcConfigurationBox {
    pub configuration: AvcDecoderConfigurationRecord,
}
impl Mp4Box for AvcConfigurationBox {
    const BOX_TYPE: [u8; 4] = *b"avcC";

    fn write_box_payload<W: Write>(&self, writer: W) -> Result<()> {
        track!(self.configuration.write_to(writer))
    }
}

#[derive(Debug)]
pub struct AacSampleEntry {
    pub channels: u16,
    pub sample_rate: u16,
    pub esds_box: Mpeg4EsDescriptorBox,
}
impl Mp4Box for AacSampleEntry {
    const BOX_TYPE: [u8; 4] = *b"mp4a";

    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 6);
        write_u16!(writer, 1); // data_reference_index

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

// ISO/IEC 14496-1
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
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
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
        write_u32!(writer, 0); // max_bitrate
        write_u32!(writer, 0); // avg_bitrate

        // decoder specific info
        write_u8!(writer, 0x05); // descriptor_tag=decoder_specific_info
        write_u8!(writer, 2); // descriptor_len
        write_u16!(
            writer,
            ((self.profile as u16 + 1) << 11) | (u16::from(self.frequency.as_index()) << 7)
                | ((self.channel_configuration as u16) << 3)
        );

        // sl configuration descriptor
        write_u8!(writer, 0x06); // descriptor_tag=es_configuration_descriptor
        write_u8!(writer, 1); // descriptor_len
        write_u8!(writer, 2); // MP4

        Ok(())
    }
}
