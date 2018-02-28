use std::cmp;
use std::collections::HashMap;
use mpeg2ts;
use mpeg2ts::es::{StreamId, StreamType};
use mpeg2ts::pes::{PesPacket, PesPacketReader, ReadPesPacket};
use mpeg2ts::time::Timestamp;
use mpeg2ts::ts::{Pid, ReadTsPacket, TsPacket, TsPayload};

use {Error, ErrorKind, Result};
use avc::{AvcDecoderConfigurationRecord, ByteStreamFormatNalUnits, NalUnit, NalUnitType,
          SequenceParameterSet};
use fmp4::{InitializationSegment, MediaSegment};

pub fn to_fmp4<R: ReadTsPacket>(reader: R) -> Result<(InitializationSegment, MediaSegment)> {
    let (avc_stream, aac_stream) = track!(read_avc_aac_stream(reader))?;
    panic!()
}

#[derive(Debug)]
struct AvcStream {
    configuration: AvcDecoderConfigurationRecord,
    width: usize,
    height: usize,
    min_pts: Timestamp,
    max_pts: Timestamp,
    packets: Vec<PesPacket<Vec<u8>>>,
}
impl AvcStream {
    fn duration(&self) -> u64 {
        // TODO: handle wrap around
        self.max_pts.as_u64() - self.min_pts.as_u64()
    }
}

#[derive(Debug)]
struct AacStream {
    packets: Vec<PesPacket<Vec<u8>>>,
}

// impl AacStream {
//     fn duration(&self) -> u32 {
//         let mut duration = 0;
//         for p in &self.packets {
//             let header = track_try_unwrap!(aac::AdtsHeader::read_from(&p.data[..]));
//             duration += header.duration();
//         }
//         duration
//     }
//     fn timescale(&self) -> u32 {
//         // TDOO:
//         let header = track_try_unwrap!(aac::AdtsHeader::read_from(&self.packets[0].data[..]));
//         header.timescale()
//     }
//     fn channels(&self) -> u16 {
//         // TDOO:
//         let header = track_try_unwrap!(aac::AdtsHeader::read_from(&self.packets[0].data[..]));
//         header.channel_configuration as u16
//     }
//     fn adts_header(&self) -> aac::AdtsHeader {
//         // TDOO:
//         track_try_unwrap!(aac::AdtsHeader::read_from(&self.packets[0].data[..]))
//     }
// }

fn read_avc_aac_stream<R: ReadTsPacket>(ts_reader: R) -> Result<(AvcStream, AacStream)> {
    let mut avc_stream: Option<AvcStream> = None;
    let mut aac_stream: Option<AacStream> = None;

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
            track_assert!(pes.header.data_alignment_indicator, ErrorKind::Unsupported);

            let pts = track_assert_some!(pes.header.pts, ErrorKind::InvalidInput);
            if let Some(ref mut avc_stream) = avc_stream {
                avc_stream.min_pts = cmp::min(pts, avc_stream.min_pts);
                avc_stream.max_pts = cmp::min(pts, avc_stream.max_pts);
                avc_stream.packets.push(pes);
            } else {
                let mut sps = None;
                let mut pps = None;
                let mut stream_info = None;
                for nal_unit in track!(ByteStreamFormatNalUnits::new(&pes.data))? {
                    let nal_unit_type = track!(NalUnit::read_from(nal_unit))?.nal_unit_type;
                    match nal_unit_type {
                        NalUnitType::SequenceParameterSet => {
                            stream_info =
                                Some(track!(SequenceParameterSet::read_from(&nal_unit[1..]))?);
                            sps = Some(nal_unit.to_owned());
                        }
                        NalUnitType::PictureParameterSet => {
                            pps = Some(nal_unit.to_owned());
                        }
                        _ => {}
                    }
                }

                let stream_info = track_assert_some!(stream_info, ErrorKind::InvalidInput);
                let sps = track_assert_some!(sps, ErrorKind::InvalidInput);
                let pps = track_assert_some!(pps, ErrorKind::InvalidInput);
                avc_stream = Some(AvcStream {
                    configuration: AvcDecoderConfigurationRecord {
                        profile_idc: stream_info.profile_idc,
                        constraint_set_flag: stream_info.constraint_set_flag,
                        level_idc: stream_info.level_idc,
                        sequence_parameter_set: sps,
                        picture_parameter_set: pps,
                    },
                    width: stream_info.width(),
                    height: stream_info.height(),
                    min_pts: pts,
                    max_pts: pts,
                    packets: vec![pes],
                });
            }
        } else {
            track_assert!(pes.header.stream_id.is_audio(), ErrorKind::InvalidInput);
            track_assert_eq!(stream_type, StreamType::AdtsAac, ErrorKind::Unsupported);
            if let Some(ref mut aac_stream) = aac_stream {
                aac_stream.packets.push(pes);
            } else {
            }
        }
    }

    let avc_stream = track_assert_some!(avc_stream, ErrorKind::InvalidInput);
    let aac_stream = track_assert_some!(aac_stream, ErrorKind::InvalidInput);
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
                Some(TsPayload::Pmt(ref pmt)) => for es_info in &pmt.table {
                    self.pid_to_stream_type
                        .insert(es_info.elementary_pid, es_info.stream_type);
                },
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
