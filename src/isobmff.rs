use std::ffi::CString;
use std::fmt;
use std::io::{Read, Write};
use std::str;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use trackable::error::ErrorKindExt;

use {ErrorKind, Result};
use fmp4::WriteTo;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoxHeader {
    pub size: u32,
    pub kind: BoxType,
}
impl BoxHeader {
    const SIZE: u32 = 8;

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let size = track_io!(reader.read_u32::<BigEndian>())?;
        let kind = track!(BoxType::read_from(&mut reader))?;
        track_assert_ne!(size, 1, ErrorKind::Unsupported);
        track_assert_ne!(size, 0, ErrorKind::Unsupported);
        track_assert!(size >= Self::SIZE, ErrorKind::InvalidInput);

        track_assert_ne!(&kind.0, b"uuid", ErrorKind::Unsupported);

        Ok(BoxHeader { size, kind })
    }

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
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_u32::<BigEndian>())?;
        let version = (n >> 24) as u8;
        let flags = n & 0xFF_FFFF;
        Ok(FullBoxHeader { version, flags })
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
impl BoxType {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut buf = [0; 4];
        track_io!(reader.read_exact(&mut buf[..]))?;
        Ok(BoxType(buf))
    }
}
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
impl Brand {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut buf = [0; 4];
        track_io!(reader.read_exact(&mut buf[..]))?;
        Ok(Brand(buf))
    }
}
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
pub struct HandlerType(pub [u8; 4]);
impl HandlerType {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut buf = [0; 4];
        track_io!(reader.read_exact(&mut buf[..]))?;
        Ok(HandlerType(buf))
    }
}
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
pub struct FileTypeBox {
    pub major_brand: Brand,
    pub minor_version: u32,
    pub compatible_brands: Vec<Brand>,
}
impl FileTypeBox {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let major_brand = track!(Brand::read_from(&mut reader))?;
        let minor_version = track_io!(reader.read_u32::<BigEndian>())?;
        let mut compatible_brands = Vec::new();

        let mut peek = [0];
        while 0 != track_io!(reader.read(&mut peek))? {
            compatible_brands.push(track!(Brand::read_from(peek.chain(&mut reader)))?);
        }

        Ok(FileTypeBox {
            major_brand,
            minor_version,
            compatible_brands,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaDataBox {
    pub data: Vec<u8>,
}
impl MediaDataBox {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut data = Vec::new();
        track_io!(reader.read_to_end(&mut data))?;
        Ok(MediaDataBox { data })
    }
}

fn each_boxes<R: Read, F>(mut reader: R, mut f: F) -> Result<()>
where
    F: FnMut(BoxType, &mut ::std::io::Take<&mut R>) -> Result<()>,
{
    let mut peek = [0];
    while 1 == track_io!(reader.read(&mut peek))? {
        let header = track!(BoxHeader::read_from(peek.chain(&mut reader)))?;
        let mut reader = reader.by_ref().take(u64::from(header.data_size()));
        track!(f(header.kind, reader.by_ref()))?;
        track_assert_eq!(reader.limit(), 0, ErrorKind::InvalidInput);
    }
    Ok(())
}

pub struct File {
    pub ftyp_box: FileTypeBox,
    pub moov_box: MoovBox,
    pub mdat_boxes: Vec<MediaDataBox>,
    pub moof_boxes: Vec<MoofBox>,
    pub mfra_box: Option<MfraBox>,
}
impl File {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut ftyp_box = None;
        let mut moov_box = None;
        let mut mdat_boxes = Vec::new();
        let mut moof_boxes = Vec::new();
        let mut mfra_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"ftyp" => {
                track_assert!(ftyp_box.is_none(), ErrorKind::InvalidInput);
                let x = track!(FileTypeBox::read_from(reader))?;
                println!("[ftyp] {:?}", x);
                ftyp_box = Some(x);
                Ok(())
            }
            b"moov" => {
                track_assert!(moov_box.is_none(), ErrorKind::InvalidInput);
                println!("[moov]");
                let x = track!(MoovBox::read_from(reader))?;
                moov_box = Some(x);
                Ok(())
            }
            b"mdat" => {
                let x = track!(MediaDataBox::read_from(reader))?;
                println!("[mdat] {} bytes", x.data.len());
                mdat_boxes.push(x);
                Ok(())
            }
            b"moof" => {
                moof_boxes.push(track!(MoofBox::read_from(reader))?);
                Ok(())
            }
            b"mfra" => read_exactly_one(reader, &mut mfra_box),
            _ => {
                println!("[todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        let ftyp_box = track_assert_some!(ftyp_box, ErrorKind::InvalidInput);
        let moov_box = track_assert_some!(moov_box, ErrorKind::InvalidInput);
        Ok(File {
            ftyp_box,
            moov_box,
            mdat_boxes,
            moof_boxes,
            mfra_box,
        })
    }
}
impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "File {{ ... }}")
    }
}

