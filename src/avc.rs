//! AVC (H.264) related constituent elements.
use std::io::{Read, Write};
use byteorder::ReadBytesExt;

use {ErrorKind, Result};
use io::AvcBitReader;

/// AVC decoder configuration record.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct AvcDecoderConfigurationRecord {
    pub profile_idc: u8,
    pub constraint_set_flag: u8,
    pub level_idc: u8,
    pub sequence_parameter_set: Vec<u8>,
    pub picture_parameter_set: Vec<u8>,
}
impl AvcDecoderConfigurationRecord {
    pub(crate) fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u8!(writer, 1); // configuration_version

        match self.profile_idc {
            100 | 110 | 122 | 144 => track_panic!(ErrorKind::Unsupported),
            _ => {}
        }
        write_u8!(writer, self.profile_idc);
        write_u8!(writer, self.constraint_set_flag);
        write_u8!(writer, self.level_idc);
        write_u8!(writer, 0b1111_1100 | 0b0000_0011); // reserved and length_size_minus_one

        write_u8!(writer, 0b1110_0000 | 0b0000_0001); // reserved and num_of_sequence_parameter_set_ext
        write_u16!(writer, self.sequence_parameter_set.len() as u16);
        write_all!(writer, &self.sequence_parameter_set);

        write_u8!(writer, 0b0000_0001); // num_of_picture_parameter_set_ext
        write_u16!(writer, self.picture_parameter_set.len() as u16);
        write_all!(writer, &self.picture_parameter_set);
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SpsSummary {
    pub profile_idc: u8,
    pub constraint_set_flag: u8,
    pub level_idc: u8,
    pic_width_in_mbs_minus_1: u64,
    pic_height_in_map_units_minus_1: u64,
    frame_mbs_only_flag: u8,
    frame_crop_left_offset: u64,
    frame_crop_right_offset: u64,
    frame_crop_top_offset: u64,
    frame_crop_bottom_offset: u64,
}
impl SpsSummary {
    pub fn width(&self) -> usize {
        (self.pic_width_in_mbs_minus_1 as usize + 1) * 16
            - (self.frame_crop_right_offset as usize * 2)
            - (self.frame_crop_left_offset as usize * 2)
    }

    pub fn height(&self) -> usize {
        (2 - self.frame_mbs_only_flag as usize)
            * ((self.pic_height_in_map_units_minus_1 as usize + 1) * 16)
            - (self.frame_crop_bottom_offset as usize * 2)
            - (self.frame_crop_top_offset as usize * 2)
    }

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let profile_idc = track_io!(reader.read_u8())?;
        let constraint_set_flag = track_io!(reader.read_u8())?;
        let level_idc = track_io!(reader.read_u8())?;

        let mut reader = AvcBitReader::new(reader);
        let _seq_parameter_set_id = track!(reader.read_ue())?;

        match profile_idc {
            100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 => {
                track_panic!(ErrorKind::Unsupported, "profile_idc={}", profile_idc)
            }
            _ => {}
        }

        let _log2_max_frame_num_minus4 = track!(reader.read_ue())?;
        let pic_order_cnt_type = track!(reader.read_ue())?;
        match pic_order_cnt_type {
            0 => {
                let _log2_max_pic_order_cnt_lsb_minus4 = track!(reader.read_ue())?;
            }
            1 => {
                let _delta_pic_order_always_zero_flag = track!(reader.read_bit())?;
                let _offset_for_non_ref_pic = track!(reader.read_ue())?;
                let _ffset_for_top_to_bottom_field = track!(reader.read_ue())?;
                let num_ref_frames_in_pic_order_cnt_cycle = track!(reader.read_ue())?;
                for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
                    let _offset_for_ref_frame = track!(reader.read_ue())?;
                }
            }
            _ => track_panic!(ErrorKind::InvalidInput),
        }
        let _num_ref_frames = track!(reader.read_ue())?;
        let _gaps_in_frame_num_value_allowed_flag = track!(reader.read_bit())?;
        let pic_width_in_mbs_minus_1 = track!(reader.read_ue())?;
        let pic_height_in_map_units_minus_1 = track!(reader.read_ue())?;
        let frame_mbs_only_flag = track!(reader.read_bit())?;
        if frame_mbs_only_flag == 0 {
            let _mb_adaptive_frame_field_flag = track!(reader.read_bit())?;
        }
        let _direct_8x8_inference_flag = track!(reader.read_bit())?;
        let frame_cropping_flag = track!(reader.read_bit())?;
        let (
            frame_crop_left_offset,
            frame_crop_right_offset,
            frame_crop_top_offset,
            frame_crop_bottom_offset,
        ) = if frame_cropping_flag == 1 {
            (
                track!(reader.read_ue())?,
                track!(reader.read_ue())?,
                track!(reader.read_ue())?,
                track!(reader.read_ue())?,
            )
        } else {
            (0, 0, 0, 0)
        };

