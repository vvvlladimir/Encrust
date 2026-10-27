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
    assert_eq!((f32_at(row(0) + 8), f32_at(row(0) + 12)), (9.0, 0.5));
    assert_eq!(
        (f32_at(row(1) + 8), f32_at(row(1) + 12)),
        (5.0, 1.0),
        "a speed is millimetres a second in this container"
    );
}
