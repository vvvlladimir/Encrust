use std::cell::Cell;

use super::*;
use crate::clear::clear;
use crate::fixtures::{box_mesh, columns_on, profile, under_a_gap_over_a_bar};
use crate::{Profiles, SupportPoint, grow};
use core_geometry::{Bvh, Transform, Vec3};

/// A beam straight down beside a 10 mm cube, clear of it.
fn beside_a_cube() -> Beam {
    Beam {
        from: Vec3::new(15.0, 5.0, 8.0),
        to: Vec3::new(15.0, 5.0, 1.0),
        from_radius_mm: 0.5,
        to_radius_mm: 0.5,
    }
}

#[test]
fn a_beam_asked_about_twice_is_swept_once() {
    let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let bvh = Bvh::build(&model);
    let kept = Clearances::default();
    let placed = Placed::new(&model, &bvh, Transform::default()).remembering(&kept);
    let sweeps = Cell::new(0);
    let sweep = || {
        sweeps.set(sweeps.get() + 1);
        true
    };

    assert!(kept.answer(&placed, &beside_a_cube(), sweep));
    assert!(kept.answer(&placed, &beside_a_cube(), sweep));
    assert_eq!(sweeps.get(), 1, "the second answer is the kept one");
}

#[test]
fn moving_the_model_forgets_what_was_kept() {
    let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let bvh = Bvh::build(&model);
    let kept = Clearances::default();
    let here = Placed::new(&model, &bvh, Transform::default()).remembering(&kept);
    let moved = Transform::from_translation(Vec3::new(10.0, 0.0, 0.0));
    let there = Placed::new(&model, &bvh, moved).remembering(&kept);

    assert!(clear(&here, &beside_a_cube()));
    assert!(
        !clear(&there, &beside_a_cube()),
        "moved 10 mm along x, the cube stands where the beam runs"
    );
}

#[test]
fn a_full_shard_forgets_and_keeps_answering() {
    let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let bvh = Bvh::build(&model);
    let kept = Clearances::default();
    let placed = Placed::new(&model, &bvh, Transform::default());
    for step in 0..=SHARDS * MAX_ANSWERS_PER_SHARD {
        let beam = Beam {
            from_radius_mm: step as f32 * 1e-6,
            ..beside_a_cube()
        };
        assert!(kept.answer(&placed, &beam, || true));
    }
    let most = kept
        .shards
        .iter()
        .map(|shard| shard.lock().expect("no test thread panicked").answers.len())
        .max();
    assert!(
        most <= Some(MAX_ANSWERS_PER_SHARD),
        "no shard holds past its bound, got {most:?}"
    );
}

#[test]
fn a_forest_grown_from_kept_answers_is_the_one_grown_without_them() {
    let model = under_a_gap_over_a_bar();
    let bvh = Bvh::build(&model);
    let profile = profile();
    let profiles = Profiles::single(&profile);
    let points: Vec<SupportPoint> = (0..6)
        .map(|step| SupportPoint::new(Vec3::new(-4.0 + 1.5 * step as f32, 0.0, 9.0)))
        .collect();
    let columns = columns_on(&points, &model, Transform::default(), profiles);
    let fresh = grow(
        &columns,
        &Placed::new(&model, &bvh, Transform::default()),
        profiles,
    );

    let kept = Clearances::default();
    let placed = Placed::new(&model, &bvh, Transform::default()).remembering(&kept);
    let first = grow(&columns, &placed, profiles);
    let again = grow(&columns, &placed, profiles);

    assert!(!fresh.is_empty(), "the bar is held up");
    assert_eq!(first, fresh);
    assert_eq!(
        again, fresh,
        "answered from what was kept, the forest is the same"
    );
}