        Ok(SpsSummary {
            profile_idc,
            constraint_set_flag,
            level_idc,
            pic_width_in_mbs_minus_1,
            pic_height_in_map_units_minus_1,
            frame_mbs_only_flag,
            frame_crop_left_offset,
            frame_crop_right_offset,
            frame_crop_top_offset,
            frame_crop_bottom_offset,
        })
    }
}

#[derive(Debug)]
pub(crate) struct NalUnit {
    pub nal_ref_idc: u8,
    pub nal_unit_type: NalUnitType,
}
impl NalUnit {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let b = track_io!(reader.read_u8())?;

        let nal_ref_idc = (b >> 5) & 0b11;
        let nal_unit_type = track!(NalUnitType::from_u8(b & 0b1_1111))?;
        Ok(NalUnit {
            nal_ref_idc,
            nal_unit_type,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NalUnitType {
    CodedSliceOfANonIdrPicture = 1,
    CodedSliceDataPartitionA = 2,
    CodedSliceDataPartitionB = 3,
    CodedSliceDataPartitionC = 4,
    CodedSliceOfAnIdrPicture = 5,
    SupplementalEnhancementInformation = 6,
    SequenceParameterSet = 7,
    PictureParameterSet = 8,
    AccessUnitDelimiter = 9,
    EndOfSequence = 10,
    EndOfStream = 11,
    FilterData = 12,
    SequenceParameterSetExtension = 13,
    PrefixNalUnit = 14,
    SubsetSequenceParameterSet = 15,
    CodedSliceOfAnAuxiliaryCodedPictureWithoutPartitioning = 19,
    CodedSliceExtension = 20,
}
impl NalUnitType {
    fn from_u8(n: u8) -> Result<Self> {
        Ok(match n {
            1 => NalUnitType::CodedSliceOfANonIdrPicture,
            2 => NalUnitType::CodedSliceDataPartitionA,
            3 => NalUnitType::CodedSliceDataPartitionB,
            4 => NalUnitType::CodedSliceDataPartitionC,
            5 => NalUnitType::CodedSliceOfAnIdrPicture,
            6 => NalUnitType::SupplementalEnhancementInformation,
            7 => NalUnitType::SequenceParameterSet,
            8 => NalUnitType::PictureParameterSet,
            9 => NalUnitType::AccessUnitDelimiter,
            10 => NalUnitType::EndOfSequence,
            11 => NalUnitType::EndOfStream,
            12 => NalUnitType::FilterData,
            13 => NalUnitType::SequenceParameterSetExtension,
            14 => NalUnitType::PrefixNalUnit,
            15 => NalUnitType::SubsetSequenceParameterSet,
            19 => NalUnitType::CodedSliceOfAnAuxiliaryCodedPictureWithoutPartitioning,
            20 => NalUnitType::CodedSliceExtension,
            _ => track_panic!(ErrorKind::InvalidInput),
        })
    }
}

#[derive(Debug)]
pub(crate) struct ByteStreamFormatNalUnits<'a> {
    bytes: &'a [u8],
}
impl<'a> ByteStreamFormatNalUnits<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        let bytes = if bytes.starts_with(&[0, 0, 1][..]) {
            &bytes[3..]
        } else if bytes.starts_with(&[0, 0, 0, 1][..]) {
            &bytes[4..]
        } else {
            track_panic!(ErrorKind::InvalidInput);
        };
        Ok(ByteStreamFormatNalUnits { bytes })
    }
}
impl<'a> Iterator for ByteStreamFormatNalUnits<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes.is_empty() {
            None
        } else {
            let mut nal_unit_end = self.bytes.len();
            let mut next_start = self.bytes.len();
            for i in 0..self.bytes.len() {
                if (&self.bytes[i..]).starts_with(&[0, 0, 0, 1][..]) {
                    nal_unit_end = i;
                    next_start = i + 4;
                    break;
                } else if (&self.bytes[i..]).starts_with(&[0, 0, 1][..]) {
                    nal_unit_end = i;
                    next_start = i + 3;
                    break;
                }
            }
            let nal_unit = &self.bytes[..nal_unit_end];
            self.bytes = &self.bytes[next_start..];
            Some(nal_unit)
        }
    }
}