/// Movie Box.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MoovBox {
    pub mvhd_box: MvhdBox,
    pub trak_boxes: Vec<TrakBox>,
    pub mvex_box: Option<MvexBox>,
}
impl MoovBox {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut mvhd_box = None;
        let mut trak_boxes = Vec::new();
        let mut mvex_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"mvhd" => {
                track_assert!(mvhd_box.is_none(), ErrorKind::InvalidInput);
                let x = track!(MvhdBox::read_from(reader))?;
                println!("    [mvhd] {:?}", x);
                mvhd_box = Some(x);
                Ok(())
            }
            b"trak" => {
                println!("    [trak]");
                let x = track!(TrakBox::read_from(reader))?;
                trak_boxes.push(x);
                Ok(())
            }
            b"mvex" => track!(read_exactly_one(reader, &mut mvex_box)),
            _ => {
                println!("    [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        let mvhd_box = track_assert_some!(mvhd_box, ErrorKind::InvalidInput);
        track_assert!(!trak_boxes.is_empty(), ErrorKind::InvalidInput);
        Ok(MoovBox {
            mvhd_box,
            trak_boxes,
            mvex_box,
        })
    }
}

/// Movie Header Box.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MvhdBox {
    pub creation_time: u64,
    pub modification_time: u64,
    pub timescale: u32,
    pub duration: u64,
    pub rate: i32,   // fixed point 16.16
    pub volume: i16, // fixed point 8.8
    pub matrix: [i32; 9],
    pub next_track_id: u32, // 0xFFFF_FFFF means ...
}
impl MvhdBox {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;

        let creation_time;
        let modification_time;
        let timescale;
        let duration;
        if header.version == 0 {
            creation_time = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
            modification_time = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
            timescale = track_io!(reader.read_u32::<BigEndian>())?;
            duration = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
        } else if header.version == 1 {
            creation_time = track_io!(reader.read_u64::<BigEndian>())?;
            modification_time = track_io!(reader.read_u64::<BigEndian>())?;
            timescale = track_io!(reader.read_u32::<BigEndian>())?;
            duration = track_io!(reader.read_u64::<BigEndian>())?;
        } else {
            track_panic!(ErrorKind::Unsupported, "version={}", header.version);
        }
        let rate = track_io!(reader.read_i32::<BigEndian>())?;
        let volume = track_io!(reader.read_i16::<BigEndian>())?;
        let _ = track_io!(reader.read_u16::<BigEndian>())?; // reserved
        let _ = track_io!(reader.read_u64::<BigEndian>())?; // reserved

        let mut matrix = [0; 9];
        for i in 0..9 {
            matrix[i] = track_io!(reader.read_i32::<BigEndian>())?;
        }
        let _ = track_io!(reader.read_exact(&mut [0; 4 * 6]))?; // pre_defined
        let next_track_id = track_io!(reader.read_u32::<BigEndian>())?;
        Ok(MvhdBox {
            creation_time,
            modification_time,
            timescale,
            duration,
            rate,
            volume,
            matrix,
            next_track_id,
        })
    }
}

/// 8.8.1 Movie Extends Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MvexBox {
    pub mehd_box: Option<MehdBox>,
    pub trex_boxes: Vec<TrexBox>,
}
impl ReadFrom for MvexBox {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
        println!("    [mvex]");

        let mut mehd_box = None;
        let mut trex_boxes = Vec::new();
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"mehd" => track!(read_exactly_one(reader, &mut mehd_box)),
            b"trex" => {
                trex_boxes.push(track!(TrexBox::read_from(reader))?);
                Ok(())
            }
            _ => {
                println!("        [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        Ok(MvexBox {
            mehd_box,
            trex_boxes,
        })
    }
}

/// 8.8.2 Movie Extends Header Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MehdBox {
    pub fragment_duration: u64,
}
impl ReadFrom for MehdBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;

        let fragment_duration;
        if header.version == 0 {
            fragment_duration = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
        } else if header.version == 1 {
            fragment_duration = track_io!(reader.read_u64::<BigEndian>())?;
        } else {
            track_panic!(ErrorKind::InvalidInput, "version={}", header.version);
        }

        let this = MehdBox { fragment_duration };
        println!("        [mehd] {:?}", this);
        Ok(this)
    }
}

