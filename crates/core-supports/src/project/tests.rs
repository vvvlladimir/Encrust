use super::*;
use crate::fixtures::box_mesh;
use core_geometry::{Bvh, Transform};

/// The overhang angle the shipped profiles are around, so a ceiling is held and a
/// wall is not.
const OVERHANG_DEG: Scalar = 45.0;

/// A model standing where it was modelled, with nothing blocked.
fn standing<'a>(model: &'a Mesh, bvh: &'a Bvh) -> Placed<'a> {
    Placed::new(model, bvh, Transform::default())
}

/// A 10 mm square of two triangles lying flat at `z`, wound so it looks down: the
/// ceiling a patch fill is really for.
fn ceiling(z: Scalar) -> Mesh {
    Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, z),
            Vec3::new(10.0, 0.0, z),
            Vec3::new(10.0, 10.0, z),
            Vec3::new(0.0, 10.0, z),
        ],
        vec![[0, 2, 1], [0, 3, 2]],
    )
}

fn whole(model: &Mesh) -> Region {
    let mut region = Region::default();
    for face in 0..model.faces.len() {
        region.set(face, true);
    }
    region
}

fn filled(model: &Mesh, settings: &ProjectSettings) -> Vec<Vec3> {
    let bvh = Bvh::build(model);
    every(project(
        &standing(model, &bvh),
        &Adjacency::of(model),
        &whole(model),
        settings,
        &[],
        OVERHANG_DEG,
    ))
}

/// The rim's points and the inside's together.
fn every(projected: Projected) -> Vec<Vec3> {
    projected.rim.into_iter().chain(projected.inside).collect()
}

/// The fill as it was before offsets: the rim on the patch's edges, the grid up to them.
fn at_the_edge() -> ProjectSettings {
    ProjectSettings {
        border_offset_mm: 0.0,
        second_border_mm: None,
        infill_offset_mm: 0.0,
        pattern: InfillPattern::Grid,
        ..ProjectSettings::default()
    }
}

#[test]
fn nothing_painted_is_nothing_to_hold_up() {
    let model = ceiling(5.0);
    let bvh = Bvh::build(&model);
    let points = every(project(
        &standing(&model, &bvh),
        &Adjacency::of(&model),
        &Region::default(),
        &ProjectSettings::default(),
        &[],
        OVERHANG_DEG,
    ));
    assert!(points.is_empty());
}

#[test]
fn a_square_patch_is_filled_on_the_grid_it_was_asked_for() {
    let points = filled(
        &ceiling(5.0),
        &ProjectSettings {
            infill_spacing_mm: Some(2.0),
            border_spacing_mm: None,
            ..at_the_edge()
        },
    );

    // Cell centres at 1, 3, 5, 7 and 9 mm along each side of a 10 mm square.
    assert_eq!(points.len(), 25);
    assert!(
        points.iter().all(|point| (point.z - 5.0).abs() < 1e-4),
        "every point sits on the patch it fills"
    );
}

#[test]
fn a_patch_is_filled_where_the_model_stands_not_where_it_was_modelled() {
    let model = ceiling(5.0);
    let bvh = Bvh::build(&model);
    let moved = Placed::new(
        &model,
        &bvh,
        Transform::from_translation(Vec3::new(40.0, 0.0, 0.0)),
    );
    let points = every(project(
        &moved,
        &Adjacency::of(&model),
        &whole(&model),
        &ProjectSettings {
            infill_spacing_mm: Some(2.0),
            border_spacing_mm: None,
            ..at_the_edge()
        },
        &[],
        OVERHANG_DEG,
    ));

    assert_eq!(points.len(), 25);
    assert!(
        points.iter().all(|point| point.x >= 40.0),
        "the fill followed the model across the plate"
    );
}

#[test]
fn the_rim_of_a_patch_is_walked_at_its_own_spacing() {
    let points = filled(
        &ceiling(5.0),
        &ProjectSettings {
            infill_spacing_mm: None,
            border_spacing_mm: Some(2.0),
            ..at_the_edge()
        },
    );

    // Four 10 mm sides walked every 2 mm, the corners counted once each.
    assert_eq!(points.len(), 20);
    assert!(
        points
            .iter()
            .all(|point| point.x.min(point.y) <= 1e-4 || point.x.max(point.y) >= 10.0 - 1e-4),
        "every rim point is on an edge of the square"
    );
}

/// The same square meshed as a fan of many small triangles looking down: the rim is made of
/// edges far shorter than the spacing asked for.
fn fine_ceiling(z: Scalar, steps: usize) -> Mesh {
    let mut vertices = vec![Vec3::new(5.0, 5.0, z)];
    for step in 0..steps {
        let along = 40.0 * step as Scalar / steps as Scalar;
        let (x, y) = match along {
            a if a < 10.0 => (a, 0.0),
            a if a < 20.0 => (10.0, a - 10.0),
            a if a < 30.0 => (30.0 - a, 10.0),
            a => (0.0, 40.0 - a),
        };
        vertices.push(Vec3::new(x, y, z));
    }
    let faces = (0..steps)
        .map(|step| [0, 1 + ((step + 1) % steps) as u32, 1 + step as u32])
        .collect();
    Mesh::new(vertices, faces)
}

