extern crate clap;
extern crate mpeg2ts;
extern crate mse_fmp4;
#[macro_use]
extern crate trackable;

use std::fs::File;
use clap::{App, Arg};
use mse_fmp4::io::WriteTo;
use mse_fmp4::mpeg2_ts;
use mpeg2ts::ts::TsPacketReader;
use trackable::error::Failure;

fn main() {
    let matches = App::new("ts_to_fmp4")
        .arg(
            Arg::with_name("OUTPUT_FILE_PREFIX")
                .long("output-file-prefix")
                .takes_value(true)
                .default_value("movie"),
        )
        .get_matches();
    let output_file_prefix = matches.value_of("OUTPUT_FILE_PREFIX").unwrap();

    let (initialization_segment, media_segment) =
        track_try_unwrap!(mpeg2_ts::to_fmp4(TsPacketReader::new(std::io::stdin())));

    let path = format!("{}-init.mp4", output_file_prefix);
    let out = track_try_unwrap!(File::create(&path).map_err(Failure::from_error));
    track_try_unwrap!(initialization_segment.write_to(out));
    println!("# Initialization Segment: {:?}", path);

    let path = format!("{}.m4s", output_file_prefix);
    let out = track_try_unwrap!(File::create(&path).map_err(Failure::from_error));
    track_try_unwrap!(media_segment.write_to(out));
    println!("# Media Segment: {:?}", path);
}
