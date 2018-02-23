extern crate clap;
extern crate fmp4;
#[macro_use]
extern crate trackable;

use std::io::{stdin, Read};
use clap::{App, Arg};
use fmp4::{Error, ErrorKind, Result};
use fmp4::isobmff;

macro_rules! track_io {
    ($expr:expr) => {
        $expr.map_err(|e: std::io::Error| {
            use trackable::error::ErrorKindExt;
            track!(Error::from(ErrorKind::Other.cause(e)))
        })
    }
}

fn main() {
    let matches = App::new("parse")
        .arg(
            Arg::with_name("TYPE")
                .long("type")
                .takes_value(true)
                .possible_values(&["toplevel", "tree"])
                .default_value("toplevel"),
        )
        .get_matches();
    match matches.value_of("TYPE").unwrap() {
        "toplevel" => track_try_unwrap!(parse_toplevel()),
        "tree" => track_try_unwrap!(parse_tree()),
        _ => unreachable!(),
    }
}

fn parse_toplevel() -> Result<()> {
    let mut peek = [0];
    while 1 == track_io!(stdin().read(&mut peek))? {
        let header = track_try_unwrap!(isobmff::BoxHeader::read_from(peek.chain(stdin())));
        println!("{:?}", header);
        {
            let mut reader = stdin().take(u64::from(header.data_size()));
            track_io!(reader.read_to_end(&mut Vec::new()))?;
        }
    }
    Ok(())
}

fn parse_tree() -> Result<()> {
    let file = track!(isobmff::File::read_from(stdin()));
    println!("{:?}", file);
    Ok(())
}
