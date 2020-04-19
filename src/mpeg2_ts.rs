//! MPEG-2 TS related constituent elements.
use crate::aac::{self, AdtsHeader};
use crate::avc::{
    AvcDecoderConfigurationRecord, ByteStreamFormatNalUnits, NalUnit, NalUnitType, SpsSummary,
};
use crate::fmp4::{
    AacSampleEntry, AvcConfigurationBox, AvcSampleEntry, InitializationSegment, MediaDataBox,
    MediaSegment, MovieExtendsHeaderBox, Mp4Box, Mpeg4EsDescriptorBox, Sample, SampleEntry,
    SampleFlags, TrackBox, TrackExtendsBox, TrackFragmentBox,
};
use crate::io::ByteCounter;
use crate::{Error, ErrorKind, Result};
use byteorder::{BigEndian, WriteBytesExt};
use mpeg2ts;
use mpeg2ts::es::{StreamId, StreamType};
use mpeg2ts::pes::{PesPacketReader, ReadPesPacket};
use mpeg2ts::time::Timestamp;
use mpeg2ts::ts::{Pid, ReadTsPacket, TsPacket, TsPayload};
use std::cmp;
use std::collections::HashMap;
use std::io::Write;

/// Reads TS packets from `reader`, and converts them into fragmented MP4 segments.
pub fn to_fmp4<R: ReadTsPacket>(reader: R) -> Result<(InitializationSegment, MediaSegment)> {
    let (avc_stream, aac_stream) = track!(read_avc_aac_stream(reader))?;

    let initialization_segment = track!(make_initialization_segment(&avc_stream, &aac_stream))?;
    let media_segment = track!(make_media_segment(avc_stream, aac_stream))?;
    Ok((initialization_segment, media_segment))
}

fn make_initialization_segment(
    avc_stream: &AvcStream,
    aac_stream: &AacStream,
) -> Result<InitializationSegment> {
    let video_duration = track!(avc_stream.duration())?;
    let audio_duration = track!(aac_stream.duration())?;

    let mut segment = InitializationSegment::default();
    if video_duration < audio_duration {
        segment.moov_box.mvhd_box.timescale = aac::SAMPLES_IN_FRAME as u32;
        segment.moov_box.mvhd_box.duration = audio_duration;
        segment.moov_box.mvex_box.mehd_box = Some(MovieExtendsHeaderBox {
            fragment_duration: audio_duration,
        });
    } else {
        segment.moov_box.mvhd_box.timescale = Timestamp::RESOLUTION as u32;
        segment.moov_box.mvhd_box.duration = video_duration;
        segment.moov_box.mvex_box.mehd_box = Some(MovieExtendsHeaderBox {
            fragment_duration: video_duration,
        });
    }

    // video track
    let mut track = TrackBox::new(true);
    track.tkhd_box.width = (avc_stream.width as u32) << 16;
    track.tkhd_box.height = (avc_stream.height as u32) << 16;
    track.tkhd_box.duration = video_duration;
    track.edts_box.elst_box.media_time = avc_stream.start_time();
    track.mdia_box.mdhd_box.timescale = Timestamp::RESOLUTION as u32;
    track.mdia_box.mdhd_box.duration = video_duration;

    let avc_sample_entry = AvcSampleEntry {
        width: avc_stream.width as u16,
        height: avc_stream.height as u16,
        avcc_box: AvcConfigurationBox {
            configuration: avc_stream.configuration.clone(),
        },
    };
    track
        .mdia_box
        .minf_box
        .stbl_box
        .stsd_box
        .sample_entries
        .push(SampleEntry::Avc(avc_sample_entry));
    segment.moov_box.trak_boxes.push(track);
    segment
        .moov_box
        .mvex_box
        .trex_boxes
        .push(TrackExtendsBox::new(true));

    // audio track
    let mut track = TrackBox::new(false);
    track.tkhd_box.duration = audio_duration;
    track.mdia_box.mdhd_box.timescale = aac_stream.adts_header.sampling_frequency.as_u32();
    track.mdia_box.mdhd_box.duration = audio_duration;

    let aac_sample_entry = AacSampleEntry {
        esds_box: Mpeg4EsDescriptorBox {
            profile: aac_stream.adts_header.profile,
            frequency: aac_stream.adts_header.sampling_frequency,
            channel_configuration: aac_stream.adts_header.channel_configuration,
        },
    };
    track
        .mdia_box
        .minf_box
        .stbl_box
        .stsd_box
        .sample_entries
        .push(SampleEntry::Aac(aac_sample_entry));
    segment.moov_box.trak_boxes.push(track);
    segment
        .moov_box
        .mvex_box
        .trex_boxes
        .push(TrackExtendsBox::new(false));

    Ok(segment)
}

