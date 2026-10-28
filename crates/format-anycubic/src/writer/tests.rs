use std::io::Cursor;

use super::*;
use crate::fixtures::sample_job;

fn layer() -> LayerRuns {
    LayerRuns::builder(8, 4).finish()
}

#[test]
fn every_extension_round_trips_through_its_own_name() {
    for flavour in AnycubicFlavour::ALL {
        assert_eq!(
            AnycubicFlavour::of_extension(flavour.extension()),
            Some(flavour)
        );
    }
    assert_eq!(
        AnycubicFlavour::of_extension("PWMX"),
        Some(AnycubicFlavour::Pwmx)
    );
    assert_eq!(AnycubicFlavour::of_extension("goo"), None);
    assert_eq!(AnycubicWriter::default().extension(), "pwmx");
}

#[test]
fn a_panel_past_the_machine_blocks_sixteen_bits_is_refused() {
    let mut job = sample_job(1);
    job.raster.width_px = 70_000;
    job.printer.display.width_px = 70_000;
    let err = AnycubicWriter::new(AnycubicFlavour::Pwmx, AnycubicVersion::V516)
        .begin(&job, &mut Cursor::new(Vec::new()))
        .err()
        .expect("the machine block cannot state the panel");
    assert!(matches!(
        err,
        FormatError::Encoding {
            what: "the machine block",
            ..
        }
    ));
}

#[test]
fn a_job_without_layers_is_rejected_before_anything_is_written() {
    let mut buffer = Cursor::new(Vec::new());
    let err = AnycubicWriter::default()
        .begin(&sample_job(0), &mut buffer)
        .err()
        .expect("an empty job cannot be written");
    assert!(matches!(err, FormatError::EmptyJob));
    assert!(buffer.into_inner().is_empty());
}

#[test]
fn a_layer_of_the_wrong_size_is_rejected() {
    let mut buffer = Cursor::new(Vec::new());
    let mut sink = AnycubicWriter::default()
        .begin(&sample_job(1), &mut buffer)
        .expect("the job is sound");
    let err = sink
        .push(AnycubicSink::encode(&LayerRuns::builder(4, 4).finish()))
        .unwrap_err();
    assert!(matches!(
        err,
        FormatError::ResolutionMismatch { index: 0, .. }
    ));
}

#[test]
fn more_layers_than_promised_are_refused() {
    let mut buffer = Cursor::new(Vec::new());
    let mut sink = AnycubicWriter::default()
        .begin(&sample_job(1), &mut buffer)
        .expect("the job is sound");
    sink.push(AnycubicSink::encode(&layer()))
        .expect("the first layer");
    let err = sink.push(AnycubicSink::encode(&layer())).unwrap_err();
    assert!(matches!(
        err,
        FormatError::LayerCountMismatch {
            expected: 1,
            written: 2
        }
    ));
}

#[test]
fn fewer_layers_than_promised_are_refused_at_the_end() {
    let mut buffer = Cursor::new(Vec::new());
    let sink = AnycubicWriter::default()
        .begin(&sample_job(2), &mut buffer)
        .expect("the job is sound");
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
fn the_model_block_states_what_the_layers_light() {
    // Column 2 of row 1 in the first layer, columns 5 and 6 of row 3 in the second.
    let lit = |at: &[(u32, u32)]| {
        let mut builder = LayerRuns::builder(8, 4);
        let mut cursor = 0;
        for &(start, length) in at {
            builder.push(start - cursor, 0);
            builder.push(length, 255);
            cursor = start + length;
        }
        builder.push(32 - cursor, 0);
        builder.finish()
    };
    let job = sample_job(2);
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut sink = AnycubicWriter::default()
            .begin(&job, &mut buffer)
            .expect("the job is sound");
        sink.push(AnycubicSink::encode(&lit(&[(10, 1)])))
            .expect("a layer");
        sink.push(AnycubicSink::encode(&lit(&[(29, 2)])))
            .expect("a layer");
        sink.finish(0.0).expect("every promised layer arrived");
    }
    let bytes = buffer.into_inner();
    let model = tables::Offsets::of(2).model as usize + 12;
    let f32_at =
        |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    // The panel is 0.8 x 0.4 mm about its centre; the lit columns run from 2 to 6.
    let expected = [-0.2, -0.1, 0.0, 0.3, 0.2, job.height_mm()];
    for (field, want) in expected.into_iter().enumerate() {
        let got = f32_at(model + 4 * field);
        assert!(
            (got - want).abs() < 1e-5,
            "field {field} is {got}, expected {want}"
        );
    }
}

#[test]
fn a_bottom_layer_lifts_by_its_own_numbers() {
    let mut job = sample_job(3);
    job.material.bottom_layers = 1;
    job.material.bottom_lift_distance_mm = 9.0;
    job.material.bottom_lift_speed_mm_min = 30.0;
    job.material.lift_distance_mm = 5.0;
    job.material.lift_speed_mm_min = 60.0;

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut sink = AnycubicWriter::default()
            .begin(&job, &mut buffer)
            .expect("the job is sound");
        for _ in 0..3 {
            sink.push(AnycubicSink::encode(&layer())).expect("a layer");
        }
        sink.finish(0.0).expect("every promised layer arrived");
    }
    let bytes = buffer.into_inner();
    let table =
        tables::Offsets::of(3).layer_table as usize + tables::LAYER_TABLE_HEAD_BYTES as usize;
    let f32_at =
        |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);

    let row = |index: usize| table + index * LAYER_DEF_BYTES as usize;
    assert_eq!((f32_at(row(0) + 8), f32_at(row(0) + 12)), (9.0, 30.0));
    assert_eq!(
        (f32_at(row(1) + 8), f32_at(row(1) + 12)),
        (5.0, 60.0),
        "a row's speed is millimetres a minute, as the vendor writes it"
    );
}

#[test]
fn a_second_stage_turns_advanced_mode_on_and_fills_the_extra_block() {
    let mut job = sample_job(1);
    job.material.bottom_lift_distance_mm = 5.0;
    job.material.bottom_lift_speed_mm_min = 120.0;
    job.material.bottom_retract_speed_mm_min = 240.0;
    job.material.second_stage.bottom_lift_distance_mm = 3.0;
    job.material.second_stage.bottom_lift_speed_mm_min = 180.0;
    job.material.second_stage.bottom_retract_speed_mm_min = 180.0;

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut sink = AnycubicWriter::default()
            .begin(&job, &mut buffer)
            .expect("the job is sound");
        sink.push(AnycubicSink::encode(&layer())).expect("a layer");
        sink.finish(0.0).expect("every promised layer arrived");
    }
    let bytes = buffer.into_inner();
    let at = |offset: usize| {
        [
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]
    };
    let header = tables::Offsets::of(1).header as usize + 16;
    assert_eq!(u32::from_le_bytes(at(header + 0x50)), 1, "advanced mode");

    let extra = tables::Offsets::of(1).extra as usize + 16;
    let bottom: Vec<f32> = (0..6)
        .map(|i| f32::from_le_bytes(at(extra + 4 + 4 * i)))
        .collect();
    assert_eq!(
        bottom,
        [5.0, 2.0, 3.0, 3.0, 3.0, 4.0],
        "first lift with the second retract, then the second lift with the first retract"
    );
}
