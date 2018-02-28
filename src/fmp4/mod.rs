pub use self::common::{BoxHeader, BoxType, Brand, FullBoxHeader, HandlerType, SampleFormat,
                       WriteBoxTo};

macro_rules! write_u8 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::WriteBytesExt;
            track_io!($w.write_u8($n))?;
        }
    }
}
macro_rules! write_u16 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_u16::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_i16 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_i16::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_u24 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_uint::<BigEndian>($n as u64, 3))?;
        }
    }
}
macro_rules! write_u32 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_u32::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_i32 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_i32::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_u64 {
    ($w:expr, $n:expr) => {
        {
            use byteorder::{BigEndian, WriteBytesExt};
            track_io!($w.write_u64::<BigEndian>($n))?;
        }
    }
}
macro_rules! write_all {
    ($w:expr, $n:expr) => { track_io!($w.write_all($n))?; }
}
macro_rules! write_zeroes {
    ($w:expr, $n:expr) => { track_io!($w.write_all(&[0;$n][..]))?; }
}
macro_rules! write_box {
    ($w:expr, $b:expr) => { track!($b.write_box_to(&mut $w))?; }
}
macro_rules! write_boxes {
    ($w:expr, $bs:expr) => {
        for b in $bs {
            track!(b.write_box_to(&mut $w))?;
        }
    }
}

mod common;
pub mod initialization;
pub mod media;
