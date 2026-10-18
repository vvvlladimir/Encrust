//! A model becomes the parts it is made of. What has to hold is that bodies meeting only
//! along an edge are two, that a cavity stays with its body, that scraps are joined or
//! dropped rather than handed back as parts, and that a narrow join is parted.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_geometry::{Mesh, Scalar, Vec3, cuboid, diagnose, signed_volume};
use core_volume::{Cancel, PartSettings, VolumeError, parts};

fn whole(meshes: &[Mesh]) -> Mesh {
    let mut whole = Mesh::default();
    for mesh in meshes {
        whole.append(mesh);
    }
    whole
}

fn volumes(meshes: &[Mesh]) -> Vec<Scalar> {
    meshes
        .iter()
        .map(|mesh| signed_volume(mesh).abs())
        .collect()
}

fn near(actual: Scalar, expected: Scalar, share: Scalar, why: &str) {
    assert!(
        (actual - expected).abs() <= share * expected,
        "{why}: expected {expected}, got {actual}"
    );
}

#[test]
fn boxes_meeting_only_along_an_edge_are_two_parts() {
    let mesh = whole(&[
        cuboid(Vec3::ZERO, Vec3::splat(10.0)),
        cuboid(Vec3::new(10.0, 10.0, 0.0), Vec3::new(20.0, 20.0, 10.0)),
    ]);
    let split = parts(&mesh, &PartSettings::default(), Cancel::never()).expect("boxes split");
    assert_eq!(split.parts.len(), 2, "four faces on one edge join nothing");
}

#[test]
fn a_cavity_stays_in_the_part_round_it() {
    let mut cavity = cuboid(Vec3::splat(2.0), Vec3::splat(8.0));
    for face in &mut cavity.faces {
        face.swap(1, 2);
    }
    let mesh = whole(&[cuboid(Vec3::ZERO, Vec3::splat(10.0)), cavity]);
    let split = parts(&mesh, &PartSettings::default(), Cancel::never()).expect("a shell splits");
    assert_eq!(split.parts.len(), 1);
    near(
        volumes(&split.parts)[0],
        784.0,
        1e-4,
        "10³ less the 6³ cavity",
    );
}

#[test]
fn a_scrap_touching_a_part_is_joined_to_it_and_a_stray_one_dropped() {
    let mesh = whole(&[
        cuboid(Vec3::ZERO, Vec3::splat(10.0)),
        // Half a cubic millimetre sitting on the top face, and one hanging in the air.
        cuboid(Vec3::new(4.0, 4.0, 10.0), Vec3::new(5.0, 5.0, 10.5)),
        cuboid(Vec3::new(30.0, 0.0, 0.0), Vec3::new(30.5, 0.5, 0.5)),
    ]);
    let split = parts(&mesh, &PartSettings::default(), Cancel::never()).expect("scraps split");
    assert_eq!(split.parts.len(), 1, "neither scrap is a part");
    assert_eq!((split.joined, split.dropped), (1, 1));
    near(
        volumes(&split.parts)[0],
        1000.5,
        1e-4,
        "the box and the scrap on it",
    );
}

#[test]
fn a_model_of_nothing_but_scraps_keeps_them_all() {
    let mesh = whole(&[
        cuboid(Vec3::ZERO, Vec3::splat(0.5)),
        cuboid(Vec3::new(2.0, 0.0, 0.0), Vec3::new(2.5, 0.5, 0.5)),
    ]);
    let split = parts(&mesh, &PartSettings::default(), Cancel::never()).expect("scraps split");
    assert_eq!(split.parts.len(), 2, "a small model is not thrown away");
}

/// Two 10 mm boxes 10 mm apart, joined by a bar `bar_mm` square.
fn dumbbell(bar_mm: Scalar) -> Mesh {
    let (low, high) = (5.0 - bar_mm / 2.0, 5.0 + bar_mm / 2.0);
    let left = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let right = cuboid(Vec3::new(20.0, 0.0, 0.0), Vec3::new(30.0, 10.0, 10.0));
    let bar = cuboid(Vec3::new(10.0, low, low), Vec3::new(20.0, high, high));
    let solid = |mesh: &Mesh| core_csg::Solid::new(mesh).expect("a box is a solid");
    solid(&left)
        .union(&solid(&bar))
        .and_then(|joined| joined.union(&solid(&right)))
        .expect("three boxes in a row join")
        .mesh()
}

fn at_necks(neck_mm: Scalar) -> PartSettings {
    PartSettings {
        at_necks: true,
        neck_mm,
        ..PartSettings::default()
    }
}

#[test]
fn a_bar_narrower_than_the_neck_is_parted_and_each_side_keeps_its_box() {
    let split = parts(&dumbbell(0.6), &at_necks(1.5), Cancel::never()).expect("a dumbbell parts");
    assert_eq!((split.parts.len(), split.necks), (2, 1));
    for (part, volume) in split.parts.iter().zip(volumes(&split.parts)) {
        assert_eq!(diagnose(part).boundary_edges, 0, "a part is closed");
        // A 10 mm box and some of the 0.6 mm bar, whose whole 10 mm is 3.6 mm³.
        near(volume, 1001.8, 0.01, "a box and up to the bar's length");
    }
    let total: Scalar = volumes(&split.parts).iter().sum();
    near(total, 2003.6, 1e-3, "parting loses and adds nothing");
}

#[test]
fn a_bar_wider_than_the_neck_holds_the_boxes_together() {
    let split = parts(&dumbbell(3.0), &at_necks(1.5), Cancel::never()).expect("a dumbbell parts");
    assert_eq!((split.parts.len(), split.necks), (1, 0));
}

#[test]
fn a_neck_search_given_up_on_is_cancelled() {
    let stop = || true;
    let error = parts(&dumbbell(0.6), &at_necks(1.5), Cancel::when(&stop))
        .expect_err("the run was stopped");
    assert_eq!(error, VolumeError::Cancelled);
}
