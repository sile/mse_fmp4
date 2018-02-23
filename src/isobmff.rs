use std::fmt;
use std::io::Read;
use std::str;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FullBoxHeader {
    pub version: u8,
    pub flags: u32, // u24
}
impl FullBoxHeader {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_u32::<BigEndian>())?;
        let version = (n >> 24) as u8;
        let flags = n & 0xFF_FFFF;
        Ok(FullBoxHeader { version, flags })
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileTypeBox {
    pub major_brand: Brand,
    pub minor_version: u32,
    pub compatible_brands: Vec<Brand>,
}
impl FileTypeBox {
    pub const TYPE: BoxType = BoxType([b'f', b't', b'y', b'p']);
    pub const CONTAINER: &'static str = "File";
    pub const MANDATORY: bool = true;
    pub const QUANTITY: Quantity = Quantity::ExactlyOne;

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        // let header = track!(BoxHeader::read_from(&mut reader))?;
        // track_assert_eq!(header.kind, Self::TYPE, ErrorKind::InvalidInput);

        // let mut reader = reader.take(u64::from(header.data_size()));
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quantity {
    ExactlyOne,
    ZeroOrMore,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaDataBox {
    pub data: Vec<u8>,
}
impl MediaDataBox {
    pub const TYPE: BoxType = BoxType([b'm', b'd', b'a', b't']);
    pub const CONTAINER: &'static str = "File";
    pub const MANDATORY: bool = false;
    pub const QUANTITY: Quantity = Quantity::ZeroOrMore;

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
}
impl File {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut ftyp_box = None;
        let mut moov_box = None;
        let mut mdat_boxes = Vec::new();

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
}
impl MoovBox {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut mvhd_box = None;
        let mut trak_boxes = Vec::new();

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

/// Track Box.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrakBox {}
impl TrakBox {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        track!(each_boxes(reader, |kind, reader| match &kind.0 {
            _ => {
                println!("        [todo] {:?}", kind);
                track_io!(reader.read_to_end(&mut Vec::new()))?;
                Ok(())
            }
        }))?;
        Ok(TrakBox {})
    }
}
