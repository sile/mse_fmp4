extern crate fmp4;
#[macro_use]
extern crate trackable;

use std::io::Read;
use trackable::error::Failure;

fn main() {
    let mut peek = [0];
    while 1
        == track_try_unwrap!(
            std::io::stdin()
                .read(&mut peek)
                .map_err(Failure::from_error)
        ) {
        let header = track_try_unwrap!(fmp4::isobmff::BoxHeader::read_from(peek.chain(
            std::io::stdin()
        )));
        println!("{:?}", header);

        let mut buf = vec![0; header.data_size() as usize];
        track_try_unwrap!(
            std::io::stdin()
                .read_exact(&mut buf)
                .map_err(Failure::from_error)
        );
    }
}
