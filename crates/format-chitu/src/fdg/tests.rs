use std::io::Cursor;

use core_format::{FormatError, LayerSink, OpenFile, SlicedFileReader, SlicedFileWriter};
use core_raster::LayerRuns;

use super::header::HEADER_BYTES;
use super::rle::{self, FdgLayer};
use super::{FdgReader, FdgSink, FdgWriter};
use crate::fixtures::sample_job;

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
    let mut job = sample_job(layers);
    job.material.light_pwm = 200;
    job.material.bottom_light_pwm = 100;
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = FdgWriter.begin(&job, &mut file).expect("the job is sound");
        for _ in 0..layers {
            sink.push(FdgSink::encode(&block())).expect("a layer");
        }
        sink.finish(1500.0).expect("every promised layer arrived");
    }
    file.into_inner()
}

#[test]
fn a_row_and_its_middle_each_open_with_a_colour_byte() {
    let data = FdgLayer::encode(&block()).data().to_vec();
    assert_eq!(
        data,
        [
            0x80, 3, 0x80, 3, // row 0: the opening byte and three more, the middle and three
            0x80, 1, 0xFC, 1, 0xFC, 1, 0x80, 1, // row 1: two dark, two lit, two lit, two dark
            0x80, 1, 0xFC, 1, 0xFC, 1, 0x80, 1, // row 2 the same
            0x80, 3, 0x80, 3, // row 3 as row 0
        ]
    );
}

#[test]
fn a_long_stretch_is_counted_in_bytes_of_at_most_125() {
    let mut builder = LayerRuns::builder(1000, 1);
    builder.push(1000, 0);
    let data = FdgLayer::encode(&builder.finish()).data().to_vec();
    // 499 pixels behind the opening byte, then the middle's colour byte and 499 more.
    let half = [0x7D, 0x7D, 0x7D, 124];
    let expected: Vec<u8> = [&[0x80][..], &half, &[0x80], &half].concat();
    assert_eq!(data, expected);
}

#[test]
fn the_codec_keeps_what_is_lit_and_drops_what_is_below_half() {
    let mut builder = LayerRuns::builder(8, 2);
    builder.push(3, 0);
    builder.push(5, 200);
    builder.push(4, 100);
    builder.push(4, 255);
    let layer = builder.finish();

    let runs = rle::decode(FdgLayer::encode(&layer).data(), 16).expect("our own bytes");
    let lit: Vec<(u32, u8)> = runs.iter().map(|run| (run.length, run.value)).collect();
    assert_eq!(lit, [(3, 0), (5, 255), (4, 0), (4, 255)]);
}

#[test]
fn runs_past_the_panel_are_refused() {
    let err = rle::decode(&[0x80, 0x7D], 8).unwrap_err();
    assert!(matches!(err, FormatError::MalformedRun { offset: 1 }));
}

#[test]
fn the_cipher_undoes_itself_and_zero_leaves_the_bytes_alone() {
    let clear: Vec<u8> = (0..23).collect();
    let mut covered = clear.clone();
    rle::crypt(0x1234_5678, 7, &mut covered);
    assert_ne!(covered, clear);
    rle::crypt(0x1234_5678, 7, &mut covered);
    assert_eq!(covered, clear);

    let mut untouched = clear.clone();
    rle::crypt(0, 7, &mut untouched);
    assert_eq!(untouched, clear);
}

#[test]
fn a_written_file_reads_back() {
    let bytes = written(3);
    let mut open = FdgReader.open(Cursor::new(bytes)).expect("an .fdg we wrote");
    let facts = open.facts();
    assert_eq!(facts.layer_count(), 3);
    assert_eq!((facts.width_px, facts.height_px), (8, 4));
    assert_eq!(facts.machine.as_deref(), Some("Test"));

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
fn a_file_with_its_layers_covered_by_a_key_reads_back() {
    let mut bytes = written(2);
    let key = 0x00C0_FFEE_u32;
    bytes[0x5C..0x60].copy_from_slice(&key.to_le_bytes());
    let mut open = FdgReader
        .open(Cursor::new(bytes.clone()))
        .expect("the header");
    for index in 0..2 {
        let entry = open.facts().layers[index as usize];
        let at = entry.offset as usize;
        rle::crypt(key, index, &mut bytes[at..at + entry.size as usize]);
    }
    open = FdgReader.open(Cursor::new(bytes)).expect("the header");
    let lit: u32 = open
        .layer(1)
        .expect("a covered layer")
        .iter()
        .filter(|run| run.value > 0)
        .map(|run| run.length)
        .sum();
    assert_eq!(lit, 8);
}

#[test]
fn the_header_counts_the_heights_and_names_its_blocks() {
    let bytes = written(3);
    let word = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    assert_eq!(word(0), 0xBD3C_7AC8);
    assert_eq!(word(0x2C), HEADER_BYTES as u32, "the large preview follows the header");
    assert_eq!((word(0xCC), word(0xD0)), (0xD8, 0x50));
    assert_eq!(word(0x10C), 3, "three layers at three heights");
}

#[test]
fn the_bottom_light_power_comes_before_the_light_power() {
    let bytes = written(1);
    assert_eq!(&bytes[0x40..0x44], &[100, 0, 200, 0]);

    let open = FdgReader
        .open(Cursor::new(bytes))
        .expect("an .fdg we wrote");
    assert_eq!(open.facts().stated.light_pwm, Some(200));
    assert_eq!(open.facts().stated.bottom_light_pwm, Some(100));
}

#[test]
fn a_file_of_another_container_is_refused_by_its_magic() {
    let err = FdgReader
        .open(Cursor::new(0x12FD_0106u32.to_le_bytes().to_vec()))
        .err()
        .expect("a .ctb magic");
    assert!(matches!(
        err,
        FormatError::NotThisFormat {
            format: "fdg",
            found: 0x12FD_0106
        }
    ));
}

#[test]
fn a_layer_count_other_than_promised_is_refused() {
    let mut buffer = Cursor::new(Vec::new());
    let mut sink = FdgWriter
        .begin(&sample_job(1), &mut buffer)
        .expect("the job is sound");
    sink.push(FdgSink::encode(&block())).expect("the first layer");
    let err = sink.push(FdgSink::encode(&block())).unwrap_err();
    assert!(matches!(
        err,
        FormatError::LayerCountMismatch {
            expected: 1,
            written: 2
        }
    ));

    let mut buffer = Cursor::new(Vec::new());
    let sink = FdgWriter
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

#[test]
fn a_layer_of_the_wrong_size_is_refused() {
    let mut buffer = Cursor::new(Vec::new());
    let mut sink = FdgWriter
        .begin(&sample_job(1), &mut buffer)
        .expect("the job is sound");
    let err = sink
        .push(FdgLayer::encode(&LayerRuns::builder(4, 4).finish()))
        .unwrap_err();
    assert!(matches!(err, FormatError::ResolutionMismatch { index: 0, .. }));
}
