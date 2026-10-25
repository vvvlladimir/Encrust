use super::*;
use crate::config::GRID_PITCH_MM;
use crate::field::Grid;
use core_geometry::Scalar;
use core_slicer::{Contour, Layer};

fn grid() -> Grid {
    Grid::covering(Vec2::ZERO, Vec2::splat(60.0), GRID_PITCH_MM)
}

fn rectangle(x: Scalar, y: Scalar, width: Scalar, height: Scalar) -> Field {
    let points = vec![
        Vec2::new(x, y),
        Vec2::new(x + width, y),
        Vec2::new(x + width, y + height),
        Vec2::new(x, y + height),
    ];
    let layer = Layer {
        z: 1.0,
        contours: vec![Contour::from_points(points).expect("a rectangle encloses area")],
        extra: Vec::new(),
    };
    Field::of_layer(&layer, &grid())
}

fn circle(centre: Vec2, radius: Scalar) -> Field {
    let points = (0..96)
        .map(|side| {
            let angle = std::f32::consts::TAU * side as Scalar / 96.0;
            centre + Vec2::new(angle.cos(), angle.sin()) * radius
        })
        .collect();
    let layer = Layer {
        z: 1.0,
        contours: vec![Contour::from_points(points).expect("a circle encloses area")],
        extra: Vec::new(),
    };
    Field::of_layer(&layer, &grid())
}

/// Fifty cells of a tenth of a millimetre: the spacing the medium preset asks for.
const SPACING: i32 = 50;

/// Three cells: a head 0.4 mm across and the cell it keeps clear of the outline.
const INSET: i32 = 3;

/// `piece` sampled as a whole layer of its own, so its outline is the layer's.
fn sampled(piece: &Field, spacing: i32) -> Vec<[i32; 2]> {
    sample_piece(piece, &piece.shrunk(INSET), INSET, spacing)
}

#[test]
fn a_wide_piece_is_covered_at_the_spacing_it_was_given() {
    let piece = rectangle(10.0, 10.0, 20.0, 20.0);
    let points = sampled(&piece, SPACING);

    assert!(points.len() > 8, "a 400 mm2 piece needs a lattice");
    for point in &points {
        assert!(
            piece.contains(point[0], point[1]),
            "{point:?} fell outside the piece"
        );
    }

    // Nowhere inside is further from a support than the lattice allows.
    let (min, max) = piece.bounds().expect("the piece has cells");
    for y in (min[1]..=max[1]).step_by(7) {
        for x in (min[0]..=max[0]).step_by(7) {
            if !piece.contains(x, y) {
                continue;
            }
            let nearest = points
                .iter()
                .map(|point| {
                    let dx = (point[0] - x) as f32;
                    let dy = (point[1] - y) as f32;
                    dx.hypot(dy)
                })
                .fold(f32::INFINITY, f32::min);
            // The lattice covers the inside and the walk the rim, each within its own spacing;
            // where they meet a suppressed rim point can leave half a step more.
            assert!(
                nearest <= 1.5 * SPACING as f32,
                "cell {x},{y} is {nearest} cells from the nearest support"
            );
        }
    }
}

#[test]
fn a_sliver_too_thin_for_the_lattice_still_gets_a_line_of_supports() {
    let piece = rectangle(10.0, 10.02, 30.0, 0.2);
    let points = sampled(&piece, SPACING);

    assert!(points.len() >= 5, "a 30 mm sliver needs a row of supports");
    for point in &points {
        assert!(piece.contains(point[0], point[1]), "{point:?} fell outside");
    }
    for (index, point) in points.iter().enumerate() {
        for other in &points[index + 1..] {
            let dx = (point[0] - other[0]) as f32;
            let dy = (point[1] - other[1]) as f32;
            assert!(
                dx.hypot(dy) >= SPACING as f32 - 1.0,
                "two supports landed on top of each other"
            );
        }
    }
}

#[test]
fn every_corner_of_a_ledge_gets_a_support_wherever_the_lattice_falls() {
    // Off the lattice on every side, so no corner has a lattice point to lean on.
    let piece = rectangle(1.3, 2.7, 17.0, 13.0);
    let points = sampled(&piece, SPACING);
    let (min, max) = piece.bounds().expect("the piece has cells");

    for corner in [
        [min[0] + INSET, min[1] + INSET],
        [max[0] - INSET, min[1] + INSET],
        [max[0] - INSET, max[1] - INSET],
        [min[0] + INSET, max[1] - INSET],
    ] {
        assert!(
            points.contains(&corner),
            "a corner is the first place a ledge lifts, and {corner:?} has no support"
        );
    }
}