/// 8.8.3 Track Extends Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrexBox {
    pub track_id: u32,
    pub default_sample_description_index: u32,
    pub default_sample_duration: u32,
    pub default_sample_size: u32,
    pub default_sample_flags: u32,
}
impl ReadFrom for TrexBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::InvalidInput);

        let track_id = track_io!(reader.read_u32::<BigEndian>())?;
        let default_sample_description_index = track_io!(reader.read_u32::<BigEndian>())?;
        let default_sample_duration = track_io!(reader.read_u32::<BigEndian>())?;
        let default_sample_size = track_io!(reader.read_u32::<BigEndian>())?;
        let default_sample_flags = track_io!(reader.read_u32::<BigEndian>())?;

        let this = TrexBox {
            track_id,
            default_sample_description_index,
            default_sample_duration,
            default_sample_size,
            default_sample_flags,
        };
        println!("        [trex] {:?}", this);
        Ok(this)
    }
}

/// 8.3.1 Track Box.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrakBox {
    pub tkhd_box: TkhdBox,
    pub mdia_box: MdiaBox,
    pub edts_box: Option<EdtsBox>,
}
impl TrakBox {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut tkhd_box = None;
        let mut mdia_box = None;
        let mut edts_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"tkhd" => {
                track_assert!(tkhd_box.is_none(), ErrorKind::InvalidInput);
                let x = track!(TkhdBox::read_from(reader))?;
                println!("        [tkhd] {:?}", x);
                tkhd_box = Some(x);
                Ok(())
            }
            b"mdia" => {
                track_assert!(mdia_box.is_none(), ErrorKind::InvalidInput);
                println!("        [mdia]");
                let x = track!(MdiaBox::read_from(reader))?;
                mdia_box = Some(x);
                Ok(())
            }
            b"edts" => track!(read_exactly_one(reader, &mut edts_box)),
            _ => {
                println!("        [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        let tkhd_box = track_assert_some!(tkhd_box, ErrorKind::InvalidInput);
        let mdia_box = track_assert_some!(mdia_box, ErrorKind::InvalidInput);
        Ok(TrakBox {
            tkhd_box,
            mdia_box,
            edts_box,
        })
    }
}

/// 8.3.2 Track Header Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TkhdBox {
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
impl TkhdBox {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;

        let creation_time;
        let modification_time;
        let track_id;
        let duration;
        if header.version == 0 {
            creation_time = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
            modification_time = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
            track_id = track_io!(reader.read_u32::<BigEndian>())?;
            let _ = track_io!(reader.read_exact(&mut [0; 4][..]))?; // reserved
            duration = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
        } else if header.version == 1 {
            creation_time = track_io!(reader.read_u64::<BigEndian>())?;
            modification_time = track_io!(reader.read_u64::<BigEndian>())?;
            track_id = track_io!(reader.read_u32::<BigEndian>())?;
            let _ = track_io!(reader.read_exact(&mut [0; 4][..]))?; // reserved
            duration = track_io!(reader.read_u64::<BigEndian>())?;
        } else {
            track_panic!(ErrorKind::Unsupported, "version={}", header.version);
        }

        let _ = track_io!(reader.read_exact(&mut [0; 8][..]))?; // reserved
        let layer = track_io!(reader.read_i16::<BigEndian>())?;
        let alternate_group = track_io!(reader.read_i16::<BigEndian>())?;
        let volume = track_io!(reader.read_i16::<BigEndian>())?;
        let _ = track_io!(reader.read_exact(&mut [0; 2][..]))?; // reserved

        let mut matrix = [0; 9];
        for i in 0..9 {
            matrix[i] = track_io!(reader.read_i32::<BigEndian>())?;
        }

        let width = track_io!(reader.read_u32::<BigEndian>())?;
        let height = track_io!(reader.read_u32::<BigEndian>())?;
        Ok(TkhdBox {
            track_enabled: (header.flags & 0x00_0001) != 0,
            track_in_movie: (header.flags & 0x00_0002) != 0,
            track_in_preview: (header.flags & 0x00_0004) != 0,
            track_size_is_aspect_ratio: (header.flags & 0x00_0008) != 0,
            creation_time,
            modification_time,
            track_id,
            duration,
            layer,
            alternate_group,
            volume,
            matrix,
            width,
            height,
        })
    }
}

/// 8.4.1 Media Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MdiaBox {
    pub mdhd_box: MdhdBox,
    pub hdlr_box: HdlrBox,
    pub minf_box: MinfBox,
}
impl MdiaBox {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut mdhd_box = None;
        let mut hdlr_box = None;
        let mut minf_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"mdhd" => track!(read_exactly_one(reader, &mut mdhd_box)),
            b"hdlr" => track!(read_exactly_one(reader, &mut hdlr_box)),
            b"minf" => track!(read_exactly_one(reader, &mut minf_box)),
            _ => {
                println!("            [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        Ok(MdiaBox {
            mdhd_box: track_assert_some!(mdhd_box, ErrorKind::InvalidInput),
            hdlr_box: track_assert_some!(hdlr_box, ErrorKind::InvalidInput),
            minf_box: track_assert_some!(minf_box, ErrorKind::InvalidInput),
        })
    }
}

