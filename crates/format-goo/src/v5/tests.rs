use std::io::Cursor;

use core_format::{OpenFile, SlicedFileReader};

use super::*;
use crate::GooReader;
use crate::fixtures::sample_job;

/// An 8 x 4 layer lit on the right half of its second row.
fn layer() -> LayerRuns {
    let mut builder = LayerRuns::builder(8, 4);
    builder.push(12, 0);
    builder.push(4, 255);
    builder.pad_to(32);
    builder.finish()
}

fn written(layers: u32) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut sink = Goo5Writer
            .begin(&sample_job(layers), &mut buffer)
            .expect("the job is sound");
        for _ in 0..layers {
            sink.push(Goo5Sink::encode(&layer())).expect("a layer");
        }
        sink.finish(10.0).expect("every layer arrived");
    }
    buffer.into_inner()
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(bytes, at))
}

#[test]
fn the_file_names_its_revision_and_states_its_fields_least_significant_first() {
    let bytes = written(2);
    assert_eq!(&bytes[..4], b"V5.1");
    assert_eq!(u32_at(&bytes, 195_310), 2, "the layer count");
    assert_eq!(
        u32_at(&bytes, 195_470),
        HEADER_SIZE,
        "the end of the shared header"
    );
}

#[test]
fn the_tables_point_at_the_definitions_and_the_masks() {
    let bytes = written(2);
    let tables = Tables::of(2);
    assert_eq!(u32_at(&bytes, HEADER_SIZE as usize + 1), tables.definitions);
    assert_eq!(bytes[tables.masks as usize], MASKS_MARK);

    let definition = u32_at(&bytes, tables.definitions as usize + 9) as usize;
    assert!(
        (f32_at(&bytes, definition + 6) - 0.1).abs() < 1e-6,
        "layer 1's top"
    );

    let entry = tables.masks as usize + 1 + 16;
    let (right_at, right_size) = (u32_at(&bytes, entry + 8), u32_at(&bytes, entry + 12));
    assert_eq!(bytes[right_at as usize], LAYER_MAGIC);
    assert_eq!(
        &bytes[(right_at + right_size) as usize - 2..][..2],
        &DELIMITER
    );
}

#[test]
fn the_retract_states_the_travel_down_to_a_layer_higher() {
    let bytes = written(2);
    let job = sample_job(2);
    let stated = f32_at(&bytes, 195_401);
    let expected = job.material.retract_distance_mm - job.nominal_height_mm();
    assert!(
        (stated - expected).abs() < 1e-6,
        "expected the resin's retract less a layer, {expected}, got {stated}"
    );
}

#[test]
fn the_file_ends_with_the_md5_of_everything_before_it() {
    let bytes = written(2);
    let (body, tail) = bytes.split_at(bytes.len() - 32);
    assert_eq!(tail, hex_of(&Md5::digest(body)).as_bytes());
}

#[test]
fn a_written_file_reads_back_as_the_layers_it_was_given() {
    let bytes = written(2);
    let mut open = GooReader.open(Cursor::new(bytes)).expect("our own file");
    assert_eq!(open.facts().version, Some(51));
    let job = sample_job(2);
    let retract = open.facts().stated.retract_distance_mm;
    assert_eq!(
        retract,
        Some(job.material.retract_distance_mm),
        "the resin's own"
    );
    assert_eq!(open.layer(1).expect("layer 1"), layer().runs());
}

#[test]
fn a_panel_of_odd_width_is_refused() {
    let mut job = sample_job(1);
    job.raster.width_px = 7;
    job.printer.display.width_px = 7;
    let err = Goo5Writer
        .begin(&job, &mut Cursor::new(Vec::new()))
        .err()
        .expect("seven pixels have no middle");
    assert!(matches!(err, FormatError::Encoding { .. }));
}

#[test]
fn a_layer_of_the_wrong_size_is_refused() {
    let mut buffer = Cursor::new(Vec::new());
    let mut sink = Goo5Writer
        .begin(&sample_job(1), &mut buffer)
        .expect("the job is sound");
    let err = sink
        .push(Goo5Sink::encode(&LayerRuns::builder(4, 4).finish()))
        .unwrap_err();
    assert!(matches!(
        err,
        FormatError::ResolutionMismatch { index: 0, .. }
    ));
}

#[test]
fn fewer_layers_than_promised_are_refused_at_the_end() {
    let mut buffer = Cursor::new(Vec::new());
    let sink = Goo5Writer
        .begin(&sample_job(2), &mut buffer)
        .expect("the job is sound");
    assert!(matches!(
        sink.finish(0.0).unwrap_err(),
        FormatError::LayerCountMismatch {
            expected: 2,
            written: 0
        }
    ));
}
