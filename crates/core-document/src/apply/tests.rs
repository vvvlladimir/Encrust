use core_edit::{Axis, Joints, Keep};
use core_geometry::{Mesh, Scalar, Vec3, signed_volume, transform_mesh};
use core_volume::{DrainHole, HoleShape};

use super::*;
use crate::fixtures::{SIDE_MM, cube};

/// What f32 sums over a cube's cut faces drift by, mm³.
const VOLUME_MM3: Scalar = 1e-2;
const CUBE: ObjectId = ObjectId(1);

fn cut_z(at_mm: Scalar, keep: Keep, lay_flat: bool) -> Edit {
    cut_of(CUBE, at_mm, keep, lay_flat)
}

fn cut_of(object: ObjectId, at_mm: Scalar, keep: Keep, lay_flat: bool) -> Edit {
    Edit::Cut {
        object,
        axis: Axis::Z,
        at_mm,
        keep,
        joints: Joints::default(),
        lay_flat,
    }
}

/// Each object's mesh where it stands on the plate.
fn placed(document: &Document, ids: &[(ObjectId, Role)]) -> Vec<Mesh> {
    ids.iter()
        .map(|(id, _)| {
            let (state, meshes) = document.object(*id).expect("an added object is there");
            transform_mesh(&meshes.source, state.transform)
        })
        .collect()
}

#[test]
fn a_cube_cut_through_its_middle_leaves_two_halves_of_half_its_volume() {
    let mut document = Document::from_project(cube());

    let applied = apply(&mut document, &cut_z(SIDE_MM / 2.0, Keep::Both, false))
        .expect("the plane crosses the cube");

    let half = SIDE_MM.powi(3) / 2.0;
    for mesh in placed(&document, &applied.added) {
        let volume = signed_volume(&mesh);
        assert!(
            (volume - half).abs() < VOLUME_MM3,
            "each half holds {half} mm³, got {volume}"
        );
    }
    let roles: Vec<Role> = applied.added.iter().map(|(_, role)| *role).collect();
    assert_eq!(roles, [Role::Lower, Role::Upper]);
}

#[test]
fn the_cut_cube_is_gone_and_its_halves_take_new_ids_and_its_name() {
    let mut document = Document::from_project(cube());

    let applied = apply(&mut document, &cut_z(5.0, Keep::Both, false)).expect("it cuts");

    assert_eq!(applied.removed, [CUBE]);
    assert!(document.object(CUBE).is_none());
    let ids: Vec<ObjectId> = applied.added.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [ObjectId(2), ObjectId(3)]);
    let (lower, _) = document
        .object(ObjectId(2))
        .expect("the lower half is there");
    assert_eq!(lower.name, "cube (lower)");
}

#[test]
fn an_upper_half_laid_flat_stands_on_its_cut_face_at_its_own_height() {
    let mut document = Document::from_project(cube());

    let applied =
        apply(&mut document, &cut_z(7.0, Keep::Above, true)).expect("the plane crosses it");

    assert_eq!(applied.added.len(), 1);
    assert_eq!(applied.added[0].1, Role::Kept);
    let mesh = &placed(&document, &applied.added)[0];
    let bounds = mesh.aabb().expect("the half has faces");
    assert!(
        bounds.mins.z.abs() < 1e-4 && (bounds.maxs.z - 3.0).abs() < 1e-4,
        "the 3 mm cap stands on the plate from 0 to 3, got {} to {}",
        bounds.mins.z,
        bounds.maxs.z
    );
}

#[test]
fn a_plane_that_misses_the_object_leaves_the_plate_as_it_was() {
    let mut document = Document::from_project(cube());

    let error = apply(&mut document, &cut_z(SIDE_MM + 5.0, Keep::Both, false))
        .expect_err("the plane is above the cube");

    assert_eq!(error, ApplyError::Missed(CUBE));
    assert!(document.object(CUBE).is_some());
}

#[test]
fn an_edit_naming_no_object_on_the_plate_is_refused() {
    let mut document = Document::from_project(cube());
    let edit = cut_of(ObjectId(9), 5.0, Keep::Both, false);

    let error = apply(&mut document, &edit).expect_err("there is no object 9");

    assert_eq!(error, ApplyError::NoObject(ObjectId(9)));
}

#[test]
fn holes_that_would_leave_nothing_of_the_object_refuse_the_cut() {
    let mut project = cube();
    // A bore wider than the cube, through all of it, would take the whole cube away.
    project.manifest.objects[0].hollow.drains = vec![DrainHole {
        at: Vec3::new(SIDE_MM / 2.0, SIDE_MM / 2.0, SIDE_MM),
        axis: -Vec3::Z,
        diameter_mm: 4.0 * SIDE_MM,
        depth_mm: 4.0 * SIDE_MM,
        taper: 1.0,
        lift_mm: 1.0,
        shape: HoleShape::default(),
        turn_deg: 0.0,
        kind: core_volume::HoleKind::default(),
    }];
    let mut document = Document::from_project(project);

    let error = apply(&mut document, &cut_z(SIDE_MM / 2.0, Keep::Both, false))
        .expect_err("nothing of the cube is left to cut");

    assert_eq!(error, ApplyError::Undrilled(CUBE));
}
