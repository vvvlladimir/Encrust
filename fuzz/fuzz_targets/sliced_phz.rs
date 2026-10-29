//! `.phz`: the header, the layer table, the cipher and the one-bit codec. The path is only
//! read for its extension, so no file is touched.
#![no_main]

use std::io::Cursor;
use std::path::Path;

use core_format::OpenFile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut open) = core_pipeline::open(Path::new("fuzz.phz"), Cursor::new(data)) else {
        return;
    };
    // A header is free to claim four billion layers; reading the first few hundred covers every
    // branch of the decoder without turning the session into one file.
    let count = open.facts().layer_count().min(256);
    for index in 0..count {
        let _ = open.layer(index);
    }
});