#[test]
fn a_finely_meshed_rim_is_walked_at_the_spacing_not_at_the_triangle() {
    let points = filled(
        &fine_ceiling(5.0, 200),
        &ProjectSettings {
            infill_spacing_mm: None,
            border_spacing_mm: Some(2.0),
            ..at_the_edge()
        },
    );

    // 40 mm of rim every 2 mm, give or take where the corners fall.
    assert!(
        (16..=24).contains(&points.len()),
        "a rim of 200 short edges became {} supports",
        points.len()
    );
}

#[test]
fn the_diagonal_the_square_is_meshed_along_is_not_a_rim() {
    let points = filled(
        &ceiling(5.0),
        &ProjectSettings {
            infill_spacing_mm: None,
            border_spacing_mm: Some(2.0),
            ..at_the_edge()
        },
    );
    assert!(
        !points
            .iter()
            .any(|point| (point.x - 5.0).abs() < 1e-4 && (point.y - 5.0).abs() < 1e-4),
        "the shared edge of the two triangles is inside the patch"
    );
}

#[test]
fn a_rim_point_keeps_the_grid_from_crowding_it() {
    let points = filled(
        &ceiling(5.0),
        &ProjectSettings {
            infill_spacing_mm: Some(2.0),
            border_spacing_mm: Some(2.0),
            ..at_the_edge()
        },
    );
    let close = points.iter().enumerate().any(|(at, point)| {
        points[at + 1..]
            .iter()
            .any(|other| point.distance(*other) < 1.0)
    });
    assert!(!close, "no two supports were put within a millimetre");
}

#[test]
fn a_blocked_face_is_left_out_of_the_fill() {
    let model = ceiling(5.0);
    let bvh = Bvh::build(&model);
    let mut blocked_region = Region::default();
    blocked_region.set(0, true);
    let blocked = crate::Blocked::new(&model, &blocked_region, Transform::default())
        .expect("half the square was blocked");
    let placed = standing(&model, &bvh).blocking(Some(&blocked));

    let points = every(project(
        &placed,
        &Adjacency::of(&model),
        &whole(&model),
        &ProjectSettings {
            infill_spacing_mm: Some(2.0),
            border_spacing_mm: None,
            ..at_the_edge()
        },
        &[],
        OVERHANG_DEG,
    ));

    assert!(
        points.iter().all(|point| point.y >= point.x - 1e-4),
        "the blocked half of the square was filled anyway"
    );
    assert!(!points.is_empty(), "the half that is free is still filled");
}

#[test]
fn a_fill_puts_nothing_on_top_of_a_support_already_standing() {
    let model = ceiling(5.0);
    let bvh = Bvh::build(&model);
    let settings = ProjectSettings {
        infill_spacing_mm: Some(2.0),
        border_spacing_mm: None,
        ..at_the_edge()
    };
    let once = every(project(
        &standing(&model, &bvh),
        &Adjacency::of(&model),
        &whole(&model),
        &settings,
        &[],
        OVERHANG_DEG,
    ));
    let twice = every(project(
        &standing(&model, &bvh),
        &Adjacency::of(&model),
        &whole(&model),
        &settings,
        &once,
        OVERHANG_DEG,
    ));

    assert!(
        twice.is_empty(),
        "filling the same patch again added {} more supports",
        twice.len()
    );
}

