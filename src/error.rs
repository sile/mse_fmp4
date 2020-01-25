use mpeg2ts;
use trackable::error::{ErrorKind as TrackableErrorKind, ErrorKindExt, TrackableError};

/// This crate specific `Error` type.
#[derive(Debug, Clone, TrackableError)]
pub struct Error(TrackableError<ErrorKind>);
impl From<mpeg2ts::Error> for Error {
    fn from(f: mpeg2ts::Error) -> Self {
        let kind = match *f.kind() {
            mpeg2ts::ErrorKind::InvalidInput => ErrorKind::InvalidInput,
            mpeg2ts::ErrorKind::Unsupported => ErrorKind::Unsupported,
            mpeg2ts::ErrorKind::Other => ErrorKind::Other,
        };
        kind.takes_over(f).into()
    }
}

/// Possible error kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum ErrorKind {
    InvalidInput,
    Unsupported,
    Other,
}
impl TrackableErrorKind for ErrorKind {}
