use proptest::prelude::*;

use super::*;

fn layer_of(width: u32, pixels: &[u8]) -> LayerRuns {
    let mut builder = LayerRuns::builder(width, pixels.len() as u32 / width);
    for &value in pixels {
        builder.push(1, value);
    }
    builder.finish()
}

/// Every pixel of the panel the two halves of `encoded` carry, row by row.
fn expanded(encoded: &EncodedHalves) -> Vec<u8> {
    let [left, right] = encoded
        .halves()
        .clone()
        .map(|half| decode_half(&half.data).expect("what the encoder wrote decodes"));
    let joined = join_halves(&left, &right, encoded.width() / 2, encoded.height())
        .expect("both halves cover the panel");
    joined
        .iter()
        .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
        .collect()
}

#[test]
fn a_dark_half_and_a_lit_half_are_one_run_each() {
    let encoded = EncodedHalves::encode(&layer_of(4, &[0, 0, 255, 255]));
    let [left, right] = encoded.halves();
    assert_eq!(left.data, [0x10], "two dark pixels in the short form");
    assert_eq!(
        right.data,
        [0xA7, 0x40],
        "a step of +7, then a length of two"
    );
    assert_eq!(left.checksum, !0x10);
}

#[test]
fn a_run_past_seven_pixels_carries_its_length_after_the_escape() {
    let encoded = EncodedHalves::encode(&layer_of(40, &[0; 40]));
    // Twenty pixels: escape, then 20 - 8 = 12 in the four-bit field.
    assert_eq!(encoded.halves()[0].data, [0x38, 0x4C]);
}

#[test]
fn a_vendor_layer_decodes_to_the_runs_it_was_cut_from() {
    // The first bytes of a Jupiter 2 layer: a dark lead-in, then a lit and a dark run.
    let runs = decode_half(&[
        0x38, 0x73, 0xB1, 0xEB, 0x10, 0xA7, 0x59, 0x0F, 0x99, 0x6B, 0xC8, 0x01,
    ])
    .expect("vendor bytes");
    let lengths: Vec<(u32, u8)> = runs.iter().map(|run| (run.length, run.value)).collect();
    assert_eq!(lengths, [(17_742_619, 0), (251, 255), (7309, 0)]);
}

#[test]
fn a_length_field_with_nothing_in_front_of_it_is_refused() {
    assert!(matches!(
        decode_half(&[0x41]),
        Err(FormatError::MalformedRun { offset: 0 })
    ));
}

#[test]
fn a_step_past_the_brightest_level_is_refused() {
    assert!(matches!(
        decode_half(&[0xA7, 0xA1]),
        Err(FormatError::MalformedRun { offset: 1 })
    ));
}

#[test]
fn a_short_run_of_another_level_is_refused() {
    assert!(matches!(
        decode_half(&[0x09]),
        Err(FormatError::MalformedRun { offset: 0 })
    ));
}

#[test]
fn halves_that_fall_short_of_the_panel_are_refused() {
    let short = [Run {
        length: 3,
        value: 0,
    }];
    assert!(matches!(
        join_halves(&short, &short, 2, 2),
        Err(FormatError::MalformedRun { .. })
    ));
}

proptest! {
    #[test]
    fn a_layer_comes_back_with_each_grey_on_its_level(
        rows in 1_u32..6,
        half in 1_u32..9,
        seed in proptest::collection::vec(any::<u8>(), 96),
    ) {
        let width = 2 * half;
        let pixels: Vec<u8> = (0..width * rows)
            .map(|index| seed[index as usize % seed.len()])
            .collect();
        let back = expanded(&EncodedHalves::encode(&layer_of(width, &pixels)));
        let expected: Vec<u8> = pixels.iter().map(|&value| grey_of(value >> 5)).collect();
        prop_assert_eq!(back, expected);
    }
}
