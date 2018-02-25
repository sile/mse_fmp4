extern crate mpeg2ts;
extern crate mse_fmp4;
#[macro_use]
extern crate trackable;

use mpeg2ts::time::Timestamp;
use mse_fmp4::fmp4::{self, WriteTo};

fn main() {
    let mut f = fmp4::File::new();
    f.moov_box.mvhd_box.timescale = Timestamp::RESOLUTION as u32;
    f.moov_box.mvhd_box.duration = 1 * Timestamp::RESOLUTION as u32; // TODO

    let mut t = fmp4::TrackBox::new(true);
    t.tkhd_box.duration = 1 * Timestamp::RESOLUTION as u32; // TODO
    t.mdia_box.mdhd_box.timescale = Timestamp::RESOLUTION as u32;
    t.mdia_box.mdhd_box.duration = 1 * Timestamp::RESOLUTION as u32; // TODO
    f.moov_box.trak_boxes.push(t);

    track_try_unwrap!(f.write_to(std::io::stdout()));
}
