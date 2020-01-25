use crate::fmp4::{Mp4Box, AUDIO_TRACK_ID, VIDEO_TRACK_ID};
use crate::io::{ByteCounter, WriteTo};
use crate::{ErrorKind, Result};
use std::io::Write;

/// [ISO BMFF Byte Stream Format: 4. Media Segments][media_segment]
///
/// [media_segment]: https://w3c.github.io/media-source/isobmff-byte-stream-format.html#iso-media-segments
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct MediaSegment {
    pub moof_box: MovieFragmentBox,
    pub mdat_boxes: Vec<MediaDataBox>,
}
impl WriteTo for MediaSegment {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.mdat_boxes.is_empty(), ErrorKind::InvalidInput);
        write_box!(writer, self.moof_box);
        write_boxes!(writer, &self.mdat_boxes);
        Ok(())
    }
}

/// 8.1.1 Media Data Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct MediaDataBox {
    pub data: Vec<u8>,
}
impl Mp4Box for MediaDataBox {
    const BOX_TYPE: [u8; 4] = *b"mdat";

    fn box_payload_size(&self) -> Result<u32> {
        Ok(self.data.len() as u32)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_all!(writer, &self.data);
        Ok(())
    }
}

/// 8.8.4 Movie Fragment Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct MovieFragmentBox {
    pub mfhd_box: MovieFragmentHeaderBox,
    pub traf_boxes: Vec<TrackFragmentBox>,
}
impl Mp4Box for MovieFragmentBox {
    const BOX_TYPE: [u8; 4] = *b"moof";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += box_size!(self.mfhd_box);
        size += boxes_size!(self.traf_boxes);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.traf_boxes.is_empty(), ErrorKind::InvalidInput);
        write_box!(writer, self.mfhd_box);
        write_boxes!(writer, &self.traf_boxes);
        Ok(())
    }
}

/// 8.8.5 Movie Fragment Header Box (ISO/IEC 14496-12).
#[derive(Debug)]
pub struct MovieFragmentHeaderBox {
    /// The number associated with this fragment.
    pub sequence_number: u32,
}
impl Mp4Box for MovieFragmentHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"mfhd";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.sequence_number);
        Ok(())
    }
}
impl Default for MovieFragmentHeaderBox {
    /// Return the default value of `MovieFragmentHeaderBox`.
    ///
    /// This is equivalent to `MovieFragmentHeaderBox { sequence_number: 1 }`.
    fn default() -> Self {
        MovieFragmentHeaderBox { sequence_number: 1 }
    }
}

/// 8.8.6 Track Fragment Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TrackFragmentBox {
    pub tfhd_box: TrackFragmentHeaderBox,
    pub tfdt_box: TrackFragmentBaseMediaDecodeTimeBox,
    pub trun_box: TrackRunBox,
}
impl TrackFragmentBox {
    /// Makes a new `TrackFragmentBox` instance.
    pub fn new(is_video: bool) -> Self {
        let track_id = if is_video {
            VIDEO_TRACK_ID
        } else {
            AUDIO_TRACK_ID
        };
        TrackFragmentBox {
            tfhd_box: TrackFragmentHeaderBox::new(track_id),
            tfdt_box: TrackFragmentBaseMediaDecodeTimeBox,
            trun_box: TrackRunBox::default(),
        }
    }
}
impl Mp4Box for TrackFragmentBox {
    const BOX_TYPE: [u8; 4] = *b"traf";

    fn box_payload_size(&self) -> Result<u32> {
        let mut size = 0;
        size += box_size!(self.tfhd_box);
        size += box_size!(self.tfdt_box);
        size += box_size!(self.trun_box);
        Ok(size)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.tfhd_box);
        write_box!(writer, self.tfdt_box);
        write_box!(writer, self.trun_box);
        Ok(())
    }
}

