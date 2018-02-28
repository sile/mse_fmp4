extern crate byteorder;
#[macro_use]
extern crate trackable;

macro_rules! track_io {
    ($expr:expr) => {
        $expr.map_err(|e: ::std::io::Error| {
            use trackable::error::ErrorKindExt;
            track!(::Error::from(::ErrorKind::Other.cause(e)))
        })
    }
}

pub use error::{Error, ErrorKind};

pub mod aac;
pub mod avc;
pub mod fmp4;

mod error;

/// This crate specific `Result` type.
pub type Result<T> = std::result::Result<T, Error>;
