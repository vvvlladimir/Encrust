use super::*;
use crate::config::{GRID_PITCH_MM, REFERENCE_RISE_MM, SampleConfig};
use core_geometry::{Scalar, Vec2};
use core_slicer::{Contour, Layer};
use printer_profiles::SupportProfile;

fn grid() -> Grid {
    Grid::covering(Vec2::ZERO, Vec2::splat(60.0), GRID_PITCH_MM)
}

fn config() -> SampleConfig {
    SampleConfig::for_profile(&crate::fixtures::profile())
}

/// One axis-aligned rectangle as a layer, read onto the grid.
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

fn square(x: Scalar, y: Scalar, side: Scalar) -> Field {
    rectangle(x, y, side, side)
}

/// The default rise, so that the reach is the profile's angle at face value.
fn found(layer: &Field, below: &Field, reference: &Field) -> Unsupported {
    unsupported(
        layer,
        below,
        reference,
        &grid(),
        &config(),
        REFERENCE_RISE_MM,
        None,
    )
}

#[test]
fn a_wall_standing_on_itself_needs_nothing() {
    let wall = square(10.0, 10.0, 10.0);
    assert!(found(&wall, &wall, &wall).is_empty());
}

/// A wall leaning out `step` mm over the reference rise, as the three fields placement reads;
/// a lean moves under a cell per layer, hence the rise (ADR 0031).
fn leaning(step: Scalar) -> (Field, Field, Field) {
    (
        square(10.0, 10.0, 10.0 + step),
        square(10.0, 10.0, 10.0 + step - GRID_PITCH_MM),
        square(10.0, 10.0, 10.0),
    )
}

#[test]
fn a_surface_leaning_less_than_the_angle_holds_itself_up() {
    // A millimetre of rise moves the wall 0.9 mm, inside the millimetre a 45 degree
    // lean is allowed.
    let (layer, below, reference) = leaning(0.9);
    assert!(found(&layer, &below, &reference).overhangs.is_empty());
}

#[test]
fn a_step_too_wide_to_bridge_is_an_overhang_however_gentle_the_lean() {
    // The same 0.9 mm, taken in one layer: over a millimetre of rise that is a lean
    // the profile allows, but in one layer it is a shelf over nothing.
    let below = square(10.0, 10.0, 10.0);
    let layer = square(10.0, 10.0, 10.9);
    assert!(!found(&layer, &below, &below).overhangs.is_empty());
}

#[test]
fn a_surface_leaning_further_than_the_angle_is_an_overhang() {
    let below = square(10.0, 10.0, 10.0);
    let layer = square(10.0, 10.0, 14.0);
    let overhangs = found(&layer, &below, &below).overhangs;

    assert_eq!(overhangs.len(), 1, "the ledge is one piece");
    let held = overhangs[0].area_mm2(&grid());
    // Everything past the millimetre of reach: a 14 mm square less an 11 mm one.
    let expected = 14.0 * 14.0 - 11.0 * 11.0;
    assert!(
        (held - expected).abs() / expected < 0.05,
        "expected about {expected} mm2 of ledge, got {held}"
    );
}

#[test]
fn a_part_with_nothing_under_it_is_an_island() {
    let below = square(10.0, 10.0, 10.0);
    let layer = square(10.0, 10.0, 10.0).with(&square(40.0, 40.0, 5.0));

    let found = found(&layer, &below, &below);
    assert_eq!(found.islands.len(), 1, "one piece stands on nothing");
    let island = found.islands[0].area_mm2(&grid());
    assert!(
        (island - 25.0).abs() < 1.0,
        "the whole 5 mm island, got {island}"
    );
}

#[test]
fn the_first_layer_of_a_stack_is_all_island() {
    let layer = square(10.0, 10.0, 10.0);
    let found = found(&layer, &Field::default(), &Field::default());
    assert_eq!(found.islands.len(), 1);
}

#[test]
fn a_speck_too_small_to_print_is_not_an_island_worth_holding() {
    let below = square(10.0, 10.0, 10.0);
    let speck = square(40.0, 40.0, 0.1);
    let layer = below.with(&speck);
    assert!(found(&layer, &below, &below).islands.is_empty());
}

