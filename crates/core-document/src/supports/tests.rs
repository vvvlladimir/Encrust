use core_edit::Edit;
use core_engine::project::{ObjectSupportState, Project, load};
use core_geometry::Vec3;
use core_supports::SupportTree;

use super::*;
use crate::apply::apply;
use crate::fixtures::cube;

const CUBE: ObjectId = ObjectId(1);

fn supports(profile: Option<&str>, at_mm: Vec<Vec3>, only_at: bool) -> Edit {
    Edit::Supports {
        object: CUBE,
        profile: profile.map(str::to_owned),
        at_mm,
        only_at,
    }
}

fn bare_cube() -> Project {
    let mut project = cube();
    project.manifest.objects[0].supports = ObjectSupportState::default();
    project
}

fn lifted_cube(lift_mm: f32) -> Document {
    let mut project = bare_cube();
    project.manifest.objects[0].transform.translation = Vec3::new(0.0, 0.0, lift_mm);
    Document::from_project(project)
}

fn tip_contacts(document: &Document) -> Vec<Vec3> {
    let (state, _) = document.object(CUBE).expect("the cube stays");
    let matrix = state.transform.to_matrix();
    state
        .supports
        .grown
        .iter()
        .flat_map(SupportTree::contacts)
        .map(|contact| matrix.transform_point3(contact))
        .collect()
}

#[test]
fn a_cube_lifted_off_the_plate_gets_trees_on_its_underside() {
    let mut document = lifted_cube(5.0);

    let applied = apply(
        &mut document,
        &supports(Some("medium"), Vec::new(), false),
        Cancel::never(),
    )
    .expect("a lifted cube is supported");

    let tips = tip_contacts(&document);
    assert!(!tips.is_empty(), "grown trees stand: {applied:?}");
    for tip in &tips {
        assert!(
            (tip.z - 5.0).abs() < 1e-3,
            "every tip's contact is on the underside at z 5, got {tip}"
        );
    }
}

#[test]
fn a_cube_standing_on_the_plate_needs_no_supports() {
    let mut document = Document::from_project(bare_cube());

    apply(
        &mut document,
        &supports(Some("medium"), Vec::new(), false),
        Cancel::never(),
    )
    .expect("a standing cube is supported with nothing");

    assert!(tip_contacts(&document).is_empty(), "nothing needs holding");
}

#[test]
fn a_point_alone_gets_one_contact_on_the_nearest_surface() {
    let mut document = lifted_cube(5.0);

    apply(
        &mut document,
        &supports(Some("medium"), vec![Vec3::new(5.0, 5.0, 4.0)], true),
        Cancel::never(),
    )
    .expect("a point under the cube is supported");

    let (state, _) = document.object(CUBE).expect("the cube stays");
    let [point] = &state.supports.points[..] else {
        panic!("one point placed, got {:?}", state.supports.points);
    };
    let at = point.contact + Vec3::new(0.0, 0.0, 5.0);
    assert!(
        at.abs_diff_eq(Vec3::new(5.0, 5.0, 5.0), 1e-3),
        "the contact is at (5, 5, 5) within 1e-3, got {at}"
    );
}

#[test]
fn the_automatic_profile_is_not_a_preset_here() {
    let mut document = lifted_cube(5.0);

    let error = apply(
        &mut document,
        &supports(Some("auto"), Vec::new(), false),
        Cancel::never(),
    )
    .expect_err("auto is sized by the window's catalogue");

    assert_eq!(error, ApplyError::NoProfile("auto".to_owned()));
}

#[test]
fn supports_written_and_read_back_keep_their_trees() {
    let mut document = lifted_cube(5.0);
    apply(
        &mut document,
        &supports(Some("medium"), Vec::new(), false),
        Cancel::never(),
    )
    .expect("a lifted cube is supported");
    let tips = tip_contacts(&document).len();
    assert!(tips > 0, "the lifted cube has grown trees to keep");
    let path = std::env::temp_dir().join(format!("{}-supported.encrust", std::process::id()));

    core_engine::project::save(&path, &document.into_project()).expect("the plate is written");
    let read = load(&path).expect("the plate reads back");
    std::fs::remove_file(&path).expect("the file is there to remove");

    let kept: usize = read.manifest.objects[0]
        .supports
        .grown
        .iter()
        .map(|tree| tree.contacts().count())
        .sum();
    assert_eq!(kept, tips, "the trees come back with every tip");
}

#[test]
fn a_run_already_stopped_is_cancelled_and_leaves_the_plate() {
    let mut document = lifted_cube(5.0);
    let stopped = || true;

    let error = apply(
        &mut document,
        &supports(Some("medium"), Vec::new(), false),
        Cancel::when(&stopped),
    )
    .expect_err("a stopped run lands nothing");

    assert_eq!(error, ApplyError::Cancelled);
    let (state, _) = document.object(CUBE).expect("the cube stays");
    assert!(state.supports.points.is_empty(), "no point was kept");
}

#[test]
fn supports_for_an_object_not_on_the_plate_name_it() {
    let mut document = lifted_cube(5.0);
    let edit = Edit::Supports {
        object: ObjectId(99),
        profile: None,
        at_mm: Vec::new(),
        only_at: false,
    };

    let error = apply(&mut document, &edit, Cancel::never()).expect_err("no such object");

    assert_eq!(error, ApplyError::NoObject(ObjectId(99)));
}

#[test]
fn an_active_group_missing_from_the_table_falls_back_to_the_first() {
    let mut project = bare_cube();
    project.manifest.supports.active = 99;
    project.manifest.objects[0].transform.translation = Vec3::new(0.0, 0.0, 5.0);
    let mut document = Document::from_project(project);

    apply(
        &mut document,
        &supports(None, vec![Vec3::new(5.0, 5.0, 4.0)], true),
        Cancel::never(),
    )
    .expect("the first group takes the point");

    let (state, _) = document.object(CUBE).expect("the cube stays");
    assert_eq!(state.supports.points[0].group, 0);
}
