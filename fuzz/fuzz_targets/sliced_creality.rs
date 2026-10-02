//! The `.cxdlp` at both revisions: version 3's lines, version 4's tables and the seven-bit
//! run-length codec behind them. The field behind the magic picks the reader, so one target
//! covers both.
#![no_main]

use std::io::Cursor;
use std::path::Path;

use core_format::OpenFile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut open) = core_pipeline::open(Path::new("fuzz.cxdlp"), Cursor::new(data)) else {
        return;
    };
    let count = open.facts().layer_count().min(256);
    for index in 0..count {
        let _ = open.layer(index);
    }
});
