use core_edit::Edit;
use core_engine::project::ObjectSupportState;
use core_geometry::Vec3;
use core_volume::Cancel;
use printer_profiles::SupportProfile;

use super::*;
use crate::apply::apply;
use crate::fixtures::cube;

const CUBE: ObjectId = ObjectId(1);

/// The 10 mm cube 5 mm over the plate, on medium supports where `rafted` stands them on a raft.
fn lifted_cube(rafted: bool) -> Document {
    let mut project = cube();
    project.manifest.objects[0].supports = ObjectSupportState::default();
    project.manifest.objects[0].transform.translation = Vec3::new(0.0, 0.0, 5.0);
    let mut raft = SupportProfile::shipped("medium")
        .expect("medium ships")
        .raft;
    raft.enabled = rafted;
    project.manifest.supports.raft = Some(raft);
    let mut document = Document::from_project(project);
    let edit = Edit::Supports {
        object: CUBE,
        profile: Some("medium".to_owned()),
        at_mm: Vec::new(),
        only_at: false,
        cavity: false,
        shown_at: Vec::new(),
        clean_surface: false,
    };
    apply(&mut document, &edit, Cancel::never()).expect("a lifted cube is held up");
    document
}

fn into_the_raft(at_mm: Vec3) -> Edit {
    Edit::Drain {
        object: CUBE,
        at_mm: vec![at_mm],
        size: HoleSize::default(),
        kind: HoleKind::Support,
        auto: false,
        vent: false,
    }
}

#[test]
fn a_hole_into_the_raft_is_kept_with_the_objects_holes() {
    let mut document = lifted_cube(true);

    apply(
        &mut document,
        &into_the_raft(Vec3::new(5.0, 5.0, -1.0)),
        Cancel::never(),
    )
    .expect("the raft under the cube takes a hole");

    let (state, _) = document.object(CUBE).expect("the cube stays");
    let kinds: Vec<HoleKind> = state.hollow.drains.iter().map(|hole| hole.kind).collect();
    assert_eq!(kinds, [HoleKind::Support], "one hole, into the supports");
}

#[test]
fn a_hole_into_a_raft_that_is_not_there_is_no_raft() {
    let mut document = lifted_cube(false);

    let refused = apply(
        &mut document,
        &into_the_raft(Vec3::new(5.0, 5.0, -1.0)),
        Cancel::never(),
    );

    assert_eq!(refused.map(|_| ()), Err(ApplyError::NoRaft(CUBE)));
}