pub trait ReadFrom: Sized {
    fn read_from<R: Read>(reader: R) -> Result<Self>;
}

fn read_exactly_one<R: Read, T: ReadFrom>(reader: R, t: &mut Option<T>) -> Result<()> {
    track_assert!(t.is_none(), ErrorKind::InvalidInput);
    *t = Some(track!(T::read_from(reader))?);
    Ok(())
}

/// 8.4.2 Media Header Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MdhdBox {
    pub creation_time: u64,
    pub modification_time: u64,
    pub timescale: u32,
    pub duration: u64,
    pub language: u16,
}
impl ReadFrom for MdhdBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;

        let creation_time;
        let modification_time;
        let timescale;
        let duration;
        if header.version == 0 {
            creation_time = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
            modification_time = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
            timescale = track_io!(reader.read_u32::<BigEndian>())?;
            duration = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
        } else if header.version == 1 {
            creation_time = track_io!(reader.read_u64::<BigEndian>())?;
            modification_time = track_io!(reader.read_u64::<BigEndian>())?;
            timescale = track_io!(reader.read_u32::<BigEndian>())?;
            duration = track_io!(reader.read_u64::<BigEndian>())?;
        } else {
            track_panic!(ErrorKind::Unsupported, "version={}", header.version);
        }

        let language = track_io!(reader.read_u16::<BigEndian>())?;
        let _ = track_io!(reader.read_exact(&mut [0; 2][..]))?; // pre_defined
        let this = MdhdBox {
            creation_time,
            modification_time,
            timescale,
            duration,
            language,
        };
        println!("            [mdhd] {:?}", this);
        Ok(this)
    }
}

/// 8.4.3 Handler Reference Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdlrBox {
    pub handler_type: HandlerType,
    pub name: CString,
}
impl ReadFrom for HdlrBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::Unsupported);

        let _ = track_io!(reader.read_u32::<BigEndian>())?; // pre_defined
        let handler_type = track!(HandlerType::read_from(&mut reader))?;
        let _ = track_io!(reader.read_exact(&mut [0; 12][..]))?; // reserved

        let mut name = Vec::new();
        track_io!(reader.read_to_end(&mut name))?;
        name.pop(); // NOTE: assumes the last byte is null
        let name = track!(CString::new(name).map_err(|e| ErrorKind::InvalidInput.cause(e)))?;

        let this = HdlrBox { handler_type, name };
        println!("            [hdlr] {:?}", this);
        Ok(this)
    }
}

/// 8.4.4 Media Information Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MinfBox {
    pub stbl_box: StblBox,
    pub dinf_box: DinfBox,
    pub vmhd_box: Option<VmhdBox>,
    pub smhd_box: Option<SmhdBox>,
}
impl ReadFrom for MinfBox {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
        println!("            [minf]");
        let mut stbl_box = None;
        let mut dinf_box = None;
        let mut vmhd_box = None;
        let mut smhd_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"stbl" => track!(read_exactly_one(reader, &mut stbl_box)),
            b"dinf" => track!(read_exactly_one(reader, &mut dinf_box)),
            b"vmhd" => track!(read_exactly_one(reader, &mut vmhd_box)),
            b"smhd" => track!(read_exactly_one(reader, &mut smhd_box)),
            _ => {
                println!("                [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        Ok(MinfBox {
            stbl_box: track_assert_some!(stbl_box, ErrorKind::InvalidInput),
            dinf_box: track_assert_some!(dinf_box, ErrorKind::InvalidInput),
            vmhd_box,
            smhd_box,
        })
    }
}

/// 12.1.2 Video Media Header Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VmhdBox {
    pub graphicsmode: u16,
    pub opcolor: [u16; 3],
}
impl ReadFrom for VmhdBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::InvalidInput);
        track_assert_eq!(header.flags, 1, ErrorKind::InvalidInput);

        let graphicsmode = track_io!(reader.read_u16::<BigEndian>())?;
        let opcolor = [
            track_io!(reader.read_u16::<BigEndian>())?,
            track_io!(reader.read_u16::<BigEndian>())?,
            track_io!(reader.read_u16::<BigEndian>())?,
        ];

        let this = VmhdBox {
            graphicsmode,
            opcolor,
        };
        println!("                [vmhd] {:?}", this);
        Ok(this)
    }
}

/// 12.2.2 Sound Media Header Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SmhdBox {
    pub balance: u16, // fixed point 8.8
}
impl ReadFrom for SmhdBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::InvalidInput);

        let balance = track_io!(reader.read_u16::<BigEndian>())?;
        let _ = track_io!(reader.read_exact(&mut [0; 2][..]))?; // reserved
        let this = SmhdBox { balance };
        println!("                [smhd] {:?}", this);
        Ok(this)
    }
}

