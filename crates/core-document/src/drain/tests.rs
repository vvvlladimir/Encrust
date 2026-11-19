use std::sync::Arc;

use core_edit::Edit;
use core_engine::project::load;
use core_geometry::Mesh;
use core_volume::Cancel;

use super::*;
use crate::apply::apply;
use crate::fixtures::cube;

const CUBE: ObjectId = ObjectId(1);

fn drain_at(at_mm: Vec3) -> Edit {
    Edit::Drain {
        object: CUBE,
        at_mm: vec![at_mm],
        size: HoleSize::default(),
        kind: HoleKind::default(),
    }
}

#[test]
fn a_point_over_the_top_face_drills_a_hole_down_into_it() {
    let mut document = Document::from_project(cube());

    let applied = apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect("a point over the top face drains");

    assert_eq!(applied.report, Report::Drained { holes: 1 });
    let (state, _) = document.object(CUBE).expect("the cube stays");
    let [hole] = state.hollow.drains[..] else {
        panic!("one hole was drilled, got {:?}", state.hollow.drains);
    };
    assert!(
        hole.at.abs_diff_eq(Vec3::new(5.0, 5.0, 10.0), 1e-3),
        "the mouth is at (5, 5, 10) within 1e-3, got {}",
        hole.at
    );
    let axis = hole.axis.normalize();
    assert!(
        axis.abs_diff_eq(Vec3::NEG_Z, 1e-4),
        "the axis points down (0, 0, -1) within 1e-4, got {axis}"
    );
}

#[test]
fn a_point_over_a_corner_is_off_a_face() {
    let mut document = Document::from_project(cube());

    let error = apply(
        &mut document,
        &drain_at(Vec3::new(10.5, 10.5, 10.5)),
        Cancel::never(),
    )
    .expect_err("a corner is no face to drill into");

    assert!(matches!(error, ApplyError::OffAFace { object, .. } if object == CUBE));
    let (state, _) = document.object(CUBE).expect("the cube stays");
    assert!(
        state.hollow.drains.is_empty(),
        "the plate is left as it was"
    );
}

#[test]
fn a_model_with_no_faces_has_no_surface_to_drain() {
    let mut project = cube();
    project.models[0].source = Arc::new(Mesh::default());
    let mut document = Document::from_project(project);

    let error = apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect_err("nothing to drill into");

    assert_eq!(error, ApplyError::NoSurface(CUBE));
}

#[test]
fn a_drained_plate_written_and_read_back_keeps_the_hole() {
    let mut document = Document::from_project(cube());
    apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect("the cube drains");
    let path = std::env::temp_dir().join(format!("{}-drained.encrust", std::process::id()));

    core_engine::project::save(&path, &document.into_project()).expect("the plate is written");
    let read = load(&path).expect("the plate reads back");
    std::fs::remove_file(&path).expect("the file is there to remove");

    let [hole] = read.manifest.objects[0].hollow.drains[..] else {
        panic!(
            "one hole read back, got {:?}",
            read.manifest.objects[0].hollow.drains
        );
    };
    assert!(
        hole.at.abs_diff_eq(Vec3::new(5.0, 5.0, 10.0), 1e-3),
        "the mouth is at (5, 5, 10) within 1e-3, got {}",
        hole.at
    );
}

#[test]
fn a_transform_that_cannot_be_inverted_is_bad_and_leaves_the_cube() {
    let mut project = cube();
    project.manifest.objects[0].transform.scale = Vec3::new(0.0, 1.0, 1.0);
    let mut document = Document::from_project(project);

    let error = apply(
        &mut document,
        &drain_at(Vec3::new(5.0, 5.0, 10.5)),
        Cancel::never(),
    )
    .expect_err("a flat object has no inverse");

    assert_eq!(error, ApplyError::BadTransform(CUBE));
}