/// A 10 mm square standing on its edge, wound so it looks along x: a wall holds
/// itself up whatever is painted on it.
fn wall() -> Mesh {
    Mesh::new(
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, 10.0, 10.0),
            Vec3::new(0.0, 0.0, 10.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

#[test]
fn a_painted_wall_is_not_filled() {
    let points = filled(
        &wall(),
        &ProjectSettings {
            infill_spacing_mm: Some(2.0),
            border_spacing_mm: Some(2.0),
            ..at_the_edge()
        },
    );
    assert!(
        points.is_empty(),
        "a wall leaning nothing at all took {} supports at {OVERHANG_DEG} degrees",
        points.len()
    );
}

#[test]
fn a_wall_is_filled_once_the_angle_asks_for_it() {
    let model = wall();
    let bvh = Bvh::build(&model);
    let points = every(project(
        &standing(&model, &bvh),
        &Adjacency::of(&model),
        &whole(&model),
        &ProjectSettings {
            infill_spacing_mm: None,
            border_spacing_mm: Some(2.0),
            ..at_the_edge()
        },
        &[],
        0.0,
    ));
    assert!(
        !points.is_empty(),
        "an angle of zero holds up every face that is painted"
    );
}

#[test]
fn a_patch_that_folds_over_itself_is_held_from_underneath() {
    let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let points = filled(
        &model,
        &ProjectSettings {
            infill_spacing_mm: Some(4.0),
            border_spacing_mm: None,
            ..at_the_edge()
        },
    );
    assert!(
        points.iter().all(|point| point.z < 1e-4),
        "a cube painted all over is filled on its underside, not on its lid"
    );
}
/// How far a point lies inside the 10 mm square of [`ceiling`], seen from above.
fn inside_square(point: Vec3) -> Scalar {
    point.x.min(10.0 - point.x).min(point.y).min(10.0 - point.y)
}

fn projected(settings: &ProjectSettings) -> Projected {
    let model = ceiling(5.0);
    let bvh = Bvh::build(&model);
    project(
        &standing(&model, &bvh),
        &Adjacency::of(&model),
        &whole(&model),
        settings,
        &[],
        OVERHANG_DEG,
    )
}

/// Half a footprint cell for a 2 mm spacing, the most an offset is rounded by.
const CELL_SLACK_MM: Scalar = 0.26;

#[test]
fn a_rim_offset_stands_the_rim_that_far_inside_the_edge() {
    let rim = projected(&ProjectSettings {
        infill_spacing_mm: None,
        border_spacing_mm: Some(2.0),
        border_offset_mm: 1.0,
        ..at_the_edge()
    })
    .rim;

    // A square 8 mm a side, 32 mm round, walked every 2 mm.
    assert!(
        (12..=20).contains(&rim.len()),
        "{} points on a 32 mm ring",
        rim.len()
    );
    for point in &rim {
        assert!(
            (inside_square(*point) - 1.0).abs() <= CELL_SLACK_MM,
            "{point} is not 1 mm inside the square's edge"
        );
        assert!((point.z - 5.0).abs() < 1e-4, "{point} is off the patch");
    }
}

#[test]
fn a_second_rim_stands_its_gap_inside_the_first() {
    let rim = projected(&ProjectSettings {
        infill_spacing_mm: None,
        border_spacing_mm: Some(2.0),
        border_offset_mm: 1.0,
        second_border_mm: Some(2.0),
        ..at_the_edge()
    })
    .rim;

    let near = |depth: Scalar| {
        rim.iter()
            .filter(|point| (inside_square(**point) - depth).abs() <= CELL_SLACK_MM)
            .count()
    };
    assert!(near(1.0) > 0, "no point on the first rim, 1 mm in");
    assert!(near(3.0) > 0, "no point on the second rim, 1 + 2 mm in");
    assert_eq!(
        near(1.0) + near(3.0),
        rim.len(),
        "a rim point stands on neither ring"
    );
}

#[test]
fn the_inside_keeps_its_offset_from_the_innermost_rim() {
    let inside = projected(&ProjectSettings {
        infill_spacing_mm: Some(2.0),
        border_spacing_mm: Some(2.0),
        border_offset_mm: 1.0,
        infill_offset_mm: 1.5,
        ..at_the_edge()
    })
    .inside;

    assert!(!inside.is_empty());
    for point in &inside {
        assert!(
            inside_square(*point) >= 2.5 - CELL_SLACK_MM,
            "{point} is nearer the edge than the rim at 1 mm and the offset of 1.5 mm"
        );
    }
}

#[test]
fn a_concentric_inside_lies_on_rings_the_spacing_apart() {
    let inside = projected(&ProjectSettings {
        infill_spacing_mm: Some(2.0),
        border_spacing_mm: None,
        infill_offset_mm: 1.0,
        pattern: InfillPattern::Concentric,
        ..at_the_edge()
    })
    .inside;

    // Rings 1, 3 and 5 mm in; the last is the square's middle.
    assert!(inside.len() > 8);
    for point in &inside {
        let depth = inside_square(*point);
        let off_ring = ((depth - 1.0) / 2.0 - ((depth - 1.0) / 2.0).round()).abs() * 2.0;
        assert!(
            off_ring <= CELL_SLACK_MM * 2.0,
            "{point} lies {depth} mm in, between the rings at 1, 3 and 5 mm"
        );
    }
}

#[test]
fn a_random_inside_scatters_the_same_way_every_time_and_never_crowds() {
    let settings = ProjectSettings {
        infill_spacing_mm: Some(2.0),
        border_spacing_mm: None,
        infill_offset_mm: 1.0,
        pattern: InfillPattern::Random,
        ..at_the_edge()
    };
    let inside = projected(&settings).inside;

    assert_eq!(
        inside,
        projected(&settings).inside,
        "a scatter is repeatable"
    );
    assert!(!inside.is_empty());
    for (at, point) in inside.iter().enumerate() {
        assert!(inside_square(*point) >= 1.0 - CELL_SLACK_MM);
        assert!(
            inside[at + 1..]
                .iter()
                .all(|other| other.distance(*point) >= 2.0 * CROWD),
            "two scattered points within three quarters of the spacing"
        );
    }
}

#[test]
fn the_rim_and_the_inside_come_back_apart() {
    let projected = projected(&ProjectSettings::default());
    assert!(!projected.rim.is_empty());
    assert!(!projected.inside.is_empty());
    assert!(
        projected
            .rim
            .iter()
            .all(|point| (inside_square(*point) - 1.0).abs() <= CELL_SLACK_MM),
        "the default rim stands 1 mm in"
    );
}
