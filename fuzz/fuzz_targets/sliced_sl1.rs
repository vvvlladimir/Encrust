//! `.sl1` and `.sl1s`: a zip of greyscale PNGs, so this reaches the zip reader and the PNG
//! decoder behind it. The path is only read for its extension, so no file is touched.
#![no_main]

use std::io::Cursor;
use std::path::Path;

use core_format::OpenFile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut open) = core_pipeline::open(Path::new("fuzz.sl1"), Cursor::new(data)) else {
        return;
    };
    let count = open.facts().layer_count().min(256);
    for index in 0..count {
        let _ = open.layer(index);
    }
});
