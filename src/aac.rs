//! AAC related constituent elements.
use crate::{ErrorKind, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Read;

pub(crate) const SAMPLES_IN_FRAME: usize = 1024;

#[derive(Debug, Clone)]
pub(crate) struct AdtsHeader {
    pub profile: AacProfile,
    pub sampling_frequency: SamplingFrequency,
    pub private: bool,
    pub channel_configuration: ChannelConfiguration,
    pub frame_len: u16,       // u13
    pub buffer_fullness: u16, // u11
}
impl AdtsHeader {
    const SYNC_WORD: u16 = 0b1111_1111_1111;
    const MPEG_VERSION_4: u8 = 0;
    const HEADER_LEN_WITHOUT_CRC: u16 = 7;

    pub fn raw_data_blocks_len(&self) -> u16 {
        self.frame_len - Self::HEADER_LEN_WITHOUT_CRC
    }

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_u16::<BigEndian>())?;
        track_assert_eq!(n >> 4, Self::SYNC_WORD, ErrorKind::InvalidInput);

        let mpeg_version = ((n >> 3) & 0b1) as u8;
        let layer = (n >> 1) & 0b11;
        let crc_protection_absent = (n & 0b1) != 0;
        track_assert_eq!(mpeg_version, Self::MPEG_VERSION_4, ErrorKind::Unsupported);
        track_assert_eq!(layer, 0, ErrorKind::InvalidInput);

        let n = track_io!(reader.read_u8())?;
        let profile = match n >> 6 {
            0 => AacProfile::Main,
            1 => AacProfile::Lc,
            2 => AacProfile::Ssr,
            3 => AacProfile::Ltp,
            _ => unreachable!(),
        };
        let sampling_frequency = track!(SamplingFrequency::from_index((n >> 2) & 0b1111))?;
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
        let raw_data_blocks_minus_1 = n & 0b11;
        track_assert_eq!(raw_data_blocks_minus_1, 0, ErrorKind::Unsupported);
        if !crc_protection_absent {
            // 16bits
            track_panic!(ErrorKind::Unsupported);
        }

        Ok(AdtsHeader {
            profile,
            sampling_frequency,
            private,
            channel_configuration,
            frame_len,
            buffer_fullness,
        })
    }
}

/// Profile.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AacProfile {
    /// AAC Main.
    Main = 0,

    /// AAC LC (Low Complexity).
    Lc = 1,

    /// AAC SSR (Scalable Sample Rate).
    Ssr = 2,

    /// AAC LTP (Long Term Prediction).
    Ltp = 3,
}

/// Sampling frequency.
#[allow(missing_docs)]
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
    pub(crate) fn as_u32(&self) -> u32 {
        match *self {
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

    pub(crate) fn as_index(&self) -> u8 {
        *self as u8
    }

    fn from_index(n: u8) -> Result<Self> {
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

/// Channel configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelConfiguration {
    /// Channel configuration is sent via an inband PCE.
    SentViaInbandPce = 0,

    /// 1 channel: front-center.
    OneChannel = 1,

    /// 2 channels: front-left, front-right.
    TwoChannels = 2,

    /// 3 channels: front-center, front-left, front-right.
    ThreeChannels = 3,

    /// 4 channels: front-center, front-left, front-right, back-center.
    FourChannels = 4,

    /// 5 channels: front-center, front-left, front-right, back-left, back-right.
    FiveChannels = 5,

    /// 6 channels: front-center, front-left, front-right, back-left, back-right, LFE-channel.
    SixChannels = 6,

    /// 8 channels: front-center, front-left, front-right, side-left, side-right, back-left,
    /// back-right, LFE-channel.
    EightChannels = 7,
}
impl ChannelConfiguration {
    fn from_u8(n: u8) -> Result<Self> {
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
