//! A model becomes the parts it is made of. What has to hold is that bodies meeting only
//! along an edge are two, that a cavity stays with its body, that scraps are joined or
//! dropped rather than handed back as parts, and that a part marked by a point is cut off
//! flat at its narrow join and nowhere else.
#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use core_csg::{CsgError, Solid};
use core_geometry::{Mesh, Scalar, Vec3, cuboid, diagnose, signed_volume};
use core_volume::{Cancel, PartSettings, PartedOff, VolumeError, part_off, parts};

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
    let split = parts(&mesh, &PartSettings::default());
    assert_eq!(split.parts.len(), 2, "four faces on one edge join nothing");
}

#[test]
fn a_cavity_stays_in_the_part_round_it() {
    let mut cavity = cuboid(Vec3::splat(2.0), Vec3::splat(8.0));
    for face in &mut cavity.faces {
        face.swap(1, 2);
    }
    let mesh = whole(&[cuboid(Vec3::ZERO, Vec3::splat(10.0)), cavity]);
    let split = parts(&mesh, &PartSettings::default());
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
    let split = parts(&mesh, &PartSettings::default());
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
    let split = parts(&mesh, &PartSettings::default());
    assert_eq!(split.parts.len(), 2, "a small model is not thrown away");
}

fn solid(mesh: &Mesh) -> Solid {
    Solid::new(mesh).expect("a box is a solid")
}

/// Two 10 mm boxes 10 mm apart along X, joined by a bar `bar_mm` square.
fn dumbbell(bar_mm: Scalar) -> Mesh {
    let (low, high) = (5.0 - bar_mm / 2.0, 5.0 + bar_mm / 2.0);
    let bar = cuboid(Vec3::new(10.0, low, low), Vec3::new(20.0, high, high));
    solid(&cuboid(Vec3::ZERO, Vec3::splat(10.0)))
        .union(&solid(&bar))
        .and_then(|joined| {
            joined.union(&solid(&cuboid(
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(30.0, 10.0, 10.0),
            )))
        })
        .expect("three boxes in a row join")
        .mesh()
}

/// The dumbbell with a 4 mm bar, hollowed to walls 1 mm thick everywhere: the thin-walled
/// shape a scan is.
fn hollow_dumbbell() -> Mesh {
    let cavities = solid(&cuboid(Vec3::splat(1.0), Vec3::splat(9.0)))
        .union(&solid(&cuboid(
            Vec3::new(8.5, 4.0, 4.0),
            Vec3::new(21.5, 6.0, 6.0),
        )))
        .and_then(|joined| {
            joined.union(&solid(&cuboid(
                Vec3::new(21.0, 1.0, 1.0),
                Vec3::new(29.0, 9.0, 9.0),
            )))
        })
        .expect("three cavities in a row join");
    solid(&dumbbell(4.0))
        .difference(&cavities)
        .expect("the cavities come out of the dumbbell")
        .mesh()
}

/// What of `mesh` past the left box faces +X: its area, mm², and the least and most X
/// it stands at.
fn facing_the_bar(mesh: &Mesh) -> (Scalar, Scalar, Scalar) {
    mesh.faces
        .iter()
        .map(|face| face.map(|corner| mesh.vertices[corner as usize]))
        .filter(|[a, b, c]| (a.x + b.x + c.x) / 3.0 > 10.2)
        .map(|[a, b, c]| {
            (
                (b - a).cross(c - a) / 2.0,
                a.x.min(b.x).min(c.x),
                a.x.max(b.x).max(c.x),
            )
        })
        .filter(|(area, _, _)| area.normalize_or_zero().x > 0.999)
        .fold(
            (0.0, Scalar::MAX, Scalar::MIN),
            |(total, low, high), (area, least, most)| {
                (total + area.length(), low.min(least), high.max(most))
            },
        )
}

