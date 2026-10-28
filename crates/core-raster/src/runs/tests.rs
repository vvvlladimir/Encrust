use super::*;

fn runs_of(width: u32, values: &[(u32, u8)]) -> LayerRuns {
    let mut builder = LayerRuns::builder(width, 1);
    for &(length, value) in values {
        builder.push(length, value);
    }
    builder.finish()
}

#[test]
fn reversing_the_rows_puts_the_last_row_first() {
    let mut builder = LayerRuns::builder(3, 3);
    for (length, value) in [(1, 9), (3, 0), (5, 7)] {
        builder.push(length, value);
    }
    let reversed = builder.finish().rows_reversed();
    assert_eq!(
        reversed.to_mask().pixels(),
        &[7, 7, 7, 0, 7, 7, 9, 0, 0],
        "row 2, row 1, row 0"
    );
    assert_eq!(reversed.rows_reversed().rows_reversed(), reversed);
}

#[test]
fn a_pixel_lit_at_any_grey_counts_as_lit() {
    assert_eq!(runs_of(8, &[(2, 0), (3, 1), (1, 0), (2, 255)]).lit_px(), 5);
}

#[test]
fn the_brighter_pixel_of_two_layers_wins() {
    let a = runs_of(6, &[(2, 255), (4, 0)]);
    let b = runs_of(6, &[(1, 0), (2, 100), (3, 0)]);

    let both = a.brightest_of(&b);
    assert_eq!(both.to_mask().pixels(), &[255, 255, 100, 0, 0, 0]);
    assert_eq!(
        both.to_mask().pixels(),
        b.brightest_of(&a).to_mask().pixels(),
        "the brighter of two does not depend which is asked"
    );
}

#[test]
fn a_union_covers_the_whole_panel_however_the_runs_line_up() {
    let a = runs_of(4, &[(4, 0)]);
    let b = runs_of(4, &[(1, 9), (1, 8), (1, 7), (1, 6)]);
    assert_eq!(a.brightest_of(&b).pixel_count(), 4);
    assert_eq!(a.brightest_of(&b).to_mask().pixels(), &[9, 8, 7, 6]);
}

#[test]
fn a_layer_nobody_wrote_to_is_one_dark_run() {
    let layer = LayerRuns::builder(4, 2).finish();
    assert_eq!(
        layer.runs(),
        &[Run {
            length: 8,
            value: 0
        }]
    );
    assert!(layer.is_blank());
}

#[test]
fn neighbouring_runs_of_one_value_are_merged() {
    let mut builder = LayerRuns::builder(4, 1);
    builder.push(1, 200);
    builder.push(2, 200);
    let layer = builder.finish();

    assert_eq!(
        layer.runs(),
        &[
            Run {
                length: 3,
                value: 200
            },
            Run {
                length: 1,
                value: 0
            }
        ]
    );
}

#[test]
fn padding_skips_to_a_position_and_never_backwards() {
    let mut builder = LayerRuns::builder(4, 2);
    builder.pad_to(5);
    builder.push(1, 255);
    // Already past pixel 3, so this adds nothing.
    builder.pad_to(3);
    builder.push(1, 255);
    let layer = builder.finish();

    assert_eq!(
        layer.to_mask().pixels(),
        &[0, 0, 0, 0, 0, 255, 255, 0],
        "the two lit pixels sit where they were pushed"
    );
}

#[test]
fn runs_past_the_end_of_the_layer_are_dropped() {
    let mut builder = LayerRuns::builder(2, 1);
    builder.push(100, 255);
    let layer = builder.finish();

    assert_eq!(
        layer.runs(),
        &[Run {
            length: 2,
            value: 255
        }]
    );
    assert_eq!(layer.to_mask().pixels(), &[255, 255]);
}

#[test]
fn a_mask_survives_the_trip_through_runs() {
    let mut mask = LayerMask::new(5, 2);
    mask.pixels_mut()
        .copy_from_slice(&[0, 0, 7, 7, 7, 7, 255, 0, 0, 0]);
    let layer = LayerRuns::from_mask(&mask);

    assert_eq!(layer.to_mask(), mask);
    assert_eq!(layer.runs().len(), 4, "got {:?}", layer.runs());
}

#[test]
fn a_run_longer_than_the_scan_stride_is_found_in_one_piece() {
    let mut mask = LayerMask::new(SCAN_STRIDE as u32 * 3 + 5, 1);
    mask.pixels_mut().fill(9);
    let layer = LayerRuns::from_mask(&mask);

    assert_eq!(
        layer.runs(),
        &[Run {
            length: SCAN_STRIDE as u32 * 3 + 5,
            value: 9
        }]
    );
}

#[test]
fn coverage_counts_a_half_lit_pixel_as_a_half() {
    let mut builder = LayerRuns::builder(2, 2);
    builder.push(1, 255);
    builder.push(1, 128);
    let layer = builder.finish();

    assert!((layer.coverage() - 1.502).abs() < 1e-3);
}

#[test]
fn coverage_of_a_panel_sized_layer_does_not_overflow() {
    let mut builder = LayerRuns::builder(8520, 4320);
    builder.push(8520 * 4320, 255);
    let layer = builder.finish();

    let expected = (8520u64 * 4320) as f32;
    assert!((layer.coverage() - expected).abs() < expected * 1e-6);
}
