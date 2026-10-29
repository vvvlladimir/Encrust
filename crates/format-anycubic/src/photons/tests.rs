use std::io::Cursor;

use core_format::{FormatError, LayerSink, OpenFile, SlicedFileReader, SlicedFileWriter};
use core_raster::LayerRuns;

use super::codec::{self, PhotonsLayer};
use super::header::{HEADER_BYTES, PREVIEW_PX};
use super::{PhotonsReader, PhotonsSink, PhotonsWriter};
use crate::fixtures::{lit_block, sample_job, written_by};

fn written(layers: u32) -> Vec<u8> {
    written_by(&PhotonsWriter, &sample_job(layers))
}

#[test]
fn a_stretch_is_its_colour_and_length_less_one_with_the_bits_reversed() {
    let data = PhotonsLayer::encode(&lit_block()).data().to_vec();
    // 10 dark, 4 lit across rows 1 and 2's join, 4 dark, 4 lit, 10 dark.
    assert_eq!(data, [0x90, 0xC1, 0xC0, 0xC1, 0x90]);
}

#[test]
fn a_long_stretch_is_split_at_128_pixels() {
    let mut builder = LayerRuns::builder(300, 1);
    builder.push(300, 0);
    let data = PhotonsLayer::encode(&builder.finish()).data().to_vec();
    assert_eq!(data, [0xFE, 0xFE, 43u8.reverse_bits()]);
}

#[test]
fn the_codec_keeps_what_is_lit_and_drops_what_is_below_half() {
    let mut builder = LayerRuns::builder(8, 2);
    builder.push(3, 0);
    builder.push(5, 200);
    builder.push(4, 100);
    builder.push(4, 255);
    let layer = builder.finish();

    let runs = codec::decode(PhotonsLayer::encode(&layer).data(), 16).expect("our own bytes");
    let lit: Vec<(u32, u8)> = runs.iter().map(|run| (run.length, run.value)).collect();
    assert_eq!(lit, [(3, 0), (5, 255), (4, 0), (4, 255)]);
}

#[test]
fn runs_past_the_panel_are_refused() {
    let err = codec::decode(&[0xFE, 0xFE], 200).unwrap_err();
    assert!(matches!(err, FormatError::MalformedRun { offset: 1 }));
}

#[test]
fn the_record_ends_with_the_sides_bit_reversed() {
    let mut builder = LayerRuns::builder(1440, 2560);
    builder.push(1440 * 2560, 0);
    let mut file = Cursor::new(Vec::new());
    {
        let mut fields = core_format::Fields::new(&mut file);
        PhotonsLayer::encode(&builder.finish())
            .write(&mut fields)
            .expect("in memory");
    }
    let record = file.into_inner();
    // A Photon S file carries 0xA0055000 in this word for its 1440 x 2560 panel.
    assert_eq!(record[0x18..0x1C], [0xA0, 0x05, 0x50, 0x00]);
}

#[test]
fn a_layer_that_ends_lit_does_not_count_its_last_stretch() {
    let mut builder = LayerRuns::builder(4, 1);
    builder.push(1, 255);
    builder.push(1, 0);
    builder.push(2, 255);
    let mut file = Cursor::new(Vec::new());
    {
        let mut fields = core_format::Fields::new(&mut file);
        PhotonsLayer::encode(&builder.finish())
            .write(&mut fields)
            .expect("in memory");
    }
    let record = file.into_inner();
    assert_eq!(
        record[..4],
        1u32.to_be_bytes(),
        "the count the format states"
    );
}

#[test]
fn a_layer_past_what_its_length_word_counts_is_refused() {
    assert_eq!(codec::length_bits(2).expect("two bytes fit"), 48);
    assert!(matches!(
        codec::length_bits(1 << 29),
        Err(FormatError::Encoding { .. })
    ));
}

#[test]
fn a_preview_too_large_to_address_is_refused() {
    let mut bytes = written(1);
    // The preview's width at 82, then four bytes, then its height.
    bytes[82..86].fill(0xFF);
    bytes[90..94].fill(0xFF);
    let err = PhotonsReader
        .open(Cursor::new(bytes))
        .err()
        .expect("no file holds that preview");
    assert!(matches!(err, FormatError::AddressPastEnd { .. }), "{err:?}");
}

