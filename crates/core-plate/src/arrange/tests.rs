use super::*;
use core_geometry::{Mesh, Transform, Vec3};

/// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
fn cuboid(size: Vec3) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, size)
}

/// `count` identical boxes, all stacked at the origin, which is the worst plate the
/// arranger can be handed.
fn heap(count: usize, size: Vec3, cell_mm: Scalar) -> Vec<Footprint> {
    let mesh = cuboid(size);
    (0..count)
        .map(|_| {
            Footprint::of(&mesh, Transform::default(), cell_mm).expect("a box has a footprint")
        })
        .collect()
}

/// Where a footprint ends up, as the rectangle it then covers in millimetres.
fn rect(footprint: &Footprint, offset: Vec2) -> (Vec2, Vec2) {
    let min = footprint.corner_mm + offset;
    let size = Vec2::new(
        footprint.mask.width() as Scalar * footprint.cell_mm,
        footprint.mask.height() as Scalar * footprint.cell_mm,
    );
    (min, min + size)
}

fn overlap(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> bool {
    a.0.x < b.1.x && b.0.x < a.1.x && a.0.y < b.1.y && b.0.y < a.1.y
}

#[test]
fn six_copies_are_spread_over_the_plate_without_touching() {
    let footprints = heap(6, Vec3::new(20.0, 20.0, 10.0), 1.0);
    let arranged = arrange(
        &footprints,
        Vec2::new(150.0, 80.0),
        &ArrangeSettings::default(),
    );

    assert_eq!(
        arranged.placed.len(),
        6,
        "six 20 mm boxes fit a 150 by 80 plate"
    );
    assert!(arranged.left_out.is_empty());

    for (first, placed) in arranged.placed.iter().enumerate() {
        let a = rect(&footprints[placed.index], placed.offset_mm);
        for other in &arranged.placed[first + 1..] {
            let b = rect(&footprints[other.index], other.offset_mm);
            assert!(!overlap(a, b), "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn everything_lands_inside_the_plate_and_its_margin() {
    let footprints = heap(4, Vec3::new(30.0, 25.0, 10.0), 1.0);
    let settings = ArrangeSettings::default();
    let arranged = arrange(&footprints, Vec2::new(150.0, 80.0), &settings);

    for placed in &arranged.placed {
        let (min, max) = rect(&footprints[placed.index], placed.offset_mm);
        assert!(
            min.x >= settings.margin_mm - 1e-3 && min.y >= settings.margin_mm - 1e-3,
            "{min} is inside the margin"
        );
        assert!(
            max.x <= 150.0 - settings.margin_mm + 1.0 && max.y <= 80.0 - settings.margin_mm + 1.0,
            "{max} is inside the plate"
        );
    }
}

#[test]
fn the_clearance_is_kept_between_neighbours() {
    let settings = ArrangeSettings {
        clearance_mm: 5.0,
        ..ArrangeSettings::default()
    };
    let footprints = heap(2, Vec3::new(20.0, 20.0, 10.0), 1.0);
    let arranged = arrange(&footprints, Vec2::new(150.0, 80.0), &settings);
    assert_eq!(arranged.placed.len(), 2);

    let a = rect(&footprints[0], arranged.placed[0].offset_mm);
    let b = rect(&footprints[1], arranged.placed[1].offset_mm);
    let gap_x = (b.0.x - a.1.x).max(a.0.x - b.1.x);
    let gap_y = (b.0.y - a.1.y).max(a.0.y - b.1.y);
    assert!(
        gap_x.max(gap_y) >= settings.clearance_mm - 1e-3,
        "expected {} mm between them, got {}",
        settings.clearance_mm,
        gap_x.max(gap_y)
    );
}

#[test]
fn what_does_not_fit_is_named_rather_than_stacked() {
    let footprints = heap(3, Vec3::new(60.0, 60.0, 10.0), 1.0);
    let arranged = arrange(
        &footprints,
        Vec2::new(150.0, 80.0),
        &ArrangeSettings::default(),
    );

    assert_eq!(
        arranged.placed.len(),
        2,
        "two 60 mm boxes fit across 150 mm"
    );
    assert_eq!(arranged.left_out.len(), 1);
}

#[test]
fn a_model_wider_than_the_plate_is_left_where_it_is() {
    let footprints = heap(1, Vec3::new(200.0, 20.0, 10.0), 1.0);
    let arranged = arrange(
        &footprints,
        Vec2::new(150.0, 80.0),
        &ArrangeSettings::default(),
    );
    assert!(arranged.placed.is_empty());
    assert_eq!(arranged.left_out, vec![0]);
}

#[test]
fn a_plate_smaller_than_its_own_margin_takes_nothing() {
    let footprints = heap(1, Vec3::splat(2.0), 1.0);
    let arranged = arrange(
        &footprints,
        Vec2::new(3.0, 3.0),
        &ArrangeSettings::default(),
    );
    assert_eq!(arranged.left_out, vec![0]);
}

#[test]
fn a_single_model_lands_in_the_middle() {
    let footprints = heap(1, Vec3::new(20.0, 20.0, 10.0), 1.0);
    let arranged = arrange(
        &footprints,
        Vec2::new(150.0, 80.0),
        &ArrangeSettings::default(),
    );

    let (min, max) = rect(&footprints[0], arranged.placed[0].offset_mm);
    let centre = (min + max) / 2.0;
    assert!(
        (centre - Vec2::new(75.0, 40.0)).length() < 2.0,
        "expected the middle of the plate, got {centre}"
    );
}
