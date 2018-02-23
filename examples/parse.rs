extern crate fmp4;
#[macro_use]
extern crate trackable;

use std::io::Read;
use fmp4::isobmff;
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
        {
            let mut reader = std::io::stdin().take(u64::from(header.data_size()));
            match header.kind {
                isobmff::FileTypeBox::TYPE => {
                    let b = track_try_unwrap!(isobmff::FileTypeBox::read_from(&mut reader));
                    println!("  {:?}", b);
                }
                _ => {
                    let mut buf = Vec::new();
                    track_try_unwrap!(reader.read_to_end(&mut buf).map_err(Failure::from_error));
                }
            }
            assert_eq!(reader.limit(), 0);
        }
    }
}