/// 8.7.1 Data Information Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DinfBox {
    pub dref_box: DrefBox,
}
impl ReadFrom for DinfBox {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
        println!("                [dinf]");
        let mut dref_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"dref" => track!(read_exactly_one(reader, &mut dref_box)),
            _ => {
                println!("                    [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        Ok(DinfBox {
            dref_box: track_assert_some!(dref_box, ErrorKind::InvalidInput),
        })
    }
}

/// 8.7.2 Data Reference Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DrefBox {
    pub entries: Vec<DataEntryBox>,
}
impl ReadFrom for DrefBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        println!("                    [dref]");
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::InvalidInput);

        let entry_count = track_io!(reader.read_u32::<BigEndian>())?;
        track_assert_ne!(entry_count, 0, ErrorKind::InvalidInput);

        let mut entries = Vec::with_capacity(entry_count as usize);
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"url " => {
                entries.push(DataEntryBox::Url(track!(UrlBox::read_from(reader))?));
                Ok(())
            }
            b"urn " => {
                entries.push(DataEntryBox::Urn(track!(UrnBox::read_from(reader))?));
                Ok(())
            }
            _ => track_panic!(ErrorKind::InvalidInput, "Unexpected box type: {:?}", kind),
        }))?;
        track_assert_eq!(entries.len(), entry_count as usize, ErrorKind::InvalidInput);
        Ok(DrefBox { entries })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataEntryBox {
    Url(UrlBox),
    Urn(UrnBox),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UrlBox {
    pub location: Option<CString>,
}
impl ReadFrom for UrlBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::InvalidInput);
        let this = if (header.flags & 0x00_0001) != 0 {
            UrlBox { location: None }
        } else {
            let mut buf = Vec::new();
            track_io!(reader.read_to_end(&mut buf))?;
            track_assert_eq!(buf.pop(), Some(0), ErrorKind::InvalidInput);

            let location = Some(track!(
                CString::new(buf).map_err(|e| ErrorKind::InvalidInput.cause(e))
            )?);
            UrlBox { location }
        };
        println!("                        [url ] {:?}", this);
        Ok(this)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UrnBox {}
impl ReadFrom for UrnBox {
    fn read_from<R: Read>(_reader: R) -> Result<Self> {
        track_panic!(ErrorKind::Unsupported);
    }
}

/// 8.5.1 Sample Table Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StblBox {
    pub stsd_box: StsdBox,
    pub stts_box: SttsBox,
    pub stsc_box: StscBox,
    pub stsz_box: StszBox,
    pub stco_box: StcoBox,
}
impl ReadFrom for StblBox {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
        println!("                [stbl]");
        let mut stsd_box = None;
        let mut stts_box = None;
        let mut stsc_box = None;
        let mut stsz_box = None;
        let mut stco_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"stsd" => track!(read_exactly_one(reader, &mut stsd_box)),
            b"stts" => track!(read_exactly_one(reader, &mut stts_box)),
            b"stsc" => track!(read_exactly_one(reader, &mut stsc_box)),
            b"stsz" => track!(read_exactly_one(reader, &mut stsz_box)),
            b"stco" => track!(read_exactly_one(reader, &mut stco_box)),
            _ => {
                println!("                    [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        Ok(StblBox {
            stsd_box: track_assert_some!(stsd_box, ErrorKind::InvalidInput),
            stts_box: track_assert_some!(stts_box, ErrorKind::InvalidInput),
            stsc_box: track_assert_some!(stsc_box, ErrorKind::InvalidInput),
            stsz_box: track_assert_some!(stsz_box, ErrorKind::InvalidInput),
            stco_box: track_assert_some!(stco_box, ErrorKind::InvalidInput),
        })
    }
}

/// 8.5.2 Sample Description Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StsdBox {
    pub sample_entries: Vec<SampleEntry>,
}
impl ReadFrom for StsdBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::Unsupported);

        let entry_count = track_io!(reader.read_u32::<BigEndian>())?;
        let mut sample_entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            sample_entries.push(track!(SampleEntry::read_from(&mut reader))?);
        }
        let this = StsdBox { sample_entries };
        println!("                    [stsd] {:?}", this);
        Ok(this)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SampleEntry {
    pub kind: BoxType,
    pub data_reference_index: u16,
    pub data: Vec<u8>,
}
impl ReadFrom for SampleEntry {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(BoxHeader::read_from(&mut reader))?;

        let mut reader = reader.take(u64::from(header.data_size()));
        let _ = track_io!(reader.read_exact(&mut [0; 6][..]))?; // reserved
        let data_reference_index = track_io!(reader.read_u16::<BigEndian>())?;
        let mut data = Vec::new();
        track_io!(reader.read_to_end(&mut data))?;

        Ok(SampleEntry {
            kind: header.kind,
            data_reference_index,
            data,
        })
    }
}

