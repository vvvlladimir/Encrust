use std::io::Cursor;

use core_format::{FormatError, LayerSink, OpenFile, SlicedFileReader, SlicedFileWriter};
use core_raster::LayerRuns;

use super::blocks::{FOOTER, MAGIC};
use super::{CxlineReader, CxlineSink, CxlineWriter, cxline_model_code};
use crate::fixtures::sample_job;
use core_format::Crc32;

/// An 8 x 4 layer with a lit block from column 2 to 5 in rows 1 and 2.
fn block() -> LayerRuns {
    let mut builder = LayerRuns::builder(8, 4);
    builder.push(10, 0);
    builder.push(4, 255);
    builder.push(4, 0);
    builder.push(4, 255);
    builder.push(10, 0);
    builder.finish()
}

fn written(layers: u32) -> Vec<u8> {
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = CxlineWriter
            .begin(&sample_job(layers), &mut file)
            .expect("the job is sound");
        for _ in 0..layers {
            sink.push(CxlineSink::encode(&block())).expect("a layer");
        }
        sink.finish(1500.0).expect("every promised layer arrived");
    }
    file.into_inner()
}

#[test]
fn the_d158_is_named_by_its_code_and_every_other_machine_by_its_name() {
    assert_eq!(cxline_model_code("D158"), "CD60");
    assert_eq!(cxline_model_code("D160"), "D160");
}

#[test]
fn a_file_is_headed_by_its_magic_and_ends_with_the_family_footer_and_checksum() {
    let bytes = written(2);
    assert_eq!(bytes[..4], 9u32.to_be_bytes());
    assert_eq!(&bytes[4..13], MAGIC);
    assert_eq!(bytes[13..15], 1u16.to_be_bytes());

    let (body, checksum) = bytes.split_at(bytes.len() - 4);
    assert_eq!(&body[body.len() - FOOTER.len()..], FOOTER);
    let mut sum = Crc32::default();
    sum.update(body);
    assert_eq!(
        checksum,
        sum.value().to_be_bytes(),
        "the sum over every region, joined, is the sum over the file"
    );
}

#[test]
fn a_layer_is_its_area_and_its_ring_closed_in_micrometres() {
    let bytes = written(1);
    let footer = 4 + FOOTER.len() + 4;
    // The one layer is the last thing before the footer: area, one ring, its five points.
    let layer = &bytes[bytes.len() - footer - 2 - (8 + 4 + 5 * 8)..bytes.len() - footer - 2];
    let word =
        |at: usize| u32::from_be_bytes([layer[at], layer[at + 1], layer[at + 2], layer[at + 3]]);
    assert_eq!(
        word(0),
        80,
        "8 px of 0.1 mm square, 0.08 mm2, times a thousand"
    );
    assert_eq!((word(4), word(8)), (1, 5));
    let points: Vec<(i32, i32)> = (0..5)
        .map(|index| {
            (
                word(12 + index * 8).cast_signed(),
                word(16 + index * 8).cast_signed(),
            )
        })
        .collect();
    assert_eq!(
        points[0],
        (200, 100),
        "the block's corner at column 2, row 1"
    );
    assert_eq!(points[4], points[0], "closed by its first point");
}

#[test]
fn a_written_file_reads_back() {
    let mut open = CxlineReader
        .open(Cursor::new(written(3)))
        .expect("a .cxline we wrote");
    let facts = open.facts();
    assert_eq!(facts.layer_count(), 3);
    assert_eq!((facts.width_px, facts.height_px), (8, 4));
    assert_eq!(
        facts.machine.as_deref(),
        Some("Halot Test CL-60"),
        "passed through"
    );
    assert_eq!(
        facts.volume_mm3,
        Some(1500.0),
        "the header is written again at the end"
    );

    let lit: u32 = open
        .layer(2)
        .expect("the last layer")
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 8);
}

#[test]
fn another_container_of_the_family_is_not_taken_for_this_one() {
    let mut bytes = 9u32.to_be_bytes().to_vec();
    bytes.extend_from_slice(b"CXSW3DV2\0");
    let err = CxlineReader
        .open(Cursor::new(bytes))
        .err()
        .expect("a .cxdlp magic");
    assert!(matches!(
        err,
        FormatError::NotThisFormat {
            format: "cxline",
            found: 9
        }
    ));
}

#[test]
fn a_revision_other_than_the_one_written_is_refused() {
    let mut bytes = written(1);
    bytes[13..15].copy_from_slice(&2u16.to_be_bytes());
    let err = CxlineReader
        .open(Cursor::new(bytes))
        .err()
        .expect("revision 2");
    assert!(matches!(
        err,
        FormatError::UnsupportedVersion {
            format: "cxline",
            version: 2
        }
    ));
}

#[test]
fn a_file_cut_short_inside_a_layer_is_refused() {
    let mut bytes = written(1);
    // The one ring's point count, made to claim far more points than the file holds.
    let count_at = bytes.len() - (4 + FOOTER.len() + 4) - 2 - 5 * 8 - 4;
    bytes[count_at..count_at + 4].copy_from_slice(&1000u32.to_be_bytes());
    let err = CxlineReader
        .open(Cursor::new(bytes))
        .err()
        .expect("the ring runs past the end");
    assert!(matches!(err, FormatError::AddressPastEnd { .. }));
}

#[test]
fn a_ring_running_past_its_layer_is_refused() {
    let err = super::rings::contours_of(&[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 9])
        .expect_err("nine points in no bytes");
    assert!(matches!(err, FormatError::MalformedRun { offset: 12 }));
}

#[test]
fn a_file_without_the_panel_in_millimetres_is_refused() {
    let mut job = sample_job(1);
    job.printer.display.width_mm = 0.0;
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = CxlineWriter.begin(&job, &mut file).expect("sound");
        sink.push(CxlineSink::encode(&block())).expect("a layer");
        sink.finish(0.0).expect("written");
    }
    let err = CxlineReader
        .open(Cursor::new(file.into_inner()))
        .err()
        .expect("no pitch to fill the rings on");
    assert!(matches!(err, FormatError::Missing { .. }));
}

#[test]
fn a_stack_past_sixteen_bits_is_refused() {
    let mut job = sample_job(1);
    job.raster.width_px = 70_000;
    job.printer.display.width_px = 70_000;
    let err = CxlineWriter
        .begin(&job, &mut Cursor::new(Vec::new()))
        .err()
        .expect("the header cannot state it");
    assert!(matches!(
        err,
        FormatError::Encoding {
            what: "the header",
            ..
        }
    ));
}

#[test]
fn a_layer_of_the_wrong_size_or_count_is_refused() {
    let mut file = Cursor::new(Vec::new());
    let mut sink = CxlineWriter
        .begin(&sample_job(1), &mut file)
        .expect("sound");
    let err = sink
        .push(CxlineSink::encode(&LayerRuns::builder(4, 4).finish()))
        .unwrap_err();
    assert!(matches!(
        err,
        FormatError::ResolutionMismatch { index: 0, .. }
    ));
    sink.push(CxlineSink::encode(&block())).expect("the first");
    let err = sink.push(CxlineSink::encode(&block())).unwrap_err();
    assert!(matches!(
        err,
        FormatError::LayerCountMismatch {
            expected: 1,
            written: 2
        }
    ));

    let mut file = Cursor::new(Vec::new());
    let sink = CxlineWriter
        .begin(&sample_job(2), &mut file)
        .expect("sound");
    assert!(matches!(
        sink.finish(0.0).unwrap_err(),
        FormatError::LayerCountMismatch {
            expected: 2,
            written: 0
        }
    ));
}
