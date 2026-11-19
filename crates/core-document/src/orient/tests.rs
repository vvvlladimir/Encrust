use std::sync::Arc;

use core_edit::{Edit, ObjectId, OrientAim};
use core_geometry::{Vec3, cuboid, transform_mesh};
use core_volume::Cancel;

use super::*;
use crate::apply::apply;
use crate::fixtures::cube;

/// What the stood height may exceed the short side by, mm.
const HEIGHT_TOL_MM: f32 = 1e-3;

/// A plate of one 10 x 10 x 30 mm bar standing on its end.
fn standing_bar() -> Document {
    let mut project = cube();
    project.models[0].source = Arc::new(cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 30.0)));
    Document::from_project(project)
}

fn orient(goal: OrientAim, pick: usize) -> Edit {
    Edit::Orient {
        object: ObjectId(1),
        goal,
        max_tilt_deg: None,
        clean_surface: false,
        pick,
    }
}

fn height_mm(document: &Document) -> f32 {
    let (state, meshes) = document
        .object(ObjectId(1))
        .expect("the bar is on the plate");
    let bounds = transform_mesh(&meshes.source, state.transform)
        .aabb()
        .expect("a bar has faces");
    bounds.maxs.z - bounds.mins.z
}

#[test]
fn a_bar_standing_on_its_end_is_laid_with_its_length_along_the_plate_for_the_shortest_print() {
    let mut document = standing_bar();
    let edit = orient(OrientAim::ShortestPrint, 0);
    assert!(height_mm(&document) > 29.0, "the bar starts 30 mm tall");

    let applied = apply(&mut document, &edit, Cancel::never()).expect("a bar can be oriented");

    assert!(matches!(applied.report, Report::Oriented { .. }));
    let height = height_mm(&document);
    assert!(
        height <= 10.0 + HEIGHT_TOL_MM,
        "the bar's 30 mm lies along the plate, so it stands 10 mm, got {height}"
    );
}

#[test]
fn a_pick_past_the_poses_found_is_no_choice() {
    let mut document = standing_bar();
    let edit = orient(OrientAim::ShortestPrint, 10_000);

    let error = apply(&mut document, &edit, Cancel::never()).expect_err("no such pose");

    assert!(matches!(
        error,
        ApplyError::NoChoice { object: ObjectId(1), pick: 10_000, found } if found > 0
    ));
}

#[test]
fn a_cancelled_search_is_cancelled_and_leaves_the_bar_as_it_stood() {
    let mut document = standing_bar();
    let before = document
        .object(ObjectId(1))
        .expect("on the plate")
        .0
        .transform;
    let stop = || true;

    let error = apply(
        &mut document,
        &orient(OrientAim::Auto, 0),
        Cancel::when(&stop),
    )
    .expect_err("the search is stopped");

    assert_eq!(error, ApplyError::Cancelled);
    let after = document
        .object(ObjectId(1))
        .expect("on the plate")
        .0
        .transform;
    assert_eq!(after, before);
}
