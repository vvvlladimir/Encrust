use super::*;
use crate::score::OrientGoal;
use core_geometry::Mesh;

fn cube() -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0))
}

/// A measured candidate pointing `down`, costing `overhang_mm2` of support held 10 mm up,
/// and `total`.
fn candidate(down: Vec3, overhang_mm2: f32, total: f32, cup_mm2: f32) -> Candidate {
    Candidate {
        down,
        score: Score {
            overhang_mm2,
            low_points: 0,
            support_mm3: overhang_mm2 * 10.0,
            under_mm2: overhang_mm2,
            under_mm3: overhang_mm2 * 10.0,
            bottom_mm2: 0.0,
            height_mm: 10.0,
            shadow_mm2: 100.0,
            peak_mm2: Some(100.0),
            peak_pull_n: Some(1.0),
            cup_mm2: Some(cup_mm2),
            lift_mm: 0.0,
            total,
        },
        on_plate: false,
        turn: Some(0.0),
    }
}

/// Where `candidate` ranks among `measured`, best first being 0.
fn place(measured: &[Candidate], candidate: usize, now: Vec3) -> usize {
    place_for(OrientGoal::FewestSupports, measured, candidate, now)
}

fn place_for(
    preset: impl Into<OrientPreset>,
    measured: &[Candidate],
    candidate: usize,
    now: Vec3,
) -> usize {
    let settings = OrientSettings {
        preset: preset.into(),
        ..OrientSettings::default()
    };
    let mesh = cube();
    let faces = Faces::of(&mesh)
        .expect("a cube has faces")
        .for_settings(&settings);
    let order = ranked(&faces, &settings, measured, now);
    order
        .iter()
        .position(|(at, _)| *at == candidate)
        .expect("every candidate is ranked")
}

#[test]
fn a_suction_cup_loses_whatever_it_scores_but_for_the_fewest_supports() {
    let cupping = candidate(Vec3::X, 0.0, 0.1, 400.0);
    let clean = candidate(Vec3::Y, 50.0, 9.0, 0.0);
    let peel = OrientGoal::LeastPeel;
    assert_eq!(place_for(peel, &[cupping, clean], 1, Vec3::NEG_Z), 0);
    assert_eq!(
        place(&[cupping, clean], 0, Vec3::NEG_Z),
        0,
        "a cup is a drain hole's to fix, not more support's"
    );
}

#[test]
fn a_pose_past_the_build_volume_loses_whatever_it_scores() {
    let mut overflowing = candidate(Vec3::X, 0.0, 0.1, 0.0);
    overflowing.turn = None;
    let fitting = candidate(Vec3::Y, 50.0, 9.0, 0.0);
    for preset in OrientPreset::all() {
        assert_eq!(
            place_for(preset, &[overflowing, fitting], 1, Vec3::NEG_Z),
            0,
            "{}",
            preset.label()
        );
    }
}

#[test]
fn the_goals_own_term_decides_before_the_total() {
    // Fewest supports: 100 mm3 of support against 600, whatever the other terms add.
    let few = candidate(Vec3::X, 10.0, 5.0, 0.0);
    let many = candidate(Vec3::Y, 60.0, 1.0, 0.0);
    assert_eq!(place(&[many, few], 1, Vec3::NEG_Z), 0);

    // No band: the least support wins however close the next, whatever its total.
    let close = candidate(Vec3::NEG_X, 10.5, 4.0, 0.0);
    assert_eq!(place(&[few, close], 1, Vec3::NEG_Z), 1);
}

#[test]
fn the_fewest_supports_turn_for_any_less_support() {
    // The gain is the goal's own: the support volume, not the total.
    let now = candidate(Vec3::NEG_Z, 10.0, 1.0, 0.0);
    let slightly = candidate(Vec3::X, 9.8, 2.0, 0.0);
    let same = candidate(Vec3::Y, 10.0, 0.5, 0.0);
    assert_eq!(
        place(&[slightly, now], 0, Vec3::NEG_Z),
        0,
        "2 % less is less"
    );
    assert_eq!(place(&[same, now], 1, Vec3::NEG_Z), 0, "as much stays");
}

