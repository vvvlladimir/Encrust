use super::*;

/// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
fn cuboid(size: Vec3) -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, size)
}

/// Where the model's own `axis` ends up once the orientation is applied.
fn placed(rotation: Quat, axis: Vec3) -> Vec3 {
    rotation * axis
}

#[test]
fn a_slab_pulling_past_the_limit_is_turned_off_its_face_for_the_least_peel() {
    let mesh = cuboid(Vec3::new(100.0, 100.0, 4.0));
    let settings = OrientSettings {
        preset: OrientGoal::LeastPeel.into(),
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &settings).expect("a slab can be oriented");

    // Lying flat, a 100 mm square pulls some 98 N; turned, each section is a strip.
    let thin = placed(found.rotation, Vec3::Z);
    assert!(
        thin.z.abs() < 0.95,
        "the slab does not lie flat, got {thin}"
    );
    let pull = found.score.peak_pull_n.expect("the winner was cut");
    assert!(
        f64::from(pull) < PEEL_LIMIT_N / 3.0,
        "under enough, got {pull} N"
    );
    assert!(found.considered > 100, "the sphere was searched too");
}

#[test]
fn a_slab_pulling_little_enough_stays_flat_for_the_least_peel() {
    // A 40 mm square pulls some 8 N, under a third of the limit: nothing to win.
    let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
    let settings = OrientSettings {
        preset: OrientGoal::LeastPeel.into(),
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &settings).expect("a slab can be oriented");
    let thin = placed(found.rotation, Vec3::Z);
    assert!(thin.z.abs() > 0.95, "the slab still lies flat, got {thin}");
}

#[test]
fn a_slab_lying_on_the_plate_stays_there_for_the_fewest_supports() {
    // Flat or on its edge, nothing hangs: no turn wins the goal anything, and the peel
    // of 1600 mm2 is the least-peel goal's to weigh, or a tearing pull's.
    let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
    let found = orient(&mesh, &OrientSettings::default()).expect("a slab can be oriented");
    let thin = placed(found.rotation, Vec3::Z);
    assert!(thin.z.abs() > 0.95, "the slab still lies flat, got {thin}");
}

#[test]
fn a_shortest_print_lays_a_tall_column_down() {
    let mesh = cuboid(Vec3::new(6.0, 6.0, 80.0));
    let settings = OrientSettings {
        preset: OrientGoal::ShortestPrint.into(),
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &settings).expect("a column can be oriented");
    // Lying down it is 6 mm tall rather than 80: far fewer layers.
    let long_axis = placed(found.rotation, Vec3::Z);
    assert!(
        long_axis.z.abs() < 0.05,
        "the long axis lies flat, got {long_axis}"
    );
}

#[test]
fn no_tilt_allowed_leaves_the_model_standing_as_it_is() {
    let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
    let settings = OrientSettings {
        max_tilt_deg: Some(0.0),
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &settings).expect("a slab can be oriented");
    // Unbounded, the slab is stood on its edge; held to no tilt it stays flat.
    let thin = placed(found.rotation, Vec3::Z);
    assert!(thin.z.abs() > 0.999, "the slab still lies flat, got {thin}");
}

#[test]
fn a_tall_column_prints_standing_up() {
    let mesh = cuboid(Vec3::new(6.0, 6.0, 80.0));
    let found = orient(&mesh, &OrientSettings::default()).expect("a column can be oriented");

    // 36 mm2 a layer standing, against 480 mm2 lying down: the extra layers cost less
    // than the peel would.
    let long_axis = placed(found.rotation, Vec3::Z);
    assert!(
        long_axis.z.abs() > 0.99,
        "the long axis stays vertical, got {long_axis}"
    );
}

#[test]
fn a_cube_stands_on_a_face_whichever_one_it_picks() {
    let mesh = cuboid(Vec3::splat(20.0));
    let found = orient(&mesh, &OrientSettings::default()).expect("a cube can be oriented");

    // Every face of a cube is as good as the others, so what is pinned down is that
    // one of them is flat on the plate: an axis of the model points straight down.
    let down: Vec3 = [Vec3::X, Vec3::Y, Vec3::Z]
        .into_iter()
        .map(|axis| placed(found.rotation, axis))
        .max_by(|a, b| a.z.abs().total_cmp(&b.z.abs()))
        .expect("three axes");
    assert!(
        down.z.abs() > 0.999,
        "a face is flat on the plate, got {down}"
    );
}

#[test]
fn a_model_needing_no_supports_rests_on_the_plate() {
    let mesh = cuboid(Vec3::new(40.0, 40.0, 4.0));
    let settings = OrientSettings {
        on_supports: true,
        rest_on_plate: true,
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &settings).expect("a slab can be oriented");
    // On a face or on an edge nothing of a box hangs, and neither pull is near the limit.
    assert!(found.on_plate, "it is judged set on the plate");
    let upright = [Vec3::X, Vec3::Y, Vec3::Z]
        .into_iter()
        .any(|axis| placed(found.rotation, axis).z.abs() > 0.999);
    assert!(upright, "with one of its faces flat on it");

    let lifted = orient(
        &mesh,
        &OrientSettings {
            rest_on_plate: false,
            ..settings
        },
    )
    .expect("a slab can be oriented");
    assert!(!lifted.on_plate, "unasked, it is judged on supports");
}