/// Checks that both sides are closed, that nothing was lost or added, that the part
/// holds the left box and at most the whole bar, and that its cut is one flat face
/// square to the bar, `across_mm2` in area.
fn assert_parted_across_the_bar(
    parted: &PartedOff,
    model: &Mesh,
    left_mm3: Scalar,
    bar_mm3: Scalar,
    across_mm2: Scalar,
) {
    for side in [&parted.part, &parted.rest] {
        assert_eq!(diagnose(side).boundary_edges, 0, "each side is closed");
    }
    let (part, rest) = (signed_volume(&parted.part), signed_volume(&parted.rest));
    near(
        part + rest,
        signed_volume(model),
        1e-3,
        "parting loses and adds nothing",
    );
    assert!(
        part > left_mm3 && part < left_mm3 + bar_mm3,
        "the part is the left box and some of the bar, {left_mm3} to {}, got {part}",
        left_mm3 + bar_mm3
    );
    let (area, low, high) = facing_the_bar(&parted.part);
    near(
        area,
        across_mm2,
        1e-3,
        "the cut face is the bar's cross-section",
    );
    assert!(
        high - low < 1e-3,
        "the cut face is flat, spanning {low} to {high} mm"
    );
}

#[test]
fn a_part_marked_on_a_dumbbell_is_cut_off_flat_across_its_bar() {
    let model = dumbbell(2.0);
    let parted =
        part_off(&model, Vec3::new(0.0, 5.0, 5.0), Cancel::never()).expect("the bar is a neck");
    // A 10 mm box is 1000 mm³ and the 2 mm bar's whole 10 mm is 40 mm³.
    assert_parted_across_the_bar(&parted, &model, 1000.0, 40.0, 4.0);
}

#[test]
fn a_part_marked_on_a_thin_walled_dumbbell_is_cut_off_at_its_bar() {
    let model = hollow_dumbbell();
    let parted =
        part_off(&model, Vec3::new(0.0, 5.0, 5.0), Cancel::never()).expect("the bar is a neck");
    // A 10 mm box less its 8 mm cavity is 488 mm³, less the 4 mm² the bar opens through
    // its 1 mm face; the bar's wall is 16 - 4 = 12 mm² across, over 10 mm.
    assert_parted_across_the_bar(&parted, &model, 484.0, 124.0, 12.0);
}

#[test]
fn a_part_no_material_joins_to_the_rest_is_cut_out_whole() {
    // A 4 mm box half a millimetre off a 10 mm one: no wave crosses the gap, at any lattice.
    let small = cuboid(Vec3::new(10.5, 3.0, 3.0), Vec3::new(14.5, 7.0, 7.0));
    let model = whole(&[cuboid(Vec3::ZERO, Vec3::splat(10.0)), small]);
    let parted = part_off(&model, Vec3::new(14.5, 5.0, 5.0), Cancel::never())
        .expect("the small box is a piece of its own");
    for side in [&parted.part, &parted.rest] {
        assert_eq!(diagnose(side).boundary_edges, 0, "each side is closed");
    }
    near(
        signed_volume(&parted.part),
        64.0,
        1e-4,
        "the 4 mm box, untouched",
    );
    near(
        signed_volume(&parted.rest),
        1000.0,
        1e-4,
        "the 10 mm box, untouched",
    );
}

#[test]
fn a_box_has_no_neck_to_part_at() {
    let model = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let error =
        part_off(&model, Vec3::new(0.0, 5.0, 5.0), Cancel::never()).expect_err("a box is one body");
    assert_eq!(error, VolumeError::NoNeck);
}

#[test]
fn a_point_off_the_model_parts_nothing() {
    let error = part_off(&dumbbell(2.0), Vec3::new(50.0, 5.0, 5.0), Cancel::never())
        .expect_err("the point is 20 mm past the model");
    assert_eq!(error, VolumeError::NotOnModel);
}

#[test]
fn a_part_off_given_up_on_is_cancelled() {
    let stop = || true;
    let error = part_off(
        &dumbbell(2.0),
        Vec3::new(0.0, 5.0, 5.0),
        Cancel::when(&stop),
    )
    .expect_err("the run was stopped");
    assert_eq!(error, VolumeError::Cancelled);
}

#[test]
fn a_model_that_is_no_solid_cannot_be_parted() {
    let mut open = dumbbell(2.0);
    open.faces.truncate(open.faces.len() - 1);
    let error =
        part_off(&open, Vec3::new(0.0, 5.0, 5.0), Cancel::never()).expect_err("a face is missing");
    assert!(
        matches!(error, VolumeError::Csg(CsgError::NotSolid(_))),
        "got {error:?}"
    );
}
