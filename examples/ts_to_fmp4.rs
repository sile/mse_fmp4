extern crate mpeg2ts;
extern crate mse_fmp4;
#[macro_use]
extern crate trackable;

use mse_fmp4::fmp4::{self, WriteTo};

fn main() {
    let f = fmp4::File::new();
    track_try_unwrap!(f.write_to(std::io::stdout()));
}