#[test]
fn finalists_start_with_the_answer_and_stand_apart() {
    let mesh = cuboid(Vec3::new(30.0, 20.0, 10.0));
    let settings = OrientSettings {
        on_supports: true,
        ..OrientSettings::default()
    };
    let answer = orient(&mesh, &settings).expect("a box can be oriented");
    let found = finalists(&Printed::solid(&mesh), &settings, 5, Watch::never())
        .expect("a box has finalists");

    assert_eq!(found.len(), 5);
    assert_eq!(found[0], answer);
    for (at, pose) in found.iter().enumerate() {
        for other in &found[..at] {
            let apart = (pose.rotation.inverse() * Vec3::NEG_Z)
                .angle_between(other.rotation.inverse() * Vec3::NEG_Z);
            assert!(apart.to_degrees() > 9.9, "two finalists {apart} rad apart");
        }
    }
}

#[test]
fn a_search_tells_each_cut_of_its_shortlist_and_stops_when_asked() {
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    let mesh = cuboid(Vec3::new(30.0, 20.0, 10.0));
    let settings = OrientSettings::default();
    let told = AtomicUsize::new(0);
    let counting = |phase: Phase, done: usize, total: usize| {
        assert_eq!(phase, Phase::Looking);
        assert!(done <= total);
        told.fetch_add(1, Relaxed);
        true
    };
    finalists(&Printed::solid(&mesh), &settings, 1, Watch::by(&counting))
        .expect("a box has finalists");
    assert!(
        told.load(Relaxed) >= 2,
        "the start and at least one cut are told, got {}",
        told.load(Relaxed)
    );

    let stop = |_: Phase, _: usize, _: usize| false;
    let stopped = finalists(&Printed::solid(&mesh), &settings, 1, Watch::by(&stop));
    assert!(matches!(stopped, Err(PlateError::Cancelled)));
}

#[test]
fn the_model_is_leaned_three_ways_eight_times_round() {
    let leans: Vec<Vec3> = tilts(Vec3::NEG_Z).collect();
    assert_eq!(leans.len(), 24);
    for (at, down) in leans.iter().enumerate() {
        let lean_deg = down.angle_between(Vec3::NEG_Z).to_degrees();
        let expected = TILTS_DEG[at / TILT_SIDES];
        assert!(
            (lean_deg - expected).abs() < 1e-3,
            "{lean_deg} for {expected}"
        );
    }
}

#[test]
fn the_rotation_takes_the_chosen_direction_to_the_plate() {
    let down = Vec3::new(1.0, 2.0, -3.0).normalize();
    let turned = rotation_for(down) * down;
    assert!(
        turned.abs_diff_eq(Vec3::NEG_Z, 1e-5),
        "the chosen direction ends up pointing at the plate, got {turned}"
    );
}

#[test]
fn the_peak_section_of_a_box_is_its_footprint() {
    let mesh = cuboid(Vec3::new(10.0, 6.0, 20.0));
    let peak = cut(&Printed::solid(&mesh), Vec3::NEG_Z, 8, true, MIN_CUP_MM2)
        .expect("a box slices")
        .peak_mm2;
    assert!((peak - 60.0).abs() < 1e-2, "a 10 by 6 section, got {peak}");
}

#[test]
fn the_peak_section_counts_overlapping_bodies_once() {
    // Two 10 mm boxes sharing a 2 x 2 mm corner over their whole height: the section
    // is 100 + 100 - 4, not the 200 their contour areas add up to (ADR 0206).
    let mut mesh = cuboid(Vec3::splat(10.0));
    let second = cuboid(Vec3::splat(10.0));
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend(
        second
            .vertices
            .iter()
            .map(|v| *v + Vec3::new(8.0, 8.0, 0.0)),
    );
    mesh.faces.extend(
        second
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );

    let peak = cut(&Printed::solid(&mesh), Vec3::NEG_Z, 4, true, MIN_CUP_MM2)
        .expect("two boxes slice")
        .peak_mm2;
    assert!((peak - 196.0).abs() < 1e-2, "{peak}");
}