/// 8.6.1.2 Decoding Time to Sample Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SttsBox {
    pub entries: Vec<SttsEntry>,
}
impl ReadFrom for SttsBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::Unsupported);

        let entry_count = track_io!(reader.read_u32::<BigEndian>())?;
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let sample_count = track_io!(reader.read_u32::<BigEndian>())?;
            let sample_delta = track_io!(reader.read_u32::<BigEndian>())?;
            entries.push(SttsEntry {
                sample_count,
                sample_delta,
            });
        }
        let this = SttsBox { entries };
        println!("                    [stts] {:?}", this);
        Ok(this)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SttsEntry {
    pub sample_count: u32,
    pub sample_delta: u32,
}

/// 8.7.4 Sample To Chunk Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StscBox {
    pub entries: Vec<StscEntry>,
}
impl ReadFrom for StscBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::Unsupported);

        let entry_count = track_io!(reader.read_u32::<BigEndian>())?;
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let first_chunk = track_io!(reader.read_u32::<BigEndian>())?;
            let samples_per_chunk = track_io!(reader.read_u32::<BigEndian>())?;
            let sample_description_index = track_io!(reader.read_u32::<BigEndian>())?;
            entries.push(StscEntry {
                first_chunk,
                samples_per_chunk,
                sample_description_index,
            });
        }
        let this = StscBox { entries };
        println!("                    [stsc] {:?}", this);
        Ok(this)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StscEntry {
    pub first_chunk: u32,
    pub samples_per_chunk: u32,
    pub sample_description_index: u32,
}

/// 8.7.3 Sample Size Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StszBox {
    pub sample_size: u32,
    pub sample_count: u32,
    pub entry_sizes: Vec<u32>,
}
impl ReadFrom for StszBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::Unsupported);

        let sample_size = track_io!(reader.read_u32::<BigEndian>())?;
        let sample_count = track_io!(reader.read_u32::<BigEndian>())?;

        let mut entry_sizes = Vec::new();
        if sample_size == 0 {
            for _ in 0..sample_count {
                entry_sizes.push(track_io!(reader.read_u32::<BigEndian>())?);
            }
        }
        let this = StszBox {
            sample_size,
            sample_count,
            entry_sizes,
        };
        println!("                    [stsz] {:?}", this);
        Ok(this)
    }
}

/// 8.7.5 Chunk Offset Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StcoBox {
    pub chunk_offsets: Vec<u32>,
}
impl ReadFrom for StcoBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::Unsupported);

        let entry_count = track_io!(reader.read_u32::<BigEndian>())?;
        let mut chunk_offsets = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            chunk_offsets.push(track_io!(reader.read_u32::<BigEndian>())?);
        }
        let this = StcoBox { chunk_offsets };
        println!("                    [stco] {:?}", this);
        Ok(this)
    }
}

/// 8.6.5 Edit Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdtsBox {
    pub elst_box: Option<ElstBox>,
}
impl ReadFrom for EdtsBox {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
        println!("            [edts]");

        let mut elst_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"elst" => track!(read_exactly_one(reader, &mut elst_box)),
            _ => {
                println!("                [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        Ok(EdtsBox { elst_box })
    }
}

/// 8.6.6 Edit List Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElstBox {
    pub entries: Vec<ElstEntry>,
}
impl ReadFrom for ElstBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;

        let entry_count = track_io!(reader.read_u32::<BigEndian>())?;
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let segment_duration;
            let media_time;
            if header.version == 0 {
                segment_duration = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
                media_time = i64::from(track_io!(reader.read_i32::<BigEndian>())?);
            } else if header.version == 1 {
                segment_duration = track_io!(reader.read_u64::<BigEndian>())?;
                media_time = track_io!(reader.read_i64::<BigEndian>())?;
            } else {
                track_panic!(ErrorKind::InvalidInput, "version={}", header.version);
            }
            let media_rate_integer = track_io!(reader.read_i16::<BigEndian>())?;
            let media_rate_fraction = track_io!(reader.read_i16::<BigEndian>())?;
            entries.push(ElstEntry {
                segment_duration,
                media_time,
                media_rate_integer,
                media_rate_fraction,
            });
        }
        let this = ElstBox { entries };
        println!("                [elst] {:?}", this);
        Ok(this)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElstEntry {
    pub segment_duration: u64,
    pub media_time: i64,
    pub media_rate_integer: i16,
    pub media_rate_fraction: i16,
}

/// 8.8.9 Movie Fragment Random Access Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MfraBox {
    tfra_boxes: Vec<TfraBox>,
    mfro_box: MfroBox,
}
impl ReadFrom for MfraBox {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
        println!("[mfra]");