#[test]
fn within_its_band_the_shortest_print_takes_the_shorter_stack_not_the_lesser_support() {
    let mut short = candidate(Vec3::X, 60.0, 5.0, 0.0);
    short.score.height_mm = 10.0;
    let mut tall = candidate(Vec3::Y, 10.0, 1.0, 0.0);
    tall.score.height_mm = 10.5;
    assert_eq!(
        place_for(OrientGoal::ShortestPrint, &[tall, short], 1, Vec3::NEG_Z),
        0,
        "10.5 mm is within 10 * 1.02 + 1, and support is not weighed (ADR 0283)"
    );
}

#[test]
fn auto_ranks_each_goals_own_best_right_behind_its_answer() {
    let mut tall = candidate(Vec3::X, 1.0, 1.0, 0.0);
    tall.score.height_mm = 60.0;
    let mut pulling = candidate(Vec3::Y, 30.0, 2.0, 0.0);
    pulling.score.peak_pull_n = Some(5.0);
    let mut short = candidate(Vec3::Z, 40.0, 3.0, 0.0);
    short.score.height_mm = 5.0;
    let middling = candidate(Vec3::NEG_X, 20.0, 0.5, 0.0);
    let mut measured = [middling, tall, pulling, short];
    for (at, pose) in measured.iter_mut().enumerate() {
        if at != 2 {
            pose.score.peak_pull_n = Some(20.0);
        }
    }

    let settings = OrientSettings {
        preset: OrientPreset::Auto,
        ..OrientSettings::default()
    };
    let mesh = cube();
    let faces = Faces::of(&mesh)
        .expect("a cube has faces")
        .for_settings(&settings);
    let order = ranked(&faces, &settings, &measured, Vec3::NEG_Z);
    let ends: Vec<usize> = order.iter().take(4).map(|(at, _)| *at).collect();
    for (goal, best) in [
        (OrientGoal::FewestSupports, 1),
        (OrientGoal::LeastPeel, 2),
        (OrientGoal::ShortestPrint, 3),
    ] {
        assert_eq!(place_for(goal, &measured, best, Vec3::NEG_Z), 0);
        assert!(ends.contains(&best), "{} leads the proof", goal.label());
    }
}

#[test]
fn auto_follows_its_answer_with_the_shortest_stack_not_auto_s_own_tiebreak() {
    let pose = |down: Vec3, overhang_mm2: f32, total: f32, height_mm: f32| {
        let mut pose = candidate(down, overhang_mm2, total, 0.0);
        pose.score.height_mm = height_mm;
        pose
    };
    let measured = [
        pose(Vec3::X, 60.0, 1.0, 30.0),
        pose(Vec3::Y, 120.0, 1.0, 28.0),
        pose(Vec3::Z, 180.0, 1.0, 26.0),
        pose(Vec3::NEG_X, 240.0, 1.0, 24.0),
        pose(Vec3::new(1.0, 1.0, 0.0).normalize(), 1800.0, 0.1, 10.5),
        pose(Vec3::NEG_Y, 2400.0, 9.0, 10.0),
    ];
    let short = 5;
    assert_eq!(
        place_for(OrientGoal::ShortestPrint, &measured, short, Vec3::NEG_Z),
        0
    );

    let order = ranked_for(OrientPreset::Auto, &measured);
    assert!(
        order
            .iter()
            .take(OrientGoal::ALL.len() + 1)
            .any(|(at, _)| *at == short),
        "10 mm is the shortest print's own best, whatever auto's total says: {order:?}"
    );
}

fn ranked_for(preset: OrientPreset, measured: &[Candidate]) -> Vec<(usize, Ruling)> {
    let settings = OrientSettings {
        preset,
        ..OrientSettings::default()
    };
    let mesh = cube();
    let faces = Faces::of(&mesh)
        .expect("a cube has faces")
        .for_settings(&settings);
    ranked(&faces, &settings, measured, Vec3::NEG_Z)
}
