use core_geometry::{Vec3, cuboid, diagnose};

use super::*;

/// A cube `side` across from `low`.
fn cube(low: Scalar, side: Scalar) -> Mesh {
    cuboid(Vec3::splat(low), Vec3::splat(low + side))
}

/// A 20 mm square lying flat at 10 mm: no volume, closed over or not.
fn sheet() -> Mesh {
    Mesh::new(
        vec![
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::new(30.0, 10.0, 10.0),
            Vec3::new(30.0, 30.0, 10.0),
            Vec3::new(10.0, 30.0, 10.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

/// Two 20 mm cubes overlapping in a 10 mm cube at their shared corner.
fn pair() -> Vec<Mesh> {
    vec![cube(0.0, 20.0), cube(10.0, 20.0)]
}

fn volume(combined: &Combined) -> Scalar {
    assert_eq!(
        diagnose(&combined.mesh).boundary_edges,
        0,
        "a combination is closed"
    );
    signed_volume(&combined.mesh)
}

#[test]
fn an_exact_combination_ends_on_making_its_mesh() {
    let progress = crate::Progress::default();
    let combined = combine(
        &pair(),
        Boolean::Union,
        Cancel::never().reporting(&progress),
    )
    .expect("two cubes combine");

    assert_eq!(combined.voxel_mm, None, "two solids combine exactly");
    assert_eq!(progress.read(), Some((Step::Finishing, 0, 0)));
}

#[test]
fn two_solids_combine_exactly_by_their_shared_corner() {
    for (op, expected) in [
        (Boolean::Union, 15_000.0),
        (Boolean::Difference, 7_000.0),
        (Boolean::Intersection, 1_000.0),
    ] {
        let combined = combine(&pair(), op, Cancel::never()).expect("two cubes combine");
        assert_eq!(combined.voxel_mm, None, "{op:?} of two solids is exact");
        let got = volume(&combined);
        assert!(
            (got - expected).abs() < 1e-2,
            "{op:?}: 8000 mm³ each, 1000 shared, so {expected}; got {got}"
        );
    }
}

#[test]
fn combining_toward_a_side_answers_what_combining_whole_does() {
    // A 20 mm cube with a 5 mm one standing half out of its top, seen from above and
    // from below, where the cut would take the whole cube and is not made.
    let models = vec![
        cube(0.0, 20.0),
        cuboid(Vec3::new(5.0, 5.0, 17.5), Vec3::new(10.0, 10.0, 22.5)),
    ];
    for toward in [Vec3::Z, -Vec3::Z] {
        for (op, expected) in [
            (Boolean::Union, 8_000.0 + 62.5),
            (Boolean::Difference, 8_000.0 - 62.5),
            (Boolean::Intersection, 62.5),
        ] {
            let mesh = combine_exact_toward(&models, op, toward).expect("both are solids");
            let got = volume(&Combined {
                mesh,
                voxel_mm: None,
            });
            assert!(
                (got - expected).abs() < 1e-2,
                "{op:?} toward {toward}: 8000 mm³ and half of 125 outside it; got {got}"
            );
        }
    }
}

#[test]
fn combining_toward_a_side_refuses_what_combining_whole_does() {
    assert_eq!(
        combine_exact_toward(&[cube(0.0, 1.0)], Boolean::Union, Vec3::Z),
        Err(VolumeError::TooFewModels(1))
    );
    assert_eq!(
        combine_exact_toward(&[cube(0.0, 1.0), Mesh::default()], Boolean::Union, Vec3::Z),
        Err(VolumeError::EmptyMesh)
    );
    assert!(matches!(
        combine_exact_toward(&[sheet(), cube(5.0, 20.0)], Boolean::Union, Vec3::Z),
        Err(VolumeError::Csg(CsgError::NotSolid(_)))
    ));
}

#[test]
fn a_difference_takes_every_other_model_out_of_the_first() {
    // Two cubes overlapping each other inside the first are taken out once.
    let models = vec![cube(0.0, 20.0), cube(5.0, 10.0), cube(10.0, 10.0)];
    let got = volume(&combine(&models, Boolean::Difference, Cancel::never()).expect("cut"));
    let expected = 8_000.0 - (1_000.0 + 1_000.0 - 125.0);
    assert!(
        (got - expected).abs() < 1e-2,
        "8000 less two 1000 mm³ cubes sharing 125; got {got}"
    );
}

#[test]
fn a_torn_model_is_mended_and_combined_exactly() {
    let mut torn = cube(10.0, 20.0);
    torn.faces.truncate(10);
    let models = vec![cube(0.0, 20.0), torn];
    let combined = combine(&models, Boolean::Union, Cancel::never()).expect("union");
    assert_eq!(
        combined.voxel_mm, None,
        "a cube missing half a face is mended, not sampled"
    );
    let got = volume(&combined);
    assert!(
        (got - 15_000.0).abs() < 1e-2,
        "8000 mm³ each, 1000 shared, so 15000; got {got}"
    );
}

#[test]
fn a_model_no_mending_closes_is_combined_through_the_field() {
    let models = vec![cube(0.0, 20.0), sheet()];
    let combined = combine(&models, Boolean::Union, Cancel::never()).expect("the field answers");
    assert!(
        combined.voxel_mm.is_some(),
        "a sheet holds nothing to cut exactly"
    );
    let got = volume(&combined);
    assert!(
        (got - 8_000.0).abs() < 0.03 * 8_000.0,
        "the 8000 mm³ cube within the lattice's rounding, the sheet adding nothing; got {got}"
    );
}

#[test]
fn a_model_of_crossing_shells_is_unioned_before_it_is_cut() {
    // Two cubes in one mesh, crossing in a 10 mm cube: 15000 mm³ as one solid.
    let mut crossing = cube(0.0, 20.0);
    crossing.append(&cube(10.0, 20.0));
    let models = vec![crossing, cube(40.0, 10.0)];
    let combined = combine(&models, Boolean::Union, Cancel::never()).expect("union");
    assert_eq!(
        combined.voxel_mm, None,
        "crossing shells are still cut exactly"
    );
    let got = volume(&combined);
    assert!(
        (got - 16_000.0).abs() < 1e-2,
        "15000 for the pair and 1000 apart; got {got}"
    );
}

#[test]
fn an_exact_combination_refuses_what_it_cannot_read() {
    assert!(matches!(
        combine_exact(&[cube(0.0, 20.0), sheet()], Boolean::Union),
        Err(VolumeError::Csg(CsgError::NotSolid(_)))
    ));
    assert_eq!(
        combine_exact(&[cube(0.0, 10.0), cube(20.0, 10.0)], Boolean::Intersection),
        Err(VolumeError::NothingLeft)
    );
}

#[test]
fn models_that_share_nothing_intersect_in_nothing() {
    let apart = vec![cube(0.0, 10.0), cube(20.0, 10.0)];
    assert_eq!(
        combine(&apart, Boolean::Intersection, Cancel::never()),
        Err(VolumeError::NothingLeft)
    );
}

#[test]
fn one_model_or_an_empty_one_is_refused() {
    assert_eq!(
        combine(&[cube(0.0, 1.0)], Boolean::Union, Cancel::never()),
        Err(VolumeError::TooFewModels(1))
    );
    assert_eq!(
        combine(
            &[cube(0.0, 1.0), Mesh::default()],
            Boolean::Union,
            Cancel::never()
        ),
        Err(VolumeError::EmptyMesh)
    );
}

#[test]
fn a_run_given_up_on_stops_in_the_field() {
    let stop = || true;
    assert_eq!(
        combine(
            &[cube(0.0, 20.0), sheet()],
            Boolean::Union,
            Cancel::when(&stop)
        ),
        Err(VolumeError::Cancelled)
    );
}
