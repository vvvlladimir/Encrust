//! Reads a `.goo` 5.1 the vendor's slicer wrote for an Elegoo Jupiter 2, cut to its first
//! two layers with the tables and MD5 restated, and grey edges on at the vendor's eight levels.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::fs::File;

use std::io::Cursor;

use core_format::{FormatError, OpenFile, SlicedFileReader};
use format_goo::GooReader;

const WIDTH_PX: u32 = 15_120;
const HEIGHT_PX: u32 = 6230;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/jupiter-2-two-layers.goo"
);

/// Where the mask table's offset is stated, behind the shared header and one byte.
const MASK_TABLE_FIELD: usize = 195_482;

fn open() -> impl OpenFile {
    GooReader
        .open(File::open(FIXTURE).expect("the fixture is checked in"))
        .expect("a vendor file reads")
}

/// Every pixel of layer `index`, row by row.
fn pixels(file: &mut impl OpenFile, index: u32) -> Vec<u8> {
    file.layer(index)
        .expect("a vendor layer decodes")
        .iter()
        .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
        .collect()
}

#[test]
fn the_header_states_the_machine_and_the_panel() {
    let file = open();
    let facts = file.facts();
    assert_eq!(facts.version, Some(51));
    assert_eq!(facts.machine.as_deref(), Some("ELEGOO Jupiter 2"));
    assert_eq!((facts.width_px, facts.height_px), (WIDTH_PX, HEIGHT_PX));
    assert_eq!(facts.layer_count(), 2);
}

#[test]
fn the_retract_reads_as_the_resin_stated_it() {
    // The vendor states 1.95 mm for a 2 mm lift at 0.05 mm layers.
    let retract = open().facts().stated.retract_distance_mm;
    let retract = retract.expect("a retract is stated");
    assert!((retract - 2.0).abs() < 1e-5, "expected 2 mm, got {retract}");
}

#[test]
fn a_layer_covers_the_whole_panel_and_runs_on_across_the_seam() {
    let mut file = open();
    let pixels = pixels(&mut file, 0);
    assert_eq!(pixels.len() as u32, WIDTH_PX * HEIGHT_PX);

    // Row 3404 is lit from column 7258 in the left half to 299 in the right.
    let row = &pixels[(3404 * WIDTH_PX) as usize..][..WIDTH_PX as usize];
    let lit: Vec<usize> = (0..row.len()).filter(|&x| row[x] > 0).collect();
    assert_eq!(lit.first(), Some(&7258));
    assert_eq!(lit.last(), Some(&(7560 + 298)));
    assert_eq!(
        lit.len(),
        lit.last().expect("lit") - lit[0] + 1,
        "no gap at the seam"
    );
}

#[test]
fn grey_edges_come_back_on_the_eight_levels() {
    let mut file = open();
    let mut seen = [false; 256];
    for value in pixels(&mut file, 0) {
        seen[value as usize] = true;
    }
    let levels: Vec<u8> = (0..=255).filter(|&value| seen[value as usize]).collect();
    assert_eq!(levels, [0, 36, 73, 109, 146, 182, 219, 255]);
}

#[test]
fn a_mask_too_short_for_its_frame_is_refused() {
    let mut bytes = std::fs::read(FIXTURE).expect("the fixture is checked in");
    let table = u32::from_le_bytes(
        bytes[MASK_TABLE_FIELD..][..4]
            .try_into()
            .expect("four bytes"),
    ) as usize;
    // The first left mask's size, behind the table's mark and the mask's offset.
    bytes[table + 5..][..4].copy_from_slice(&3_u32.to_le_bytes());
    let err = GooReader
        .open(Cursor::new(bytes))
        .err()
        .expect("three bytes cannot hold a magic byte, a checksum and a delimiter");
    assert!(matches!(err, FormatError::MalformedRun { .. }));
}
