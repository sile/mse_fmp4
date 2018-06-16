//! This is a library for generating fragmented MP4 that playable via Media Source Extensions.
//!
//! # References
//!
//! - [ISO BMFF Byte Stream Format (Fragmented MP4)][fmp4]
//! - [Media Source Extensions][MSE]
//!
//! [fmp4]: https://w3c.github.io/media-source/isobmff-byte-stream-format.html
//! [MSE]: http://www.w3.org/TR/media-source/
#![warn(missing_docs)]
extern crate byteorder;
extern crate mpeg2ts;
#[macro_use]
extern crate trackable;

macro_rules! track_io {
    ($expr:expr) => {
        $expr.map_err(|e: ::std::io::Error| {
            use trackable::error::ErrorKindExt;
            track!(::Error::from(::ErrorKind::Other.cause(e)))
        })
    };
}
macro_rules! write_u8 {
    ($w:expr, $n:expr) => {{
        use byteorder::WriteBytesExt;
        track_io!($w.write_u8($n))?;
    }};
}
macro_rules! write_u16 {
    ($w:expr, $n:expr) => {{
        use byteorder::{BigEndian, WriteBytesExt};
        track_io!($w.write_u16::<BigEndian>($n))?;
    }};
}
macro_rules! write_i16 {
    ($w:expr, $n:expr) => {{
        use byteorder::{BigEndian, WriteBytesExt};
        track_io!($w.write_i16::<BigEndian>($n))?;
    }};
}
macro_rules! write_u24 {
    ($w:expr, $n:expr) => {{
        use byteorder::{BigEndian, WriteBytesExt};
        track_io!($w.write_uint::<BigEndian>($n as u64, 3))?;
    }};
}
macro_rules! write_u32 {
    ($w:expr, $n:expr) => {{
        use byteorder::{BigEndian, WriteBytesExt};
        track_io!($w.write_u32::<BigEndian>($n))?;
    }};
}
macro_rules! write_i32 {
    ($w:expr, $n:expr) => {{
        use byteorder::{BigEndian, WriteBytesExt};
        track_io!($w.write_i32::<BigEndian>($n))?;
    }};
}
macro_rules! write_u64 {
    ($w:expr, $n:expr) => {{
        use byteorder::{BigEndian, WriteBytesExt};
        track_io!($w.write_u64::<BigEndian>($n))?;
    }};
}
macro_rules! write_all {
    ($w:expr, $n:expr) => {
        track_io!($w.write_all($n))?;
    };
}
macro_rules! write_zeroes {
    ($w:expr, $n:expr) => {
        track_io!($w.write_all(&[0; $n][..]))?;
    };
}
macro_rules! write_box {
    ($w:expr, $b:expr) => {
        track!($b.write_box(&mut $w))?;
    };
}
macro_rules! write_boxes {
    ($w:expr, $bs:expr) => {
        for b in $bs {
            track!(b.write_box(&mut $w))?;
        }
    };
}

pub use error::{Error, ErrorKind};

pub mod aac;
pub mod avc;
pub mod fmp4;
pub mod io;
pub mod mpeg2_ts;

mod error;

/// This crate specific `Result` type.
pub type Result<T> = std::result::Result<T, Error>;
