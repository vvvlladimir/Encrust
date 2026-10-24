use super::*;
use core_geometry::Vec2;
use core_slicer::{Contour, Winding};

const LAYER_MM: Scalar = 0.5;

fn ring(x0: Scalar, y0: Scalar, x1: Scalar, y1: Scalar, winding: Winding) -> Contour {
    let corners = vec![
        Vec2::new(x0, y0),
        Vec2::new(x1, y0),
        Vec2::new(x1, y1),
        Vec2::new(x0, y1),
    ];
    let corners = match winding {
        Winding::Outer => corners,
        Winding::Inner => corners.into_iter().rev().collect(),
    };
    Contour::new(corners, winding)
}

/// A 20 mm square, solid or with a 10 mm hole through the middle of it.
fn layer(z: Scalar, hollow: bool) -> Layer {
    let mut contours = vec![ring(10.0, 10.0, 30.0, 30.0, Winding::Outer)];
    if hollow {
        contours.push(ring(15.0, 15.0, 25.0, 25.0, Winding::Inner));
    }
    Layer::new(z, contours)
}

/// Reads a stack of `hollow` flags, bottom first, onto a scan of the square above.
fn scan(layers: &[bool]) -> Vec<Trapped> {
    let mut scan = TrapScan::new(Vec2::splat(10.0), Vec2::splat(30.0), LAYER_MM);
    for (index, hollow) in layers.iter().enumerate() {
        scan.push(&layer(index as Scalar * LAYER_MM + LAYER_MM / 2.0, *hollow));
    }
    scan.finish()
}

#[test]
fn a_solid_stack_traps_nothing() {
    assert!(scan(&[false; 6]).is_empty());
}

#[test]
fn a_cavity_closed_at_both_ends_is_trapped() {
    // Four hollow layers between a floor and a lid: 10 x 10 x 2 mm of resin.
    let found = scan(&[false, true, true, true, true, false]);
    assert_eq!(found.len(), 1, "one pocket, got {found:?}");

    let trapped = found[0];
    assert!(
        (trapped.volume_mm3 - 200.0).abs() < 5.0,
        "10 x 10 mm over four 0.5 mm layers is 200 mm3, got {}",
        trapped.volume_mm3
    );
    assert!(
        (trapped.at.x - 20.0).abs() < 1.0 && (trapped.at.y - 20.0).abs() < 1.0,
        "the hole goes in the middle of the pocket, got {}",
        trapped.at
    );
    assert!(
        (trapped.at.z - 0.75).abs() < 1e-4,
        "and at its lowest layer, got {}",
        trapped.at.z
    );
}

#[test]
fn a_cavity_open_at_the_plate_drains_and_is_not_trapped() {
    assert!(
        scan(&[true, true, true, true, false]).is_empty(),
        "the floor is open, so the resin runs out onto the plate"
    );
}

#[test]
fn a_cavity_open_at_the_top_drains_and_is_not_trapped() {
    assert!(
        scan(&[false, true, true, true, true]).is_empty(),
        "the stack ends inside the cavity, so it is open to the air"
    );
}

#[test]
fn two_cavities_one_above_the_other_are_reported_separately() {
    let found = scan(&[false, true, true, false, true, true, false]);
    assert_eq!(found.len(), 2, "one pocket each side of the floor between");
    assert!(found[0].at.z < found[1].at.z || found[1].at.z < found[0].at.z);
}

/// Each pocket carries the space it fills, not its cavity: that box marks it on screen, so
/// pockets that stand apart may not overlap (ADR 0200).
#[test]
fn each_pocket_carries_the_box_it_fills() {
    let found = scan(&[false, true, true, false, true, true, false]);
    let (low, high) = if found[0].at.z < found[1].at.z {
        (found[0].bounds, found[1].bounds)
    } else {
        (found[1].bounds, found[0].bounds)
    };

    assert!(
        low.maxs.z <= high.mins.z,
        "the floor between them keeps the boxes apart, got {low:?} and {high:?}"
    );
    // The 10 mm hole in the middle of the square, over two 0.5 mm layers each.
    for pocket in [low, high] {
        assert!(
            pocket.mins.x >= 14.0 && pocket.maxs.x <= 26.0,
            "a 10 mm pocket at 15..25 mm is held within a cell of itself, got {pocket:?}"
        );
        assert!(
            (pocket.maxs.z - pocket.mins.z - 1.0).abs() < 0.3,
            "two 0.5 mm layers of air is a millimetre tall, got {pocket:?}"
        );
    }
}

#[test]
fn a_pocket_opening_on_the_layer_two_others_join_stays_its_own() {
    let square = || ring(10.0, 10.0, 30.0, 30.0, Winding::Outer);
    let left = || ring(12.0, 12.0, 14.0, 14.0, Winding::Inner);
    let right = || ring(16.0, 12.0, 18.0, 14.0, Winding::Inner);
    let joined = || ring(12.0, 12.0, 18.0, 14.0, Winding::Inner);
    let apart = || ring(22.0, 22.0, 28.0, 28.0, Winding::Inner);

    let mut scan = TrapScan::new(Vec2::splat(10.0), Vec2::splat(30.0), LAYER_MM);
    scan.push(&layer(0.25, false));
    scan.push(&Layer::new(0.75, vec![square(), left(), right()]));
    // The two pockets below meet, and one with nothing under it opens beside them.
    for z in [1.25, 1.75, 2.25] {
        scan.push(&Layer::new(z, vec![square(), joined(), apart()]));
    }
    scan.push(&layer(2.75, false));

    let found = scan.finish();
    assert_eq!(
        found.len(),
        2,
        "the joined pocket and the one apart, got {found:?}"
    );
    let apart = found
        .iter()
        .find(|trapped| trapped.at.x > 20.0)
        .expect("the pocket apart is reported where it is");
    assert!(
        (apart.volume_mm3 - 54.0).abs() < 2.0,
        "6 x 6 mm over three 0.5 mm layers is 54 mm3, got {}",
        apart.volume_mm3
    );
}

#[test]
fn a_pocket_of_a_few_cells_is_not_worth_a_hole() {
    let mut scan = TrapScan::new(Vec2::splat(10.0), Vec2::splat(30.0), LAYER_MM);
    let pinhole = Layer::new(
        0.75,
        vec![
            ring(10.0, 10.0, 30.0, 30.0, Winding::Outer),
            ring(19.8, 19.8, 20.2, 20.2, Winding::Inner),
        ],
    );
    scan.push(&layer(0.25, false));
    scan.push(&pinhole);
    scan.push(&layer(1.25, false));

    assert!(
        scan.finish().is_empty(),
        "a pocket smaller than a drop of resin is not reported"
    );
}
