use core_geometry::{cuboid, diagnose, signed_volume};

use super::*;

fn block(min: [f32; 3], max: [f32; 3]) -> Solid {
    Solid::new(&cuboid(Vec3::from(min), Vec3::from(max))).expect("a box is a solid")
}

fn volume(solid: &Solid) -> f32 {
    let mesh = solid.mesh();
    let report = diagnose(&mesh);
    assert_eq!(report.boundary_edges, 0, "a boolean answers a closed mesh");
    signed_volume(&mesh)
}

fn near(actual: f32, expected: f32, why: &str) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "{why}: expected {expected}, got {actual}"
    );
}

#[test]
fn two_overlapping_cubes_combine_by_their_shared_corner() {
    // Two 2 mm cubes offset by 1 mm on every axis share a 1 mm cube.
    let (a, b) = (block([0.0; 3], [2.0; 3]), block([1.0; 3], [3.0; 3]));
    near(volume(&a.union(&b).expect("union")), 15.0, "8 + 8 - 1");
    near(volume(&a.difference(&b).expect("difference")), 7.0, "8 - 1");
    near(
        volume(&a.intersection(&b).expect("intersection")),
        1.0,
        "the shared cube",
    );
}

#[test]
fn crossing_bars_on_shared_planes_are_unioned_once() {
    // Two 10 x 2 x 2 bars crossing at the origin share their top and bottom planes.
    let x = block([-5.0, -1.0, -1.0], [5.0, 1.0, 1.0]);
    let y = block([-1.0, -5.0, -1.0], [1.0, 5.0, 1.0]);
    near(
        volume(&x.union(&y).expect("union")),
        72.0,
        "40 + 40 - the 8 they share",
    );
}

#[test]
fn solids_that_do_not_meet_intersect_in_nothing() {
    let apart = block([0.0; 3], [1.0; 3]).intersection(&block([2.0; 3], [3.0; 3]));
    assert!(apart.expect("an empty answer is not a failure").is_empty());
}

#[test]
fn the_empty_solid_is_the_identity_of_union_and_difference() {
    let cube = block([0.0; 3], [2.0; 3]);
    near(
        volume(&cube.union(&Solid::empty()).expect("union")),
        8.0,
        "unchanged",
    );
    near(
        volume(&cube.difference(&Solid::empty()).expect("difference")),
        8.0,
        "unchanged",
    );
    assert!(
        Solid::empty()
            .difference(&cube)
            .expect("difference")
            .is_empty()
    );
    assert!(
        cube.intersection(&Solid::empty())
            .expect("intersection")
            .is_empty()
    );
}

fn touching(min: [f32; 3], max: [f32; 3]) -> Solid {
    let mut both = cuboid(Vec3::ZERO, Vec3::ONE);
    both.append(&cuboid(Vec3::from(min), Vec3::from(max)));
    Solid::new(&both).expect("two cubes that only touch are a solid")
}

#[test]
fn cubes_touching_along_an_edge_are_one_solid_the_engine_can_cut() {
    // The unit cube and its diagonal neighbour share the edge x = y = 1, used four times.
    let pair = touching([1.0, 1.0, 0.0], [2.0, 2.0, 1.0]);
    near(volume(&pair), 2.0, "two unit cubes");
    let slab = block([-1.0, -1.0, 0.5], [3.0, 3.0, 2.0]);
    near(
        volume(&pair.difference(&slab).expect("difference")),
        1.0,
        "the lower half of each",
    );
}

#[test]
fn cubes_touching_at_a_corner_are_one_solid_the_engine_can_cut() {
    let pair = touching([1.0; 3], [2.0; 3]);
    near(volume(&pair), 2.0, "two unit cubes");
    let slab = block([-1.0, -1.0, 0.5], [3.0, 3.0, 1.5]);
    near(
        volume(&pair.intersection(&slab).expect("intersection")),
        1.0,
        "half of each",
    );
}

#[test]
fn an_open_mesh_is_not_a_solid() {
    let mut open = cuboid(Vec3::ZERO, Vec3::ONE);
    open.faces.truncate(10);
    assert!(matches!(Solid::new(&open), Err(CsgError::NotSolid(_))));
}

#[test]
fn a_mesh_whose_faces_all_weld_away_is_the_empty_solid() {
    let point = Vec3::splat(1.0);
    let flat = Mesh::new(vec![point, point, Vec3::ZERO], vec![[0, 1, 2]]);
    let solid = Solid::new(&flat).expect("nothing left is the empty solid");
    assert!(solid.is_empty());
}

#[test]
fn an_engine_that_panics_answers_that_the_boolean_failed() {
    let answer = guarded(|| -> Result<Manifold, String> { panic!("past its own tables") });
    assert!(
        matches!(answered(answer), Err(CsgError::Failed(_))),
        "a panic is an error the caller can act on"
    );
}

#[test]
fn solids_apart_are_set_side_by_side() {
    let apart = [block([0.0; 3], [1.0; 3]), block([3.0; 3], [5.0; 3])];
    let both = Solid::union_all(&apart).expect("union");
    near(volume(&both), 9.0, "1 + 8, nothing shared");
    assert_eq!(
        both.mesh().faces.len(),
        24,
        "two boxes of twelve faces, untouched"
    );
}

#[test]
fn solids_that_meet_are_unioned_and_the_rest_set_beside_them() {
    let parts = [
        block([0.0; 3], [2.0; 3]),
        block([10.0; 3], [11.0; 3]),
        block([1.0; 3], [3.0; 3]),
    ];
    near(
        volume(&Solid::union_all(&parts).expect("union")),
        16.0,
        "8 + 8 - the 1 they share, and 1 apart",
    );
}

#[test]
fn boxes_touching_on_a_face_are_unioned_not_set_side_by_side() {
    let parts = [
        block([0.0; 3], [1.0; 3]),
        block([1.0, 0.0, 0.0], [2.0, 1.0, 1.0]),
    ];
    let both = Solid::union_all(&parts).expect("union");
    near(volume(&both), 2.0, "two unit cubes");
    assert!(
        both.mesh().faces.len() < 24,
        "the shared face is gone, not left inside"
    );
}

#[test]
fn the_union_of_nothing_is_empty() {
    assert!(Solid::union_all(&[]).expect("union").is_empty());
    assert!(
        Solid::union_all(&[Solid::empty()])
            .expect("union")
            .is_empty()
    );
}
