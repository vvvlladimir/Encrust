use std::io::Cursor;

use core_format::{FormatError, LayerSink, OpenFile, SlicedFileReader, SlicedFileWriter};
use core_raster::LayerRuns;

use super::header::HEADER_BYTES;
use super::rle::{self, FdgLayer};
use super::{FdgReader, FdgSink, FdgWriter};
use crate::fixtures::{lit_block, sample_job, two_stage_job, written_by};

fn written(layers: u32) -> Vec<u8> {
    let mut job = sample_job(layers);
    job.material.light_pwm = 200;
    job.material.bottom_light_pwm = 100;
    job.material.waits = printer_profiles::Waits {
        mode: printer_profiles::WaitMode::Rest,
        before_lift_s: 1.5,
        after_lift_s: 0.5,
        after_retract_s: 2.0,
    };
    written_by(&FdgWriter, &job)
}

#[test]
fn a_row_and_its_middle_each_open_with_a_colour_byte() {
    let data = FdgLayer::encode(&lit_block()).data().to_vec();
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
fn a_lit_first_pixel_reads_back_lit() {
    let mut builder = LayerRuns::builder(8, 1);
    builder.push(2, 255);
    builder.push(6, 0);
    let layer = builder.finish();

    let runs = rle::decode(FdgLayer::encode(&layer).data(), 8).expect("our own bytes");
    let lit: Vec<(u32, u8)> = runs.iter().map(|run| (run.length, run.value)).collect();
    assert_eq!(lit, [(2, 255), (6, 0)]);
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
    let mut open = FdgReader
        .open(Cursor::new(bytes))
        .expect("an .fdg we wrote");
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
fn a_written_file_covers_its_layers_with_a_key_and_reads_back() {
    let bytes = written(2);
    let key = u32::from_le_bytes([bytes[0x5C], bytes[0x5D], bytes[0x5E], bytes[0x5F]]);
    assert_ne!(key, 0, "the vendor never writes a zero key");
    let mut open = FdgReader.open(Cursor::new(bytes)).expect("the header");
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
    let word =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    assert_eq!(word(0), 0xBD3C_7AC8);
    assert_eq!(
        word(0x2C),
        HEADER_BYTES as u32,
        "the large preview follows the header"
    );
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
fn the_block_at_0xd8_carries_the_rests_as_the_ctb_v4_parameters_do() {
    let bytes = written(1);
    let f32_at =
        |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    assert_eq!(
        (f32_at(0xC0), f32_at(0xC4)),
        (2.0, 0.5),
        "after retract, after lift"
    );
    assert_eq!((f32_at(0xE4), f32_at(0xEC)), (4.0, 4.0));
    assert_eq!(
        (f32_at(0xF0), f32_at(0xF4), f32_at(0xF8)),
        (2.0, 0.5, 1.5),
        "after retract, after lift, before lift"
    );
}

#[test]
fn the_header_states_the_anti_alias_level_the_edges_were_drawn_with() {
    let bytes = written(1);
    let word =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    assert_eq!(word(0x60), 16, "every grey, capped at sixteen");
    assert_eq!(word(0x3C), 1, "a constant of the vendor writer");
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
    sink.push(FdgSink::encode(&lit_block()))
        .expect("the first layer");
    let err = sink.push(FdgSink::encode(&lit_block())).unwrap_err();
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
    assert!(matches!(
        err,
        FormatError::ResolutionMismatch { index: 0, .. }
    ));
}

#[test]
fn the_header_states_the_bottom_light_off_and_no_height() {
    let mut job = sample_job(3);
    job.material.waits.mode = printer_profiles::WaitMode::LightOff;
    job.material.light_off_delay_s = 0.5;
    job.material.bottom_light_off_delay_s = Some(5.0);
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = FdgWriter.begin(&job, &mut file).expect("the job is sound");
        for _ in 0..3 {
            sink.push(FdgSink::encode(&lit_block())).expect("a layer");
        }
        sink.finish(1500.0).expect("every promised layer arrived");
    }
    let bytes = file.into_inner();
    let f32_at =
        |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    assert_eq!(f32_at(0x4C), 0.0, "the vendor leaves the height zero");
    assert_eq!(
        (f32_at(0x7C), f32_at(0x80)),
        (5.0, 0.5),
        "bottom, then normal light off"
    );
}

#[test]
fn a_two_stage_peel_states_both_climbs_summed_then_the_second_stage() {
    let job = two_stage_job(1);
    let mut file = Cursor::new(Vec::new());
    {
        let mut sink = FdgWriter.begin(&job, &mut file).expect("the job is sound");
        sink.push(FdgSink::encode(&lit_block())).expect("a layer");
        sink.finish(1500.0).expect("every promised layer arrived");
    }
    let bytes = file.into_inner();
    let f32_at =
        |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    assert_eq!(
        (f32_at(0x88), f32_at(0x90)),
        (8.0, 6.0),
        "bottom lift 5 + 3 mm, lift 2 + 4 mm"
    );
    let second: Vec<f32> = (0x9C..0xB4).step_by(4).map(f32_at).collect();
    assert_eq!(second, [3.0, 180.0, 4.0, 900.0, 4.0, 360.0]);
    assert_eq!(
        (f32_at(0xDC), f32_at(0xFC)),
        (120.0, 3.0),
        "the second bottom retract's speed and distance, as in the .ctb v4 block"
    );
}

#[test]
fn a_two_stage_peel_reads_back_as_its_two_stages() {
    let job = two_stage_job(1);
    let open = FdgReader
        .open(Cursor::new(written_by(&FdgWriter, &job)))
        .expect("a .fdg we wrote");
    let stated = &open.facts().stated;
    assert_eq!(stated.second_stage, Some(job.material.second_stage));
    assert_eq!(
        (stated.bottom_lift_distance_mm, stated.lift_distance_mm),
        (Some(5.0), Some(2.0)),
        "the first stage, split off the summed climbs"
    );
    assert_eq!(
        stated.retract_distance_mm,
        Some(2.0),
        "6 mm back, 4 of them slow"
    );
    assert_eq!(
        stated.bottom_light_off_delay_s,
        Some(job.material.bottom_light_off_s())
    );
}