#[test]
fn no_support_of_a_wide_piece_stands_nearer_its_outline_than_a_head_reaches() {
    let piece = rectangle(1.3, 2.7, 17.0, 13.0);
    let (min, max) = piece.bounds().expect("the piece has cells");

    for point in sampled(&piece, SPACING) {
        let room = (point[0] - min[0])
            .min(max[0] - point[0])
            .min(point[1] - min[1])
            .min(max[1] - point[1]);
        assert!(
            room >= INSET,
            "{point:?} is {room} cells from the outline and the head reaches {INSET}"
        );
    }
}

#[test]
fn an_arm_too_narrow_for_a_head_is_still_held_along_its_length() {
    // A 10 mm square with a 0.3 mm arm running 20 mm out of its side.
    let piece = rectangle(10.0, 10.0, 10.0, 10.0).with(&rectangle(20.0, 15.0, 20.0, 0.3));
    let points = sampled(&piece, SPACING);

    let far = points.iter().filter(|point| point[0] > 300).count();
    assert!(
        far >= 1,
        "the arm ends 20 mm from the square and nothing stands in its last 10 mm"
    );
}

#[test]
fn a_ledge_is_inset_from_the_outline_and_not_from_what_carries_it() {
    // The right half of a layer hangs over nothing; its left side joins the half
    // that is carried, which is no edge of the layer.
    let layer = rectangle(10.0, 10.0, 20.0, 10.0);
    let piece = rectangle(20.0, 10.0, 10.0, 10.0);
    let (min, _) = piece.bounds().expect("the piece has cells");

    let points = sample_piece(&piece, &layer.shrunk(INSET), INSET, SPACING);
    assert!(
        points.iter().any(|point| point[0] == min[0]),
        "the side a ledge is carried along has material on both sides of it"
    );
}

#[test]
fn a_round_piece_has_no_corners_to_crowd_its_rim() {
    let rim = circle(Vec2::new(30.0, 30.0), 15.0).rim();
    assert!(
        corners(&rim, SPACING).is_empty(),
        "a 30 mm circle turns 4 degrees over a millimetre of rim"
    );
}

#[test]
fn a_part_held_at_one_place_is_steadied_at_three() {
    let points = sample_stance(&circle(Vec2::new(30.0, 30.0), 3.0), INSET);
    assert_eq!(
        points.len(),
        3,
        "three points 0.8 of a circle apart fit round it and a fourth does not"
    );
}

#[test]
fn a_speck_gets_exactly_one_support() {
    let piece = rectangle(10.0, 10.0, 0.4, 0.4);
    let points = sampled(&piece, SPACING);
    assert_eq!(points.len(), 1);
    assert!(piece.contains(points[0][0], points[0][1]));
}

#[test]
fn a_sliver_held_at_one_place_is_held_at_its_middle() {
    // 0.8 by 0.3 mm: one support's worth, which an end would leave as a lever.
    let piece = rectangle(10.0, 10.0, 0.8, 0.3);
    let points = sampled(&piece, SPACING);
    assert_eq!(points.len(), 1);
    let [x, y] = points[0];
    assert!(
        (x - 104).abs() <= 1 && (y - 101).abs() <= 1,
        "the middle of cells 100..108 by 100..103 is (104, 101), got ({x}, {y})"
    );
}

#[test]
fn the_lattice_does_not_move_when_the_piece_does() {
    // Two steps, because every other row of the lattice is offset half a step: a
    // piece moved by an odd number of steps meets the other half of it.
    let here = sampled(&rectangle(10.0, 10.0, 20.0, 20.0), SPACING);
    let there = sampled(&rectangle(20.0, 20.0, 20.0, 20.0), SPACING);
    assert_eq!(
        here.len(),
        there.len(),
        "the lattice is anchored to the plate, so a whole number of steps changes nothing"
    );
}

#[test]
fn a_denser_spacing_puts_down_more_supports() {
    let piece = rectangle(10.0, 10.0, 20.0, 20.0);
    let sparse = sampled(&piece, SPACING);
    let dense = sampled(&piece, SPACING / 2);
    assert!(dense.len() > sparse.len());
}

#[test]
fn an_empty_piece_is_sampled_as_nothing() {
    assert!(sampled(&Field::default(), SPACING).is_empty());
}
