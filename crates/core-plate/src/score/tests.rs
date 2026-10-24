use super::*;

/// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
fn cuboid(size: Vec3) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, size)
}

#[test]
fn a_cube_standing_on_a_face_has_no_overhang_and_a_whole_face_down() {
    let cube = cuboid(Vec3::splat(10.0));
    let faces = Faces::of(&cube).expect("a cube has faces");
    let score = faces.score(Vec3::NEG_Z);

    assert!(score.overhang_mm2 < 1e-3, "every wall is vertical");
    assert!(
        (score.bottom_mm2 - 100.0).abs() < 1e-3,
        "the whole 10x10 face rests on the plate, got {}",
        score.bottom_mm2
    );
    assert!((score.height_mm - 10.0).abs() < 1e-3);
}

#[test]
fn a_cube_on_its_corner_rests_on_nothing_and_scores_worse() {
    let cube = cuboid(Vec3::splat(10.0));
    let faces = Faces::of(&cube).expect("a cube has faces");
    let down = Vec3::splat(-1.0).normalize();
    let score = faces.score(down);

    assert!(score.bottom_mm2 < 1e-3, "no face is flat against the plate");
    assert!(
        score.total > faces.score(Vec3::NEG_Z).total,
        "standing on a corner is worse than standing on a face"
    );
    // The three lower faces tilt 35 degrees from vertical, which resin bridges on its
    // own; at 20 degrees they stop being self-supporting.
    assert!(score.overhang_mm2 < 1e-3);
    let steep = Faces::of(&cube)
        .expect("a cube has faces")
        .for_settings(&OrientSettings {
            max_overhang_deg: 20.0,
            ..OrientSettings::default()
        });
    assert!(steep.score(down).overhang_mm2 > 0.0);
}

#[test]
fn a_ceiling_above_the_plate_is_an_overhang_and_the_footprint_is_not() {
    // A stem with a wider cap on top: the cap's underside is what needs holding up.
    let mut mushroom = cuboid(Vec3::new(2.0, 2.0, 10.0));
    let cap = cuboid(Vec3::new(10.0, 10.0, 2.0));
    let offset = mushroom.vertices.len() as u32;
    mushroom.vertices.extend(
        cap.vertices
            .iter()
            .map(|v| *v + Vec3::new(-4.0, -4.0, 10.0)),
    );
    mushroom.faces.extend(
        cap.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );

    let faces = Faces::of(&mushroom).expect("the mushroom has faces");
    let score = faces.score(Vec3::NEG_Z);
    assert!(
        (score.bottom_mm2 - 4.0).abs() < 1e-3,
        "only the 2 by 2 stem rests on the plate, got {}",
        score.bottom_mm2
    );
    // The cap's whole underside is horizontal: 100 mm2 weighted by 1 - sin(45).
    let expected = 100.0 * (1.0 - std::f32::consts::FRAC_PI_4.sin());
    assert!(
        (score.overhang_mm2 - expected).abs() < 1e-2,
        "expected {expected}, got {}",
        score.overhang_mm2
    );
}

fn on_supports() -> OrientSettings {
    OrientSettings {
        on_supports: true,
        ..OrientSettings::default()
    }
}

#[test]
fn on_supports_the_underside_hangs_and_earns_no_footprint() {
    let cube = cuboid(Vec3::splat(10.0));
    let lifted = Faces::of(&cube)
        .expect("a cube has faces")
        .for_settings(&on_supports())
        .score(Vec3::NEG_Z);
    // The whole 10 by 10 underside is horizontal: 100 mm2 weighted by 1 - sin(45).
    let expected = 100.0 * (1.0 - std::f32::consts::FRAC_PI_4.sin());
    assert!(
        (lifted.overhang_mm2 - expected).abs() < 1e-2,
        "expected {expected}, got {}",
        lifted.overhang_mm2
    );
    assert!(lifted.bottom_mm2 < 1e-3, "nothing rests on the plate");
}

/// A square pyramid with its apex at the origin and its 20 by 20 base 20 mm above.
fn pyramid() -> Mesh {
    Mesh::new(
        vec![
            Vec3::ZERO,
            Vec3::new(-10.0, -10.0, 20.0),
            Vec3::new(10.0, -10.0, 20.0),
            Vec3::new(10.0, 10.0, 20.0),
            Vec3::new(-10.0, 10.0, 20.0),
        ],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [0, 4, 3],
            [0, 1, 4],
            [1, 2, 3],
            [1, 3, 4],
        ],
    )
}