#[test]
fn a_written_file_reads_back() {
    let bytes = written(3);
    let preview_bytes = u64::from(PREVIEW_PX.0 * PREVIEW_PX.1 * 2);
    assert_eq!(
        bytes[(HEADER_BYTES + preview_bytes) as usize..][..4],
        3u32.to_be_bytes()
    );

    let mut open = PhotonsReader
        .open(Cursor::new(bytes))
        .expect("a .photons we wrote");
    let facts = open.facts();
    assert_eq!(facts.layer_count(), 3);
    assert_eq!((facts.width_px, facts.height_px), (8, 4));
    assert_eq!(
        facts.volume_mm3,
        Some(1500.0),
        "the header is written again at the end"
    );
    let (width_mm, height_mm) = facts.display_mm.expect("the pixel size is stated");
    assert!(
        (width_mm - 0.8).abs() < 1e-5,
        "8 px of 0.1 mm, got {width_mm}"
    );
    assert!(
        (height_mm - 0.4).abs() < 1e-5,
        "4 px of 0.1 mm, got {height_mm}"
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
fn another_container_is_not_taken_for_this_one() {
    let err = PhotonsReader
        .open(Cursor::new(vec![0x12, 0xFD, 0x00, 0x19, 0, 0]))
        .err()
        .expect("a .ctb magic");
    assert!(matches!(
        err,
        FormatError::NotThisFormat {
            format: "photons",
            ..
        }
    ));
}

#[test]
fn a_panel_past_sixteen_bits_is_refused() {
    let mut job = sample_job(1);
    job.raster.width_px = 70_000;
    job.printer.display.width_px = 70_000;
    let err = PhotonsWriter
        .begin(&job, &mut Cursor::new(Vec::new()))
        .err()
        .expect("the record cannot state the panel");
    assert!(matches!(
        err,
        FormatError::Encoding {
            what: "a layer record",
            ..
        }
    ));
}

#[test]
fn a_job_without_layers_is_rejected() {
    let err = PhotonsWriter
        .begin(&sample_job(0), &mut Cursor::new(Vec::new()))
        .err()
        .expect("an empty job cannot be written");
    assert!(matches!(err, FormatError::EmptyJob));
}

#[test]
fn a_layer_of_the_wrong_size_is_rejected() {
    let mut file = Cursor::new(Vec::new());
    let mut sink = PhotonsWriter
        .begin(&sample_job(1), &mut file)
        .expect("sound");
    let err = sink
        .push(PhotonsSink::encode(&LayerRuns::builder(4, 4).finish()))
        .unwrap_err();
    assert!(matches!(
        err,
        FormatError::ResolutionMismatch { index: 0, .. }
    ));
}

#[test]
fn more_or_fewer_layers_than_promised_are_refused() {
    let mut file = Cursor::new(Vec::new());
    let mut sink = PhotonsWriter
        .begin(&sample_job(1), &mut file)
        .expect("sound");
    sink.push(PhotonsSink::encode(&lit_block()))
        .expect("the first");
    let err = sink.push(PhotonsSink::encode(&lit_block())).unwrap_err();
    assert!(matches!(
        err,
        FormatError::LayerCountMismatch {
            expected: 1,
            written: 2
        }
    ));

    let mut file = Cursor::new(Vec::new());
    let sink = PhotonsWriter
        .begin(&sample_job(2), &mut file)
        .expect("sound");
    let err = sink.finish(0.0).unwrap_err();
    assert!(matches!(
        err,
        FormatError::LayerCountMismatch {
            expected: 2,
            written: 0
        }
    ));
}

#[test]
fn a_panel_past_what_a_reader_holds_is_refused() {
    let mut bytes = written(1);
    // The first record's sides, behind the layer count and the record's first twelve bytes.
    let record = (HEADER_BYTES + u64::from(PREVIEW_PX.0 * PREVIEW_PX.1 * 2)) as usize + 4;
    bytes[record + 12..record + 20].fill(0xFF);
    let err = PhotonsReader
        .open(Cursor::new(bytes))
        .err()
        .expect("no panel is that large");
    assert!(matches!(err, FormatError::PanelTooLarge { .. }), "{err:?}");
}
