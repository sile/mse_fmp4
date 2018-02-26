use std::io::{Read, Write};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use {ErrorKind, Result};

#[derive(Debug)]
pub struct AvcDecoderConfigurationRecord {
    pub profile_idc: u8,
    pub constraint_set_flag: u8,
    pub level_idc: u8,
    pub sequence_parameter_set: Vec<u8>,
    pub picture_parameter_set: Vec<u8>,
}
impl AvcDecoderConfigurationRecord {
    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_io!(writer.write_u8(1))?; // configuration_version

        match self.profile_idc {
            100 | 110 | 122 | 144 => track_panic!(ErrorKind::Unsupported),
            _ => {}
        }
        track_io!(writer.write_u8(self.profile_idc))?;
        track_io!(writer.write_u8(self.constraint_set_flag))?;
        track_io!(writer.write_u8(self.level_idc))?;
        track_io!(writer.write_u8(0b1111_1100 | 0b0000_0011))?; // reserved and length_size_minus_one

        track_io!(writer.write_u8(0b1110_0000 | 0b0000_0001))?; // reserved and num_of_sequence_parameter_set_ext
        track_io!(writer.write_u16::<BigEndian>(self.sequence_parameter_set.len() as u16))?;
        track_io!(writer.write_all(&self.sequence_parameter_set))?;

        track_io!(writer.write_u8(0b0000_0000 | 0b0000_0001))?; // reserved and num_of_picture_parameter_set_ext
        track_io!(writer.write_u16::<BigEndian>(self.picture_parameter_set.len() as u16))?;
        track_io!(writer.write_all(&self.picture_parameter_set))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct SequenceParameterSet {
    pub profile_idc: u8,
    pub constraint_set_flag: u8,
    pub level_idc: u8,
    pub seq_parameter_set_id: u64,
    pub log2_max_frame_num_minus4: u64,
    pub pic_order_cnt_type: u64,
    pub num_ref_frames: u64,
    pub gaps_in_frame_num_value_allowed_flag: u8,
    pub pic_width_in_mbs_minus_1: u64,
    pub pic_height_in_map_units_minus_1: u64,
    // pub frame_mbs_only_flag: u8,
    // pub direct_8x8_inference_flag: u8,
    // pub frame_cropping_flag: u8,
    // pub vui_prameters_present_flag: u8,
    // pub rbsp_stop_one_bit: u8,
}
impl SequenceParameterSet {
    pub fn width(&self) -> usize {
        // TODO: ((pic_width_in_mbs_minus1 +1)*16) - frame_crop_right_offset*2 - frame_crop_left_offset*2;
        (self.pic_width_in_mbs_minus_1 as usize + 1) * 16
    }
    pub fn height(&self) -> usize {
        // TODO: ((2 - frame_mbs_only_flag)* (pic_height_in_map_units_minus1 +1) * 16) - (frame_crop_top_offset * 2) - (frame_crop_bottom_offset * 2);
        (self.pic_height_in_map_units_minus_1 as usize + 1) * 16
    }

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let profile_idc = track_io!(reader.read_u8())?;
        let constraint_set_flag = track_io!(reader.read_u8())?;
        let level_idc = track_io!(reader.read_u8())?;

        let mut reader = BitReader::new(reader);
        let seq_parameter_set_id = track!(reader.read_ue())?;

        match profile_idc {
            100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 => {
                track_panic!(ErrorKind::Unsupported, "profile_idc={}", profile_idc)
            }
            _ => {}
        }