#[test]
fn an_apex_needs_a_support_on_supports_and_is_held_by_the_plate_without() {
    let mesh = pyramid();
    let plate = Faces::of(&mesh).expect("a pyramid has faces");
    let held = plate.score(Vec3::NEG_Z);
    assert_eq!(held.low_points, 0, "the apex rests on the plate");

    let lifted = Faces::of(&mesh)
        .expect("a pyramid has faces")
        .for_settings(&on_supports());
    let hanging = lifted.score(Vec3::NEG_Z);
    // Its sides lean 27 degrees from vertical, so nothing else needs holding.
    assert!(hanging.overhang_mm2 < 1e-3);
    assert_eq!(hanging.low_points, 1, "the apex is a point of its own");
    assert!(hanging.total > held.total, "and it costs something");
}

#[test]
fn support_under_a_high_ceiling_is_its_overhang_times_its_height() {
    let mut table = cuboid(Vec3::new(2.0, 2.0, 10.0));
    let top = cuboid(Vec3::new(10.0, 10.0, 2.0));
    let offset = table.vertices.len() as u32;
    table.vertices.extend(
        top.vertices
            .iter()
            .map(|v| *v + Vec3::new(-4.0, -4.0, 10.0)),
    );
    table.faces.extend(
        top.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
    let score = Faces::of(&table)
        .expect("the table has faces")
        .score(Vec3::NEG_Z);
    // Every overhang here is the underside of the top, 10 mm over the plate.
    assert!(
        (score.support_mm3 - score.overhang_mm2 * 10.0).abs() < 1e-1,
        "{} against {} x 10 mm",
        score.support_mm3,
        score.overhang_mm2
    );
}

#[test]
fn a_cube_on_supports_stands_its_floor_on_the_lift() {
    let cube = cuboid(Vec3::splat(10.0));
    let settings = OrientSettings {
        on_supports: true,
        lift_mm: 5.0,
        ..OrientSettings::default()
    };
    let faces = Faces::of(&cube)
        .expect("a cube has faces")
        .for_settings(&settings);
    let score = faces.score(Vec3::NEG_Z);
    let upright_mm3 = volume_mm3(&score, 0.0);
    assert!(
        (upright_mm3 - 500.0).abs() < 1e-2,
        "only the 100 mm2 floor hangs, 5 mm up on the lift: got {upright_mm3}"
    );

    // Leaned 30°: the floor's plan shrinks to 86.6 mm2, its middle stands 2.5 mm over
    // the low edge, and the lift is under it all: 86.6 * (2.5 + 5).
    let lean = 30.0_f32.to_radians();
    let leaning = faces.score(Vec3::new(lean.sin(), 0.0, -lean.cos()));
    let leaning_mm3 = volume_mm3(&leaning, 0.0);
    assert!(
        (leaning_mm3 - 649.5).abs() < 0.5,
        "the floor stands higher leaned, got {leaning_mm3}"
    );
    // Set leaning on the plate on its low edge, the same floor stands on no lift.
    let seated = faces.score_on_plate(Vec3::new(lean.sin(), 0.0, -lean.cos()));
    let seated_mm3 = volume_mm3(&seated, 0.0);
    assert!(
        (seated_mm3 - 216.5).abs() < 0.5,
        "86.6 mm2 held 2.5 mm up and nothing on a lift, got {seated_mm3}"
    );
}

#[test]
fn the_shortest_print_reads_the_lift_a_model_on_supports_stands_on() {
    let cube = cuboid(Vec3::splat(10.0));
    let settings = OrientSettings {
        on_supports: true,
        lift_mm: 5.0,
        ..OrientSettings::default()
    };
    let faces = Faces::of(&cube)
        .expect("a cube has faces")
        .for_settings(&settings);
    let goal = OrientGoal::ShortestPrint;
    let lifted = faces.reading(goal, &faces.score(Vec3::NEG_Z));
    let seated = faces.reading(goal, &faces.score_on_plate(Vec3::NEG_Z));
    assert!(
        (lifted - 15.0).abs() < 1e-3 && (seated - 10.0).abs() < 1e-3,
        "a 10 mm cube stacks 15 mm on a 5 mm lift and 10 mm on the plate: {lifted}, {seated}"
    );
}

#[test]
fn a_face_leaning_at_the_overhang_angle_is_held() {
    // Leaned 45°, the floor and one side meet the default 45° overhang angle exactly.
    let cube = cuboid(Vec3::splat(10.0));
    let settings = OrientSettings {
        on_supports: true,
        ..OrientSettings::default()
    };
    let faces = Faces::of(&cube)
        .expect("a cube has faces")
        .for_settings(&settings);
    let lean = 45.0_f32.to_radians();
    let score = faces.score(Vec3::new(lean.sin(), 0.0, -lean.cos()));
    assert!(
        score.under_mm2 > 140.0,
        "both faces, 70.7 mm2 of plan each, are held: got {}",
        score.under_mm2
    );
}

#[test]
fn under_an_overhang_angle_of_a_degree_no_face_facing_up_is_held() {
    let cube = cuboid(Vec3::splat(10.0));
    let settings = OrientSettings {
        on_supports: true,
        max_overhang_deg: 0.5,
        ..OrientSettings::default()
    };
    let faces = Faces::of(&cube)
        .expect("a cube has faces")
        .for_settings(&settings);
    let lean = 0.3_f32.to_radians();
    let score = faces.score(Vec3::new(-lean.sin(), 0.0, -lean.cos()));
    let plan = 100.0 * (lean.cos() + lean.sin());
    assert!(
        (score.under_mm2 - plan).abs() < 1e-2,
        "the floor and the side facing down, {plan} mm2 of plan: got {}",
        score.under_mm2
    );
}

#[test]
fn a_tall_box_is_scored_shorter_lying_down() {
    let box_mesh = cuboid(Vec3::new(10.0, 10.0, 60.0));
    let faces = Faces::of(&box_mesh).expect("a box has faces");
    assert!((faces.score(Vec3::NEG_Z).height_mm - 60.0).abs() < 1e-3);
    assert!((faces.score(Vec3::NEG_X).height_mm - 10.0).abs() < 1e-3);
}

#[test]
fn the_shadow_stands_in_for_the_section_until_one_is_measured() {
    let cube = cuboid(Vec3::splat(10.0));
    let faces = Faces::of(&cube)
        .expect("a cube has faces")
        .for_settings(&peel());
    let score = faces.score(Vec3::NEG_Z);

    assert_eq!(score.peak_mm2, None);
    assert!(
        (score.shadow_mm2 - 100.0).abs() < 1e-3,
        "a cube's shadow is one face, got {}",
        score.shadow_mm2
    );

    // A hollow model's section is smaller than its shadow, and scores better for it.
    let disc_of_40_mm2 = disc_pull_n(2.0 * (40.0 / std::f64::consts::PI).sqrt()) as Scalar;
    let measured = faces.with_cut(
        score,
        Cut {
            peak_mm2: 40.0,
            pull_n: disc_of_40_mm2,
            cup_mm2: 0.0,
        },
    );
    assert_eq!(measured.peak_mm2, Some(40.0));
    assert!(measured.total < score.total);
}

/// The least peel, which weighs what a section pulls on the film.
fn peel() -> OrientSettings {
    OrientSettings {
        preset: OrientGoal::LeastPeel.into(),
        ..OrientSettings::default()
    }
}

#[test]
fn a_thin_slab_casts_a_smaller_shadow_on_its_edge_than_on_its_face() {
    let slab = cuboid(Vec3::new(40.0, 40.0, 4.0));
    let faces = Faces::of(&slab)
        .expect("a slab has faces")
        .for_settings(&peel());

    let flat = faces.score(Vec3::NEG_Z);
    let edge = faces.score(Vec3::NEG_X);
    assert!((flat.shadow_mm2 - 1600.0).abs() < 1e-2);
    assert!((edge.shadow_mm2 - 160.0).abs() < 1e-2);
    assert!(edge.total < flat.total, "the peel is what a slab fails on");
}

#[test]
fn a_mesh_with_no_area_has_nothing_to_score() {
    let flat = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::X * 2.0], vec![[0, 1, 2]]);
    assert!(Faces::of(&flat).is_none());
    assert!(Faces::of(&Mesh::default()).is_none());
}
