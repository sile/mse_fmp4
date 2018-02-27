use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};

const SYNC_WORD: u16 = 0b1111_1111_1111;

#[derive(Debug, Clone)]
pub struct AdtsHeader {
    pub mpeg_version_id: MpegVersion,
    pub crc_protection_absent: bool,
    pub profile: AacProfile,
    pub sampling_frequency: SamplingFrequency,
    pub private: bool,
    pub channel_configuration: ChannelConfiguration,
    pub frame_len: u16,       // u13
    pub buffer_fullness: u16, // u11
    pub rdbs_minus_1: u8,     // u2
}
impl AdtsHeader {
    pub fn frame_len_exclude_header(&self) -> u16 {
        self.frame_len - if self.crc_protection_absent { 7 } else { 9 }
    }
    pub fn duration(&self) -> u32 {
        1024 * u32::from(self.rdbs_minus_1 + 1)
    }
    pub fn timescale(&self) -> u32 {
        match self.sampling_frequency {
            SamplingFrequency::Hz96000 => 96_000,
            SamplingFrequency::Hz88200 => 88_200,
            SamplingFrequency::Hz64000 => 64_000,
            SamplingFrequency::Hz48000 => 48_000,
            SamplingFrequency::Hz44100 => 44_100,
            SamplingFrequency::Hz32000 => 32_000,
            SamplingFrequency::Hz24000 => 24_000,
            SamplingFrequency::Hz22050 => 22_050,
            SamplingFrequency::Hz16000 => 16_000,
            SamplingFrequency::Hz12000 => 12_000,
            SamplingFrequency::Hz11025 => 11_025,
            SamplingFrequency::Hz8000 => 8_000,
            SamplingFrequency::Hz7350 => 7_350,
        }
    }

    pub fn to_audio_specific_config_u16(&self) -> u16 {
        let object_type = (self.profile as u16) + 1;
        let frequency_index = self.sampling_frequency as u16;
        let channel_configuration = self.channel_configuration as u16;
        let frame_len_flag = 0;
        let depends_on_core_coder = 0;
        let extension_flag = 0;
        (object_type << 11) | (frequency_index << 7) | (channel_configuration << 3)
            | (frame_len_flag << 2) | (depends_on_core_coder << 1) | extension_flag
    }

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_u16::<BigEndian>())?;
        track_assert_eq!(n >> 4, SYNC_WORD, ErrorKind::InvalidInput);

        let mpeg_version_id = if ((n >> 3) & 0b1) == 0 {
            MpegVersion::Mpeg4
        } else {
            MpegVersion::Mpeg2
        };
        let layer = (n >> 1) & 0b11;
        let crc_protection_absent = (n & 0b1) != 0;
        track_assert_eq!(mpeg_version_id, MpegVersion::Mpeg4, ErrorKind::Unsupported);
        track_assert_eq!(layer, 0, ErrorKind::InvalidInput);

        let n = track_io!(reader.read_u8())?;
        let profile = match n >> 6 {
            0 => AacProfile::Main,
            1 => AacProfile::Lc,
            2 => AacProfile::Ssr,
            3 => AacProfile::Ltp,
            _ => unreachable!(),
        };
        let sampling_frequency = track!(SamplingFrequency::from_u8((n >> 2) & 0b1111))?;
        let private = (n & 0b10) != 0;
        let channel_msb = n & 0b1;

        let n = track_io!(reader.read_u8())?;
        let channel_configuration =
            track!(ChannelConfiguration::from_u8((channel_msb << 2) | (n >> 6)))?;
        let originality = (n & 0b10_0000) != 0;
        let home = (n & 0b01_0000) != 0;
        let copyrighted = (n & 0b00_1000) != 0;
        let copyright_id_start = (n & 0b00_0100) != 0;
        track_assert!(!originality, ErrorKind::Unsupported);
        track_assert!(!home, ErrorKind::Unsupported);
        track_assert!(!copyrighted, ErrorKind::Unsupported);
        track_assert!(!copyright_id_start, ErrorKind::Unsupported);
        let frame_len_msb_2bits = u16::from(n & 0b11);

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        let frame_len = (frame_len_msb_2bits << 11) | (n >> 5);
        let buffer_fullness_msb_5bits = n & 0b1_1111;

        let n = track_io!(reader.read_u8())?;
        let buffer_fullness = (buffer_fullness_msb_5bits << 5) | u16::from(n >> 2);
        let rdbs_minus_1 = n & 0b11;
        if !crc_protection_absent {
            // 16bits
            track_panic!(ErrorKind::Unsupported);
        }

        Ok(AdtsHeader {
            mpeg_version_id,
            crc_protection_absent,
            profile,
            sampling_frequency,
            private,
            channel_configuration,
            frame_len,
            buffer_fullness,
            rdbs_minus_1,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MpegVersion {
    Mpeg4 = 0,
    Mpeg2 = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AacProfile {
    Main = 0,
    Lc = 1,
    Ssr = 2,
    Ltp = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplingFrequency {
    Hz96000 = 0,
    Hz88200 = 1,
    Hz64000 = 2,
    Hz48000 = 3,
    Hz44100 = 4,
    Hz32000 = 5,
    Hz24000 = 6,
    Hz22050 = 7,
    Hz16000 = 8,
    Hz12000 = 9,
    Hz11025 = 10,
    Hz8000 = 11,
    Hz7350 = 12,
}
impl SamplingFrequency {
    pub fn from_u8(n: u8) -> Result<Self> {
        Ok(match n {
            0 => SamplingFrequency::Hz96000,
            1 => SamplingFrequency::Hz88200,
            2 => SamplingFrequency::Hz64000,
            3 => SamplingFrequency::Hz48000,
            4 => SamplingFrequency::Hz44100,
            5 => SamplingFrequency::Hz32000,
            6 => SamplingFrequency::Hz24000,
            7 => SamplingFrequency::Hz22050,
            8 => SamplingFrequency::Hz16000,
            9 => SamplingFrequency::Hz12000,
            10 => SamplingFrequency::Hz11025,
            11 => SamplingFrequency::Hz8000,
            12 => SamplingFrequency::Hz7350,
            13 | 14 => track_panic!(ErrorKind::InvalidInput, "Reserved"),
            15 => track_panic!(ErrorKind::InvalidInput, "Forbidden"),
            _ => track_panic!(ErrorKind::InvalidInput, "Unreachable"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelConfiguration {
    SentViaInbandPce = 0,
    OneChannel = 1,
    TwoChannels = 2,
    ThreeChannels = 3,
    FourChannels = 4,
    FiveChannels = 5,
    SixChannels = 6,
    EightChannels = 7,
}
impl ChannelConfiguration {
    pub fn from_u8(n: u8) -> Result<Self> {
        Ok(match n {
            0 => ChannelConfiguration::SentViaInbandPce,
            1 => ChannelConfiguration::OneChannel,
            2 => ChannelConfiguration::TwoChannels,
            3 => ChannelConfiguration::ThreeChannels,
            4 => ChannelConfiguration::FourChannels,
            5 => ChannelConfiguration::FiveChannels,
            6 => ChannelConfiguration::SixChannels,
            7 => ChannelConfiguration::EightChannels,
            _ => track_panic!(ErrorKind::InvalidInput, "Unreachable"),
        })
    }
}
