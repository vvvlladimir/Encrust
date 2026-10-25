use super::*;
use crate::pull::disc_pull_n;
use std::f64::consts::PI;

pub const UNIT: PixelPitch = PixelPitch { x: 1.0, y: 1.0 };

pub fn paint(width: u32, height: u32, grey: impl Fn(u32, u32) -> u8) -> LayerRuns {
    let mut builder = LayerRuns::builder(width, height);
    for y in 0..height {
        for x in 0..width {
            builder.push(1, grey(x, y));
        }
    }
    builder.finish()
}

fn rectangle(width: u32, height: u32) -> LayerRuns {
    paint(width + 2, height + 2, |x, y| {
        if (1..=width).contains(&x) && (1..=height).contains(&y) {
            255
        } else {
            0
        }
    })
}

#[test]
fn a_dark_layer_measures_nothing() {
    assert_eq!(
        measure(&paint(8, 8, |_, _| 0), UNIT),
        LayerMeasure::default()
    );
}

#[test]
fn a_painted_disc_pulls_as_the_disc_it_is() {
    // 200 px at 0.05 mm is a disc of radius 10 mm.
    let pitch = PixelPitch { x: 0.05, y: 0.05 };
    let disc = paint(400, 400, |x, y| {
        let (dx, dy) = (f64::from(x) + 0.5 - 200.0, f64::from(y) + 0.5 - 200.0);
        if dx * dx + dy * dy <= 200.0 * 200.0 {
            255
        } else {
            0
        }
    });
    let measured = measure(&disc, pitch);
    let (area, pull) = (PI * 100.0, disc_pull_n(20.0));
    assert!(
        (f64::from(measured.area_mm2) - area).abs() / area < 0.01,
        "area {} against pi R^2 = {area}",
        measured.area_mm2
    );
    assert!(
        (f64::from(measured.pull_n) - pull).abs() / pull < 0.02,
        "pull {} against a 20 mm disc's {pull}",
        measured.pull_n
    );
}

#[test]
fn a_strip_pulls_less_than_a_square_of_its_area() {
    let square = measure(&rectangle(40, 40), UNIT);
    let strip = measure(&rectangle(400, 4), UNIT);
    assert!((square.area_mm2 - strip.area_mm2).abs() < 1e-3);
    // Resin reaches 10 mm into the square and 2 into the strip.
    assert!(
        strip.pull_n < square.pull_n,
        "strip {} against square {}",
        strip.pull_n,
        square.pull_n
    );
}

#[test]
fn two_pieces_pull_less_than_one_of_their_joint_area() {
    let apart = paint(50, 22, |x, y| {
        u8::from((1..21).contains(&y) && ((1..21).contains(&x) || (29..49).contains(&x))) * 255
    });
    let two = measure(&apart, UNIT);
    let one = measure(&rectangle(20, 20), UNIT);
    assert_eq!(two.pieces, 2);
    assert!((two.pull_n - 2.0 * one.pull_n).abs() / one.pull_n < 1e-4);

    let joined = measure(&rectangle(40, 20), UNIT);
    assert_eq!(joined.pieces, 1);
    assert!(two.pull_n < joined.pull_n);
}

#[test]
fn a_run_across_the_end_of_a_row_is_two_pieces() {
    // The last pixel of the first row and the first of the second are one run in
    // memory and opposite edges of the panel.
    let layer = paint(4, 2, |x, y| {
        u8::from((x, y) == (3, 0) || (x, y) == (0, 1)) * 255
    });
    assert_eq!(layer.runs().iter().filter(|run| run.value > 0).count(), 1);
    assert_eq!(measure(&layer, UNIT).pieces, 2);
}

#[test]
fn neighbouring_greys_along_a_row_are_one_piece() {
    let layer = paint(6, 1, |x, _| [0, 255, 128, 255, 64, 0][x as usize]);
    assert_eq!(measure(&layer, UNIT).pieces, 1);
}

#[test]
fn pieces_touching_only_at_a_corner_are_apart() {
    let layer = paint(2, 2, |x, y| u8::from(x == y) * 255);
    assert_eq!(measure(&layer, UNIT).pieces, 2);
}

#[test]
fn a_half_grey_pixel_cures_half_its_area() {
    let pitch = PixelPitch { x: 0.02, y: 0.03 };
    let layer = paint(10, 10, |_, _| 51);
    let expected = 100.0 * 0.02 * 0.03 * 0.2;
    assert!((measure(&layer, pitch).area_mm2 - expected).abs() < 1e-6);
}

#[test]
fn a_piece_is_centred_on_its_pixels() {
    let cured = cure(&rectangle(4, 2), PixelPitch { x: 0.5, y: 1.0 });
    let [x, y] = cured.pieces()[0].centre_mm;
    // Columns 1..5 at half a millimetre, rows 1..3 at one.
    assert!((x - 1.5).abs() < 1e-6 && (y - 2.0).abs() < 1e-6, "{x}, {y}");
}

#[test]
fn erasing_darkens_the_stretches_and_keeps_every_other_grey() {
    let layer = paint(5, 2, |x, y| if y == 0 { 100 + x as u8 } else { 255 });
    let cuts = [
        Stretch {
            row: 0,
            x0: 1,
            x1: 3,
        },
        Stretch {
            row: 1,
            x0: 4,
            x1: 5,
        },
    ];
    let erased = erase(&layer, &cuts).to_mask().pixels().to_vec();
    assert_eq!(erased, vec![100, 0, 0, 103, 104, 255, 255, 255, 255, 0]);
}

#[test]
fn a_piece_taken_out_leaves_the_rest_renumbered() {
    let layer = paint(6, 1, |x, _| u8::from(x == 0 || x >= 3) * 255);
    let (kept, taken) = cure(&layer, UNIT).split(&[true, false]);
    assert_eq!(
        taken.stretches(),
        vec![Stretch {
            row: 0,
            x0: 0,
            x1: 1
        }]
    );
    assert_eq!(taken.pieces().len(), 1);
    assert_eq!(kept.pieces().len(), 1);
    assert!(kept.spans().iter().all(|span| span.piece == 0));
}

#[test]
fn a_disc_gyrates_about_its_centre_as_its_radius_over_root_two() {
    // A disc of radius R has I_p = pi R^4 / 2 over A = pi R^2, so R / sqrt(2).
    let pitch = PixelPitch { x: 0.05, y: 0.05 };
    let disc = paint(400, 400, |x, y| {
        let (dx, dy) = (f64::from(x) + 0.5 - 200.0, f64::from(y) + 0.5 - 200.0);
        u8::from(dx * dx + dy * dy <= 200.0 * 200.0) * 255
    });
    let gyration = cure(&disc, pitch).pieces()[0].gyration_mm;
    let expected = 10.0 / std::f32::consts::SQRT_2;
    assert!(
        (gyration - expected).abs() / expected < 0.01,
        "{gyration} against R / sqrt(2) = {expected}"
    );
}

#[test]
fn the_runs_of_a_piece_are_its_own_pixels() {
    let layer = paint(6, 2, |x, _| u8::from(x == 0 || x >= 3) * 200);
    let cured = cure(&layer, UNIT);
    let second = cured.runs_of(&[false, true]);
    let lit: Vec<bool> = second.to_mask().pixels().iter().map(|p| *p > 0).collect();
    let expected: Vec<bool> = (0..12).map(|i| i % 6 >= 3).collect();
    assert_eq!(lit, expected);
}