fn make_media_segment(avc_stream: AvcStream, aac_stream: AacStream) -> Result<MediaSegment> {
    let mut segment = MediaSegment::default();

    // video traf
    let mut traf = TrackFragmentBox::new(true);
    traf.tfhd_box.default_sample_flags = Some(SampleFlags {
        is_leading: 0,
        sample_depends_on: 1,
        sample_is_depdended_on: 0,
        sample_has_redundancy: 0,
        sample_padding_value: 0,
        sample_is_non_sync_sample: true,
        sample_degradation_priority: 0,
    });
    traf.trun_box.data_offset = Some(0); // dummy
    traf.trun_box.first_sample_flags = Some(SampleFlags {
        is_leading: 0,
        sample_depends_on: 2,
        sample_is_depdended_on: 0,
        sample_has_redundancy: 0,
        sample_padding_value: 0,
        sample_is_non_sync_sample: false,
        sample_degradation_priority: 0,
    });
    traf.trun_box.samples = avc_stream.samples;
    segment.moof_box.traf_boxes.push(traf);

    // audio traf
    let mut traf = TrackFragmentBox::new(false);
    traf.tfhd_box.default_sample_duration = Some(aac::SAMPLES_IN_FRAME as u32);
    traf.trun_box.data_offset = Some(0); // dummy
    traf.trun_box.samples = aac_stream.samples;
    segment.moof_box.traf_boxes.push(traf);

    // mdat and offsets adjustment
    let mut counter = ByteCounter::with_sink();
    track!(segment.moof_box.write_box(&mut counter))?;
    segment.moof_box.traf_boxes[0].trun_box.data_offset = Some(counter.count() as i32 + 8);

    segment.mdat_boxes.push(MediaDataBox {
        data: avc_stream.data,
    });
    track!(segment.mdat_boxes[0].write_box(&mut counter))?;

    segment.moof_box.traf_boxes[1].trun_box.data_offset = Some(counter.count() as i32 + 8);
    segment.mdat_boxes.push(MediaDataBox {
        data: aac_stream.data,
    });

    Ok(segment)
}

#[derive(Debug)]
struct AvcStream {
    configuration: AvcDecoderConfigurationRecord,
    width: usize,
    height: usize,
    samples: Vec<Sample>,
    data: Vec<u8>,
}
impl AvcStream {
    fn duration(&self) -> Result<u32> {
        let mut duration: u32 = 0;
        for sample in &self.samples {
            let sample_duration = track_assert_some!(sample.duration, ErrorKind::InvalidInput);
            duration = track_assert_some!(
                duration.checked_add(sample_duration),
                ErrorKind::InvalidInput
            );
        }
        Ok(duration)
    }
    fn start_time(&self) -> i32 {
        self.samples
            .first()
            .and_then(|s| s.composition_time_offset)
            .unwrap_or(0)
    }
}

#[derive(Debug)]
struct AacStream {
    adts_header: AdtsHeader,
    samples: Vec<Sample>,
    data: Vec<u8>,
}
impl AacStream {
    fn duration(&self) -> Result<u32> {
        let duration = track_assert_some!(
            (aac::SAMPLES_IN_FRAME as u32).checked_mul(self.samples.len() as u32),
            ErrorKind::InvalidInput
        );
        Ok(duration)
    }
}