        let log2_max_frame_num_minus4 = track!(reader.read_ue())?;
        let pic_order_cnt_type = track!(reader.read_ue())?;
        match pic_order_cnt_type {
            0 => {
                let _log2_max_pic_order_cnt_lsb_minus4 = track!(reader.read_ue())?;
            }
            1 => {
                let _delta_pic_order_always_zero_flag = track!(reader.read_bit())?;
                let _offset_for_non_ref_pic = track!(reader.read_ue())?; // TODO: se
                let _ffset_for_top_to_bottom_field = track!(reader.read_ue())?; // TODO: se
                let num_ref_frames_in_pic_order_cnt_cycle = track!(reader.read_ue())?;
                for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
                    let _offset_for_ref_frame = track!(reader.read_ue())?; // TODO: se
                }
            }
            _ => track_panic!(ErrorKind::InvalidInput),
        }
        let num_ref_frames = track!(reader.read_ue())?;
        let gaps_in_frame_num_value_allowed_flag = track!(reader.read_bit())?;
        let pic_width_in_mbs_minus_1 = track!(reader.read_ue())?;
        let pic_height_in_map_units_minus_1 = track!(reader.read_ue())?;
        // let frame_mbs_only_flag = track!(reader.read_bit())?;
        // if frame_mbs_only_flag == 1 {
        //     let _mb_adaptive_frame_field_flag = track!(reader.read_bit())?;
        // }
        // let direct_8x8_inference_flag = track!(reader.read_bit())?;
        // let frame_cropping_flag = track!(reader.read_bit())?;
        // track_assert_ne!(frame_cropping_flag, 1, ErrorKind::Unsupported);

        // let vui_prameters_present_flag = track!(reader.read_bit())?;
        // track_assert_ne!(vui_prameters_present_flag, 1, ErrorKind::Unsupported);
        // let rbsp_stop_one_bit = track!(reader.read_bit())?;

        Ok(SequenceParameterSet {
            profile_idc,
            constraint_set_flag,
            level_idc,
            seq_parameter_set_id,
            log2_max_frame_num_minus4,
            pic_order_cnt_type,
            num_ref_frames,
            gaps_in_frame_num_value_allowed_flag,
            pic_width_in_mbs_minus_1,
            pic_height_in_map_units_minus_1,
            // frame_mbs_only_flag,
            // direct_8x8_inference_flag,
            // frame_cropping_flag,
            // vui_prameters_present_flag,
            // rbsp_stop_one_bit,
        })
    }
}

#[derive(Debug)]
pub struct BitReader<R> {
    stream: R,
    byte: u8,
    bit_offset: usize,
}
impl<R: Read> BitReader<R> {
    fn new(stream: R) -> Self {
        BitReader {
            stream,
            byte: 0,
            bit_offset: 8,
        }
    }
    fn read_ue(&mut self) -> Result<u64> {
        track!(self.read_exp_golomb_code())
    }
    fn read_exp_golomb_code(&mut self) -> Result<u64> {
        let mut leading_zeros = 0;
        while 0 == track!(self.read_bit())? {
            leading_zeros += 1;
        }
        let mut n = 0;
        for _ in 0..leading_zeros {
            let bit = track!(self.read_bit())?;
            n = (n << 1) | u64::from(bit);
        }
        n += 2u64.pow(leading_zeros) - 1;
        Ok(n)
    }
    fn read_bit(&mut self) -> Result<u8> {
        if self.bit_offset == 8 {
            self.byte = track_io!(self.stream.read_u8())?;
            self.bit_offset = 0;
        }
        let bit = (self.byte >> (7 - self.bit_offset)) & 0b1;
        self.bit_offset += 1;
        Ok(bit)
    }
}

#[derive(Debug)]
pub struct NalUnit {
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
pub enum NalUnitType {
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
    pub fn from_u8(n: u8) -> Result<Self> {
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
pub struct ByteStreamFormatNalUnits<'a> {
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
            let mut end = self.bytes.len();
            let mut next_start = self.bytes.len();
            for i in 0..self.bytes.len() {
                if (&self.bytes[i..]).starts_with(&[0, 0, 0, 1][..]) {
                    end = i;
                    next_start = i + 4;
                    break;
                } else if (&self.bytes[i..]).starts_with(&[0, 0, 1][..]) {
                    end = i;
                    next_start = i + 3;
                    break;
                }
            }
            let nal_unit = &self.bytes[..end];
            self.bytes = &self.bytes[next_start..];
            Some(nal_unit)
        }
    }
}