#[test]
fn a_ceiling_closing_over_a_bore_is_an_overhang() {
    let mut bore = square(12.0, 12.0, 6.0);
    // A ring: the 20 mm block with a 6 mm hole through it.
    bore = square(10.0, 10.0, 20.0).without(&bore);
    let layer = square(10.0, 10.0, 20.0);

    let overhangs = found(&layer, &bore, &bore).overhangs;
    assert_eq!(overhangs.len(), 1, "the roof over the bore");
    let roof = overhangs[0].area_mm2(&grid());
    // The whole 6 mm hole less the rim the wall around it carries, which is the step
    // the resin bridges rather than the millimetre of reach.
    assert!(
        (16.0..36.0).contains(&roof),
        "expected most of a 36 mm2 hole, got {roof}"
    );
}

#[test]
fn a_shallower_angle_finds_more_to_hold() {
    let (layer, below, reference) = leaning(0.9);
    let strict = SampleConfig::for_profile(&SupportProfile {
        max_overhang_deg: 20.0,
        ..crate::fixtures::profile()
    });

    assert!(
        found(&layer, &below, &reference).overhangs.is_empty(),
        "45 degrees lets a 0.9 mm lean over a millimetre of rise pass"
    );
    assert!(
        !unsupported(
            &layer,
            &below,
            &reference,
            &grid(),
            &strict,
            REFERENCE_RISE_MM,
            None
        )
        .overhangs
        .is_empty(),
        "20 degrees does not"
    );
}

/// What a layer has unsupported with steps asked for apart, a step's faces leaning 20
/// degrees and a steadied slope 10.
fn found_with_steps(layer: &Field, below: &Field, reference: &Field) -> Unsupported {
    unsupported(
        layer,
        below,
        reference,
        &grid(),
        &config(),
        REFERENCE_RISE_MM,
        Some([20.0, 10.0]),
    )
}

#[test]
fn a_shelf_put_out_in_one_layer_is_a_step_and_not_an_overhang() {
    let below = square(10.0, 10.0, 10.0);
    let layer = rectangle(10.0, 10.0, 10.0, 12.0);
    let split = found_with_steps(&layer, &below, &below);
    assert_eq!(split.steps.len(), 1, "the 2 mm shelf is one step");
    assert!(split.overhangs.is_empty(), "and nothing else leans");
    assert!(
        found_with_steps(&layer, &below, &below).gentle.is_empty(),
        "a shelf is not a gentle face"
    );
    assert_eq!(
        found(&layer, &below, &below).overhangs.len(),
        1,
        "without steps asked for, the shelf is an overhang as it always was"
    );
}

#[test]
fn a_face_between_the_two_angles_is_gentle_and_nothing_else() {
    // Half a millimetre out over a millimetre of rise is 27 degrees: past 20, short of
    // the fixture's 45, and a hair a layer, never a shelf.
    let reference = square(10.0, 10.0, 10.0);
    let below = rectangle(10.0, 10.0, 10.0, 10.45);
    let layer = rectangle(10.0, 10.0, 10.0, 10.5);
    let split = found_with_steps(&layer, &below, &reference);
    assert!(split.overhangs.is_empty() && split.steps.is_empty());
    assert_eq!(split.gentle.len(), 1, "the lean is one gentle piece");
    assert!(split.slopes.is_empty(), "and no slope besides");
}

#[test]
fn a_face_between_the_steadying_angle_and_a_steps_is_a_slope() {
    // 0.35 mm over a millimetre of rise is 19 degrees: past 10, short of 20.
    let reference = square(10.0, 10.0, 10.0);
    let below = rectangle(10.0, 10.0, 10.0, 10.25);
    let layer = rectangle(10.0, 10.0, 10.0, 10.35);
    let split = found_with_steps(&layer, &below, &reference);
    assert!(split.gentle.is_empty() && split.overhangs.is_empty());
    assert_eq!(split.slopes.len(), 1, "the lean is one slope");
}

#[test]
fn a_shorter_rise_scales_the_reach_it_allows() {
    // Half the rise, so half the ground a lean may cover.
    let below = square(10.0, 10.0, 10.0);
    let layer = square(10.0, 10.0, 10.7);
    assert!(
        found(&layer, &below, &below).overhangs.is_empty(),
        "0.7 mm over a millimetre of rise is under 45 degrees"
    );
    assert!(
        !unsupported(
            &layer,
            &below,
            &below,
            &grid(),
            &config(),
            REFERENCE_RISE_MM / 2.0,
            None
        )
        .overhangs
        .is_empty(),
        "the same step over half the rise is over it"
    );
}

#[test]
fn a_layer_of_nothing_needs_nothing() {
    let below = square(10.0, 10.0, 10.0);
    assert!(found(&Field::default(), &below, &below).is_empty());
}