fn read_avc_aac_stream<R: ReadTsPacket>(ts_reader: R) -> Result<(AvcStream, AacStream)> {
    let mut avc_stream: Option<AvcStream> = None;
    let mut aac_stream: Option<AacStream> = None;
    let mut avc_timestamps = Vec::new();
    let mut avc_timestamp_offset = 0;

    let mut reader = PesPacketReader::new(TsPacketReader::new(ts_reader));
    while let Some(pes) = track!(reader.read_pes_packet().map_err(Error::from))? {
        let stream_type = track_assert_some!(
            reader
                .ts_packet_reader()
                .get_stream_type(pes.header.stream_id),
            ErrorKind::InvalidInput
        );
        if pes.header.stream_id.is_video() {
            track_assert_eq!(stream_type, StreamType::H264, ErrorKind::Unsupported);

            let pts = track_assert_some!(pes.header.pts, ErrorKind::InvalidInput);
            let dts = pes.header.dts.unwrap_or(pts);

            let i = avc_timestamps.len();
            let mut timestamp = pts.as_u64();
            if i == 0 {
                avc_timestamp_offset = timestamp;
            }
            if timestamp < avc_timestamp_offset {
                timestamp += Timestamp::MAX;
            }
            avc_timestamps.push((timestamp - avc_timestamp_offset, i));

            if avc_stream.is_none() {
                let mut sps = None;
                let mut pps = None;
                let mut sps_summary = None;
                for nal_unit in track!(ByteStreamFormatNalUnits::new(&pes.data))? {
                    let nal_unit_type = track!(NalUnit::read_from(nal_unit))?.nal_unit_type;
                    match nal_unit_type {
                        NalUnitType::SequenceParameterSet => {
                            sps_summary = Some(track!(SpsSummary::read_from(&nal_unit[1..]))?);
                            sps = Some(nal_unit.to_owned());
                        }
                        NalUnitType::PictureParameterSet => {
                            pps = Some(nal_unit.to_owned());
                        }
                        _ => {}
                    }
                }

                let sps_summary = track_assert_some!(sps_summary, ErrorKind::InvalidInput);
                let sps = track_assert_some!(sps, ErrorKind::InvalidInput);
                let pps = track_assert_some!(pps, ErrorKind::InvalidInput);
                avc_stream = Some(AvcStream {
                    configuration: AvcDecoderConfigurationRecord {
                        profile_idc: sps_summary.profile_idc,
                        constraint_set_flag: sps_summary.constraint_set_flag,
                        level_idc: sps_summary.level_idc,
                        sequence_parameter_set: sps,
                        picture_parameter_set: pps,
                    },
                    width: sps_summary.width(),
                    height: sps_summary.height(),
                    samples: Vec::new(),
                    data: Vec::new(),
                });
            }

            let avc_stream = avc_stream.as_mut().expect("Never fails");
            let prev_data_len = avc_stream.data.len();
            for nal_unit in track!(ByteStreamFormatNalUnits::new(&pes.data))? {
                avc_stream
                    .data
                    .write_u32::<BigEndian>(nal_unit.len() as u32)
                    .unwrap();
                avc_stream.data.write_all(nal_unit).unwrap();
            }

            let sample_size = (avc_stream.data.len() - prev_data_len) as u32;
            let sample_composition_time_offset = (pts.as_u64() as i64 - dts.as_u64() as i64) as i32;
            avc_stream.samples.push(Sample {
                duration: None, // dummy
                size: Some(sample_size),
                flags: None,
                composition_time_offset: Some(sample_composition_time_offset),
            });
        } else {
            track_assert!(pes.header.stream_id.is_audio(), ErrorKind::InvalidInput);
            track_assert_eq!(stream_type, StreamType::AdtsAac, ErrorKind::Unsupported);
            if aac_stream.is_none() {
                let adts_header = track!(AdtsHeader::read_from(&pes.data[..]))?;
                aac_stream = Some(AacStream {
                    adts_header,
                    samples: Vec::new(),
                    data: Vec::new(),
                });
            }

            let aac_stream = aac_stream.as_mut().expect("Never fails");
            let mut bytes = &pes.data[..];
            while !bytes.is_empty() {
                let header = track!(AdtsHeader::read_from(&mut bytes))?;

                let sample_size = header.raw_data_blocks_len();
                aac_stream.samples.push(Sample {
                    duration: None,
                    size: Some(u32::from(sample_size)),
                    flags: None,
                    composition_time_offset: None,
                });
                aac_stream
                    .data
                    .extend_from_slice(&bytes[..sample_size as usize]);
                bytes = &bytes[sample_size as usize..];
            }
        }
    }

    let mut avc_stream = track_assert_some!(avc_stream, ErrorKind::InvalidInput);
    let aac_stream = track_assert_some!(aac_stream, ErrorKind::InvalidInput);

    avc_timestamps.sort();
    for (&(curr, _), &(next, i)) in avc_timestamps.iter().zip(avc_timestamps.iter().skip(1)) {
        let duration = next - curr;
        avc_stream.samples[i].duration = Some(duration as u32);
    }
    if !avc_stream.samples.is_empty() {
        avc_stream.samples[0].duration = Some(cmp::max(0, avc_stream.start_time()) as u32);
    }

    Ok((avc_stream, aac_stream))
}

#[derive(Debug)]
struct TsPacketReader<R> {
    inner: R,
    pid_to_stream_type: HashMap<Pid, StreamType>,
    stream_id_to_pid: HashMap<StreamId, Pid>,
}
impl<R> TsPacketReader<R> {
    fn new(inner: R) -> Self {
        TsPacketReader {
            inner,
            pid_to_stream_type: HashMap::new(),
            stream_id_to_pid: HashMap::new(),
        }
    }
    fn get_stream_type(&self, stream_id: StreamId) -> Option<StreamType> {
        self.stream_id_to_pid
            .get(&stream_id)
            .and_then(|pid| self.pid_to_stream_type.get(pid))
            .cloned()
    }
}
impl<R: ReadTsPacket> ReadTsPacket for TsPacketReader<R> {
    fn read_ts_packet(&mut self) -> mpeg2ts::Result<Option<TsPacket>> {
        if let Some(packet) = track!(self.inner.read_ts_packet())? {
            match packet.payload {
                Some(TsPayload::Pmt(ref pmt)) => {
                    for es_info in &pmt.table {
                        self.pid_to_stream_type
                            .insert(es_info.elementary_pid, es_info.stream_type);
                    }
                }
                Some(TsPayload::Pes(ref pes)) => {
                    if self.pid_to_stream_type.contains_key(&packet.header.pid) {
                        self.stream_id_to_pid
                            .insert(pes.header.stream_id, packet.header.pid);
                    }
                }
                _ => {}
            }
            Ok(Some(packet))
        } else {
            Ok(None)
        }
    }
}