        let mut tfra_boxes = Vec::new();
        let mut mfro_box = None;
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"tfra" => {
                tfra_boxes.push(track!(TfraBox::read_from(reader))?);
                Ok(())
            }
            b"mfro" => track!(read_exactly_one(reader, &mut mfro_box)),
            _ => {
                println!("    [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        let mfro_box = track_assert_some!(mfro_box, ErrorKind::InvalidInput);
        Ok(MfraBox {
            tfra_boxes,
            mfro_box,
        })
    }
}

/// 8.8.10 Track Fragment Random Access Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TfraBox {
    pub track_id: u32,
    pub entries: Vec<TfraEntry>,
}
impl ReadFrom for TfraBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;

        let track_id = track_io!(reader.read_u32::<BigEndian>())?;
        let _ = track_io!(reader.read_exact(&mut [0; 3][..]))?; // reserved

        let b = track_io!(reader.read_u8())?;
        let length_size_of_traf_num = (b >> 4) & 0b11;
        let length_size_of_trun_num = (b >> 2) & 0b11;
        let length_size_of_sample_num = b & 0b11;

        let traf_num_size = (length_size_of_traf_num + 1) as usize;
        let trun_num_size = (length_size_of_trun_num + 1) as usize;
        let sample_num_size = (length_size_of_sample_num + 1) as usize;

        let number_of_entry = track_io!(reader.read_u32::<BigEndian>())?;
        let mut entries = Vec::with_capacity(number_of_entry as usize);
        for _ in 0..number_of_entry {
            let time;
            let moof_offset;
            if header.version == 0 {
                time = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
                moof_offset = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
            } else {
                track_assert_eq!(header.version, 1, ErrorKind::InvalidInput);
                time = track_io!(reader.read_u64::<BigEndian>())?;
                moof_offset = track_io!(reader.read_u64::<BigEndian>())?;
            }
            let traf_number = track_io!(reader.read_uint::<BigEndian>(traf_num_size))? as u32;
            let trun_number = track_io!(reader.read_uint::<BigEndian>(trun_num_size))? as u32;
            let sample_number = track_io!(reader.read_uint::<BigEndian>(sample_num_size))? as u32;

            entries.push(TfraEntry {
                time,
                moof_offset,
                traf_number,
                trun_number,
                sample_number,
            });
        }

        let this = TfraBox { track_id, entries };
        println!("    [tfra] {:?}", this);
        Ok(this)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TfraEntry {
    pub time: u64,
    pub moof_offset: u64,
    pub traf_number: u32,
    pub trun_number: u32,
    pub sample_number: u32,
}

/// 8.8.11 Movie Fragment Random Access Offset Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MfroBox {
    pub size: u32,
}
impl ReadFrom for MfroBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let _header = track!(FullBoxHeader::read_from(&mut reader))?;
        let size = track_io!(reader.read_u32::<BigEndian>())?;
        let this = MfroBox { size };
        println!("    [mfro] {:?}", this);
        Ok(this)
    }
}

/// 8.8.4 Movie Fragment Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MoofBox {
    pub mfhd_box: MfhdBox,
    pub traf_boxes: Vec<TrafBox>,
}
impl ReadFrom for MoofBox {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
        println!("[moof]");

        let mut mfhd_box = None;
        let mut traf_boxes = Vec::new();
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"mfhd" => track!(read_exactly_one(reader, &mut mfhd_box)),
            b"traf" => {
                traf_boxes.push(track!(TrafBox::read_from(reader))?);
                Ok(())
            }
            _ => {
                println!("    [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        Ok(MoofBox {
            mfhd_box: track_assert_some!(mfhd_box, ErrorKind::InvalidInput),
            traf_boxes,
        })
    }
}

/// 8.8.5 Movie Fragment Header Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MfhdBox {
    pub sequence_number: u32,
}
impl ReadFrom for MfhdBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let _header = track!(FullBoxHeader::read_from(&mut reader))?;
        let sequence_number = track_io!(reader.read_u32::<BigEndian>())?;

        let this = MfhdBox { sequence_number };
        println!("    [mfhd] {:?}", this);
        Ok(this)
    }
}

/// 8.8.6 Track Fragment Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrafBox {
    pub tfhd_box: TfhdBox,
    pub tfdt_box: Option<TfdtBox>,
    pub trun_boxes: Vec<TrunBox>,
}
impl ReadFrom for TrafBox {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
        println!("    [traf]");

