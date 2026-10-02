//! A file the fuzzer built out of an Anycubic mark: ninety-nine bytes claiming a panel of
//! seven quintillion pixels and a layer table of ninety-eight gigabytes. Every count it
//! states is checked against the file before anything is reserved for it.

use std::io::Cursor;
use std::path::Path;

use core_format::{FormatError, SlicedFileReader};
use format_anycubic::AnycubicReader;

#[test]
fn a_header_claiming_more_than_the_file_holds_is_refused_rather_than_reserved_for() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/crashes/layer_table_too_long.pwmx");
    let bytes = std::fs::read(&path).expect("the fixture is checked in beside the test");

    let mut source = Cursor::new(bytes);
    let err = AnycubicReader
        .open(&mut source)
        .err()
        .expect("a hostile file is refused rather than read");
    assert!(
        matches!(
            err,
            FormatError::ImpossibleCount { .. } | FormatError::PanelTooLarge { .. }
        ),
        "{err}"
    );
}
