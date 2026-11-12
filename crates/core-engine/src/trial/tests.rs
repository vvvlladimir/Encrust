use core_geometry::cuboid;
use core_plate::Score;
use core_volume::{DrainHole, HoleKind, HoleShape, drill};
use printer_profiles::Compensation;

use super::*;
use crate::bake::bake;

/// A pose that prints `supports_ml`, on the plate or lifted, with `hanging` contacts.
fn tried(on_plate: bool, supports_ml: f32, hanging: usize) -> Proven {
    Proven {
        oriented: Oriented {
            rotation: Quat::from_rotation_x(if on_plate { 0.0 } else { 1.0 }),
            score: Score {
                overhang_mm2: 0.0,
                low_points: 0,
                support_mm3: 0.0,
                under_mm2: 0.0,
                under_mm3: 0.0,
                seen_mm2: 0.0,
                bottom_mm2: 0.0,
                height_mm: 0.0,
                shadow_mm2: 0.0,
                peak_mm2: None,
                peak_pull_n: None,
                cup_mm2: None,
                lift_mm: 0.0,
                total: 0.0,
            },
            on_plate,
            overflows: false,
            considered: 0,
            ruling: Ruling::Chosen,
        },
        printout: Printout {
            supports_ml,
            hanging,
            pull_n: 5.0,
            stack_mm: 20.0,
            ..Printout::default()
        },
    }
}

#[test]
fn a_seat_with_something_hanging_fails_like_a_cup_for_auto_alone() {
    let poses = [tried(true, 0.5, 1), tried(false, 2.0, 0)];
    let first = |preset| {
        judged(&poses, preset, |_| false)[0]
            .proven
            .oriented
            .on_plate
    };
    assert!(!first(OrientPreset::Auto));
    for goal in OrientGoal::ALL {
        assert!(first(OrientPreset::Goal(goal)), "{goal:?}");
    }
}

#[test]
fn a_clean_surface_is_judged_on_the_contacts_printed_where_they_show() {
    let mut poses = [tried(false, 0.5, 0), tried(false, 1.5, 0)];
    poses[0].printout.seen_contacts = 6.0;
    poses[1].printout.seen_contacts = 2.0;
    let asked = Asked {
        preset: OrientGoal::FewestSupports.into(),
        clean_surface: true,
    };
    let first = judged(&poses, asked, |_| false)[0];
    assert_eq!(
        first.proven.printout.supports_ml, 1.5,
        "2 on show beat 6, at three times the resin"
    );
}

#[test]
fn the_best_pose_that_fits_is_printed_when_no_finalist_does() {
    let mut found: Vec<Oriented> = (0..6)
        .map(|at| Oriented {
            rotation: Quat::from_rotation_x(0.3 * at as Scalar),
            ..tried(false, 1.0, 0).oriented
        })
        .collect();
    for pose in &mut found[..5] {
        pose.overflows = true;
    }
    let poses = to_print(&found, 4, |_| false);
    assert!(
        poses.iter().any(|pose| !pose.overflows),
        "the sixth pose, the only one that fits, is printed too"
    );
    assert_eq!(poses.len(), 5);
}

#[test]
fn a_bar_longer_than_the_plate_fits_across_its_diagonal() {
    // At 45 degrees a 100 by 4 mm bar spans (100 + 4) / sqrt 2 = 74 mm each way.
    let corners = (0..8)
        .map(|at| {
            let unit = Vec3::new(
                (at & 1) as Scalar,
                ((at >> 1) & 1) as Scalar,
                (at >> 2) as Scalar,
            );
            unit * Vec3::new(100.0, 4.0, 4.0)
        })
        .collect();
    let bar = Model::placed(
        Arc::new(Mesh::new(corners, vec![[0, 1, 2]])),
        Transform::default(),
    );
    let plate = |z: Scalar| BuildVolume {
        x: 90.0,
        y: 90.0,
        z,
    };
    assert!(!overflows(&bar, &plate(10.0)));
    assert!(overflows(&bar, &plate(3.0)), "4 mm tall in a 3 mm volume");
}

#[test]
fn a_trials_cuts_drill_its_raft_and_pass_its_trees() {
    let model = cuboid(Vec3::new(0.0, 0.0, 2.0), Vec3::new(10.0, 10.0, 6.0));
    let tree = cuboid(Vec3::new(4.5, 4.5, 1.0), Vec3::new(5.5, 5.5, 2.0));
    let raft = cuboid(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(11.0, 11.0, 1.0));
    // A 4 mm square bore 2 mm up into the floor, its mouth 3 mm below: through the raft.
    let hole = DrainHole {
        at: Vec3::new(5.0, 5.0, 2.0),
        axis: Vec3::Z,
        diameter_mm: 4.0 * std::f32::consts::SQRT_2,
        depth_mm: 2.0,
        taper: 1.0,
        lift_mm: 3.0,
        shape: HoleShape::Polygon { sides: 4 },
        turn_deg: 45.0,
        kind: HoleKind::Drain,
    };
    let cuts = drill(&[hole], &[]).expect("the hole drills");
    let stood = Stood {
        lifted_mm: 2.0,
        contacts: Vec::new(),
        facing: Vec::new(),
        sized: Vec::new(),
        standing: 0,
        unheld: Vec::new(),
        trees: 1,
        supports: tree,
        raft: Some(raft),
    };
    let model = on_its_supports(model, Some(cuts), stood);
    let baked = bake(&[model], &Compensation::default()).expect("the plate has material");
    let volume = signed_volume(&baked.solid.mesh);
    // 400 - 16 * 2 of model, the 1 mm3 tree whole, 144 - 16 * 1 of raft.
    assert!(
        (volume - 497.0).abs() < 0.5,
        "expected 497 mm3, the tree left whole, got {volume}"
    );
}