        let mut tfhd_box = None;
        let mut tfdt_box = None;
        let mut trun_boxes = Vec::new();
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            b"tfhd" => track!(read_exactly_one(reader, &mut tfhd_box)),
            b"tfdt" => track!(read_exactly_one(reader, &mut tfdt_box)),
            b"trun" => {
                trun_boxes.push(track!(TrunBox::read_from(reader))?);
                Ok(())
            }
            _ => {
                println!("        [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;

        Ok(TrafBox {
            tfhd_box: track_assert_some!(tfhd_box, ErrorKind::InvalidInput),
            tfdt_box,
            trun_boxes,
        })
    }
}

/// 8.8.7 Track Fragment Header Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TfhdBox {
    pub track_id: u32,
    pub duration_is_empty: bool,
    pub default_base_is_moof: bool,
    pub base_data_offset: Option<u64>,
    pub sample_description_index: Option<u32>,
    pub default_sample_duration: Option<u32>,
    pub default_sample_size: Option<u32>,
    pub default_sample_flags: Option<u32>,
}
impl ReadFrom for TfhdBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::InvalidInput);

        let track_id = track_io!(reader.read_u32::<BigEndian>())?;
        let duration_is_empty = (header.flags & 0x01_0000) != 0;
        let default_base_is_moof = (header.flags & 0x02_0000) != 0;

        let base_data_offset = if (header.flags & 0x00_0001) != 0 {
            Some(track_io!(reader.read_u64::<BigEndian>())?)
        } else {
            None
        };
        let sample_description_index = if (header.flags & 0x00_0002) != 0 {
            Some(track_io!(reader.read_u32::<BigEndian>())?)
        } else {
            None
        };
        let default_sample_duration = if (header.flags & 0x00_0008) != 0 {
            Some(track_io!(reader.read_u32::<BigEndian>())?)
        } else {
            None
        };
        let default_sample_size = if (header.flags & 0x00_0010) != 0 {
            Some(track_io!(reader.read_u32::<BigEndian>())?)
        } else {
            None
        };
        let default_sample_flags = if (header.flags & 0x00_0020) != 0 {
            Some(track_io!(reader.read_u32::<BigEndian>())?)
        } else {
            None
        };

        let this = TfhdBox {
            track_id,
            duration_is_empty,
            default_base_is_moof,
            base_data_offset,
            sample_description_index,
            default_sample_duration,
            default_sample_size,
            default_sample_flags,
        };
        println!("        [tfhd] {:?}", this);
        Ok(this)
    }
}

/// 8.8.8 Track Run Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrunBox {
    pub data_offset: Option<i32>,
    pub first_sample_flags: Option<u32>,
    pub entries: Vec<TrunEntry>,
}
impl ReadFrom for TrunBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;
        track_assert_eq!(header.version, 0, ErrorKind::InvalidInput);

        let sample_count = track_io!(reader.read_u32::<BigEndian>())?;
        let data_offset = if (header.flags & 0x00_0001) != 0 {
            Some(track_io!(reader.read_i32::<BigEndian>())?)
        } else {
            None
        };
        let first_sample_flags = if (header.flags & 0x00_0004) != 0 {
            Some(track_io!(reader.read_u32::<BigEndian>())?)
        } else {
            None
        };

        let mut entries = Vec::with_capacity(sample_count as usize);
        for _ in 0..sample_count {
            let sample_duration = if (header.flags & 0x00_0100) != 0 {
                Some(track_io!(reader.read_u32::<BigEndian>())?)
            } else {
                None
            };
            let sample_size = if (header.flags & 0x00_0200) != 0 {
                Some(track_io!(reader.read_u32::<BigEndian>())?)
            } else {
                None
            };
            let sample_flags = if (header.flags & 0x00_0400) != 0 {
                Some(track_io!(reader.read_u32::<BigEndian>())?)
            } else {
                None
            };
            let sample_composition_time_offset = if (header.flags & 0x00_0800) != 0 {
                if header.version == 0 {
                    Some(i64::from(track_io!(reader.read_u32::<BigEndian>())?))
                } else {
                    Some(i64::from(track_io!(reader.read_i32::<BigEndian>())?))
                }
            } else {
                None
            };
            entries.push(TrunEntry {
                sample_duration,
                sample_size,
                sample_flags,
                sample_composition_time_offset,
            });
        }

        let this = TrunBox {
            data_offset,
            first_sample_flags,
            entries,
        };
        println!(
            "        [trun] data_offset={:?}, first_sample_flags={:?}, entries.len={}",
            this.data_offset,
            this.first_sample_flags,
            this.entries.len()
        );
        Ok(this)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrunEntry {
    pub sample_duration: Option<u32>,
    pub sample_size: Option<u32>,
    pub sample_flags: Option<u32>,
    pub sample_composition_time_offset: Option<i64>,
}

/// 8.8.12 Track fragment decode time
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TfdtBox {
    pub base_media_decode_time: u64,
}
impl ReadFrom for TfdtBox {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(FullBoxHeader::read_from(&mut reader))?;

        let base_media_decode_time;
        if header.version == 0 {
            base_media_decode_time = u64::from(track_io!(reader.read_u32::<BigEndian>())?);
        } else {
            track_assert_eq!(header.version, 1, ErrorKind::InvalidInput);
            base_media_decode_time = track_io!(reader.read_u64::<BigEndian>())?;
        };
        let this = TfdtBox {
            base_media_decode_time,
        };
        println!("        [tfdt] {:?}", this);
        Ok(this)
    }
}
