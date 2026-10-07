//! Files the fuzzer built out of a Chitu header. One panicked multiplying the panel's two
//! fields, the other asked for a layer table of gigabytes out of 110 bytes; a reader must
//! answer both with an error.
#![expect(
    clippy::expect_used,
    reason = "a missing fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::path::Path;

use core_format::{FormatError, SlicedFileReader};
use format_chitu::ChituReader;

fn refusal_of(name: &str) -> FormatError {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/crashes")
        .join(name);
    let bytes = std::fs::read(&path).expect("the fixture is checked in beside the test");

    ChituReader::default()
        .open(&mut Cursor::new(bytes))
        .err()
        .expect("a hostile file is refused rather than read")
}

#[test]
fn a_panel_whose_two_fields_multiply_past_a_word_is_refused_rather_than_wrapped() {
    let err = refusal_of("panel_overflows.ctb");
    assert!(matches!(err, FormatError::PanelTooLarge { .. }), "{err}");
}

#[test]
fn a_header_claiming_more_than_the_file_holds_is_refused_rather_than_reserved_for() {
    let err = refusal_of("layer_table_too_long.ctb");
    assert!(
        matches!(
            err,
            FormatError::ImpossibleCount { .. } | FormatError::PanelTooLarge { .. }
        ),
        "{err}"
    );
}