#[test]
fn a_shortest_print_lays_a_slab_flat_whatever_it_pulls() {
    let mesh = cuboid(Vec3::new(100.0, 100.0, 4.0));
    let settings = OrientSettings {
        preset: OrientGoal::ShortestPrint.into(),
        on_supports: true,
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &settings).expect("a slab can be oriented");
    // Flat, a 100 mm square pulls some 98 N, past the 30 N limit, and stacks 4 mm: the
    // least of any pose, which the shortest print takes (ADR 0283).
    let thin = placed(found.rotation, Vec3::Z);
    assert!(thin.z.abs() > 0.999, "the slab lies flat, got {thin}");
    let pull = found.score.peak_pull_n.expect("the winner was cut");
    assert!(f64::from(pull) > PEEL_LIMIT_N, "{pull}");
}

/// An open-topped 30 mm box with 2 mm walls, open side up as its file has it.
fn open_box() -> Mesh {
    let mut mesh = Mesh::default();
    let parts = [
        (Vec3::ZERO, Vec3::new(30.0, 30.0, 2.0)),
        (Vec3::ZERO, Vec3::new(2.0, 30.0, 20.0)),
        (Vec3::new(28.0, 0.0, 0.0), Vec3::new(30.0, 30.0, 20.0)),
        (Vec3::ZERO, Vec3::new(30.0, 2.0, 20.0)),
        (Vec3::new(0.0, 28.0, 0.0), Vec3::new(30.0, 30.0, 20.0)),
    ];
    for (min, max) in parts {
        let part = cuboid(max - min);
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend(part.vertices.iter().map(|v| *v + min));
        mesh.faces.extend(
            part.faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );
    }
    mesh
}

#[test]
fn an_open_box_is_not_left_standing_as_a_suction_cup() {
    let mesh = open_box();
    let as_is =
        cut(&Printed::solid(&mesh), Vec3::NEG_Z, 24, true, MIN_CUP_MM2).expect("the box cuts");
    assert!(
        as_is.cup_mm2 > 600.0,
        "open side up, its floor seals the inside"
    );

    let peel = OrientSettings {
        preset: OrientGoal::LeastPeel.into(),
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &peel).expect("a box can be oriented");
    assert_eq!(
        found.score.cup_mm2,
        Some(0.0),
        "the turn it takes seals nothing"
    );

    let found = orient(&mesh, &OrientSettings::default()).expect("a box can be oriented");
    let floor = found.rotation * Vec3::NEG_Z;
    assert!(
        floor.z < -0.99,
        "the fewest supports leaves its floor on the plate, cup and all, got {floor}"
    );
}

/// How far `mesh` spans along each plate axis once `rotation` stands it.
fn span(mesh: &Mesh, rotation: Quat) -> Vec3 {
    let turned: Vec<Vec3> = mesh.vertices.iter().map(|v| rotation * *v).collect();
    let low = turned
        .iter()
        .copied()
        .reduce(Vec3::min)
        .expect("a box has corners");
    let high = turned
        .iter()
        .copied()
        .reduce(Vec3::max)
        .expect("a box has corners");
    high - low
}

#[test]
fn a_column_taller_than_the_printer_is_laid_down_to_fit() {
    let mesh = cuboid(Vec3::new(6.0, 6.0, 80.0));
    let settings = OrientSettings {
        build_volume_mm: Some(Vec3::new(120.0, 70.0, 50.0)),
        ..OrientSettings::default()
    };
    // Standing is the fewest supports' answer without a volume, and 80 mm is past 50.
    let found = orient(&mesh, &settings).expect("a column can be oriented");
    assert!(!found.overflows, "a pose that fits is found");
    let size = span(&mesh, found.rotation);
    assert!(size.z < 50.0, "it lies low enough, got {size}");
}

#[test]
fn a_bar_longer_than_the_plate_is_turned_across_it() {
    // 100 mm against a 90 mm square plate: lying flat it fits only at a slant, where it
    // spans (100 + 4) / sqrt 2 = 74 mm each way, inside 90 less the base's margins.
    let mesh = cuboid(Vec3::new(100.0, 4.0, 4.0));
    let settings = OrientSettings {
        on_supports: true,
        preset: OrientGoal::ShortestPrint.into(),
        build_volume_mm: Some(Vec3::new(90.0, 90.0, 40.0)),
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &settings).expect("a bar can be oriented");
    assert!(!found.overflows, "it fits turned");
    let size = span(&mesh, found.rotation);
    assert!(
        size.x < 90.0 && size.y < 90.0,
        "within the plate, got {size}"
    );
}

#[test]
fn a_model_no_pose_fits_is_still_answered_as_overflowing() {
    let mesh = cuboid(Vec3::splat(50.0));
    let settings = OrientSettings {
        build_volume_mm: Some(Vec3::splat(40.0)),
        ..OrientSettings::default()
    };
    let found = orient(&mesh, &settings).expect("a cube can be oriented");
    assert!(found.overflows, "a 50 mm cube fits no 40 mm volume");
}

#[test]
fn a_mesh_with_no_faces_cannot_be_oriented() {
    let error = orient(&Mesh::default(), &OrientSettings::default())
        .expect_err("an empty mesh has no orientation");
    assert!(matches!(error, PlateError::NothingToOrient));
}