/// 8.8.7 Track Fragment Header Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug)]
pub struct TrackFragmentHeaderBox {
    track_id: u32,
    pub duration_is_empty: bool,
    pub default_base_is_moof: bool,
    pub base_data_offset: Option<u64>,
    pub sample_description_index: Option<u32>,
    pub default_sample_duration: Option<u32>,
    pub default_sample_size: Option<u32>,
    pub default_sample_flags: Option<SampleFlags>,
}
impl TrackFragmentHeaderBox {
    fn new(track_id: u32) -> Self {
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
impl Mp4Box for TrackFragmentHeaderBox {
    const BOX_TYPE: [u8; 4] = *b"tfhd";

    fn box_flags(&self) -> Option<u32> {
        let flags = self.base_data_offset.is_some() as u32
            | (self.sample_description_index.is_some() as u32 * 0x00_0002)
            | (self.default_sample_duration.is_some() as u32 * 0x00_0008)
            | (self.default_sample_size.is_some() as u32 * 0x00_0010)
            | (self.default_sample_flags.is_some() as u32 * 0x00_0020)
            | (self.duration_is_empty as u32 * 0x01_0000)
            | (self.default_base_is_moof as u32 * 0x02_0000);
        Some(flags)
    }
    fn box_payload_size(&self) -> Result<u32> {
        let size = track!(ByteCounter::calculate(|w| self.write_box_payload(w)))?;
        Ok(size as u32)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
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
            write_u32!(writer, x.to_u32());
        }
        Ok(())
    }
}

/// 8.8.12 Track fragment decode time (ISO/IEC 14496-12).
#[derive(Debug)]
pub struct TrackFragmentBaseMediaDecodeTimeBox;
impl Mp4Box for TrackFragmentBaseMediaDecodeTimeBox {
    const BOX_TYPE: [u8; 4] = *b"tfdt";

    fn box_version(&self) -> Option<u8> {
        Some(0)
    }
    fn box_payload_size(&self) -> Result<u32> {
        Ok(4)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0); // base_media_decode_time
        Ok(())
    }
}

/// 8.8.8 Track Fragment Run Box (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default)]
pub struct TrackRunBox {
    pub data_offset: Option<i32>,
    pub first_sample_flags: Option<SampleFlags>,
    pub samples: Vec<Sample>,
}
impl Mp4Box for TrackRunBox {
    const BOX_TYPE: [u8; 4] = *b"trun";

    fn box_version(&self) -> Option<u8> {
        Some(1)
    }
    fn box_flags(&self) -> Option<u32> {
        let sample = self
            .samples
            .first()
            .cloned()
            .unwrap_or_else(Sample::default);
        let flags = self.data_offset.is_some() as u32
            | (self.first_sample_flags.is_some() as u32 * 0x00_0004)
            | sample.to_box_flags();
        Some(flags)
    }
    fn box_payload_size(&self) -> Result<u32> {
        let size = track!(ByteCounter::calculate(|w| self.write_box_payload(w)))?;
        Ok(size as u32)
    }
    fn write_box_payload<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.samples.len() as u32);
        if let Some(x) = self.data_offset {
            write_i32!(writer, x);
        }
        if let Some(x) = self.first_sample_flags {
            write_u32!(writer, x.to_u32());
        }

        let mut sample_flags = None;
        for sample in &self.samples {
            if sample_flags.is_none() {
                sample_flags = Some(sample.to_box_flags());
            }
            track_assert_eq!(
                Some(sample.to_box_flags()),
                sample_flags,
                ErrorKind::InvalidInput
            );

            if let Some(x) = sample.duration {
                write_u32!(writer, x);
            }
            if let Some(x) = sample.size {
                write_u32!(writer, x);
            }
            if let Some(x) = sample.flags {
                write_u32!(writer, x.to_u32());
            }
            if let Some(x) = sample.composition_time_offset {
                write_i32!(writer, x);
            }
        }
        Ok(())
    }
}

/// 8.8.8.2 A sample (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Sample {
    pub duration: Option<u32>,
    pub size: Option<u32>,
    pub flags: Option<SampleFlags>,
    pub composition_time_offset: Option<i32>,
}
impl Sample {
    fn to_box_flags(&self) -> u32 {
        (self.duration.is_some() as u32 * 0x00_0100)
            | (self.size.is_some() as u32 * 0x00_0200)
            | (self.flags.is_some() as u32 * 0x00_0400)
            | (self.composition_time_offset.is_some() as u32 * 0x00_0800)
    }
}

/// 8.8.8.1 Flags for a sample (ISO/IEC 14496-12).
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SampleFlags {
    pub is_leading: u8,             // u2
    pub sample_depends_on: u8,      // u2
    pub sample_is_depdended_on: u8, // u2
    pub sample_has_redundancy: u8,  // u2
    pub sample_padding_value: u8,   // u3
    pub sample_is_non_sync_sample: bool,
    pub sample_degradation_priority: u16,
}
impl SampleFlags {
    fn to_u32(&self) -> u32 {
        (u32::from(self.is_leading) << 26)
            | (u32::from(self.sample_depends_on) << 24)
            | (u32::from(self.sample_is_depdended_on) << 22)
            | (u32::from(self.sample_has_redundancy) << 20)
            | (u32::from(self.sample_padding_value) << 17)
            | ((self.sample_is_non_sync_sample as u32) << 16)
            | u32::from(self.sample_degradation_priority)
    }
}
