use core_edit::Edit;
use core_engine::project::ObjectSupportState;
use core_geometry::Vec3;
use core_volume::Cancel;

use super::*;
use crate::apply::apply;
use crate::fixtures::cube;

const CUBE: ObjectId = ObjectId(1);

/// The 10 mm cube 5 mm over the plate on medium supports, and where its contacts touch it.
fn supported_cube() -> (Document, Vec<Vec3>) {
    let mut project = cube();
    project.manifest.objects[0].supports = ObjectSupportState::default();
    project.manifest.objects[0].transform.translation = Vec3::new(0.0, 0.0, 5.0);
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
    let applied = apply(&mut document, &edit, Cancel::never()).expect("a lifted cube is held up");
    let Report::Supported { contacts_mm, .. } = applied.report else {
        panic!("supports report their contacts: {applied:?}");
    };
    (document, contacts_mm)
}

fn merge(near_mm: Vec<Vec3>) -> Edit {
    Edit::MergeSupports {
        object: CUBE,
        near_mm,
    }
}

fn trees_on_the_plate(document: &Document) -> Vec<SupportTree> {
    let (state, meshes) = document.object(CUBE).expect("the cube stays");
    let plate = document.support_state();
    let table = table_of(state, &plate.groups, plate.raft);
    let surface = meshes.printed();
    let bvh = Arc::new(Bvh::build(surface));
    let (source, drains) = (&meshes.source, &state.hollow.drains);
    supports_of(
        &state.supports,
        drains,
        source,
        surface,
        &bvh,
        state.transform,
        &table,
    )
    .trees()
    .to_vec()
}

/// A 40 x 10 x 2 mm slab 20 mm over the plate held up at both ends alone, as the engine's own
/// merge is proved on, and its two contacts.
fn slab_on_two_supports() -> (Document, [Vec3; 2]) {
    let mut project = cube();
    let object = &mut project.manifest.objects[0];
    object.supports = ObjectSupportState::default();
    object.transform.scale = Vec3::new(4.0, 1.0, 0.2);
    object.transform.translation = Vec3::new(0.0, -5.0, 20.0);
    let mut document = Document::from_project(project);
    let ends = [Vec3::new(5.0, 0.0, 20.0), Vec3::new(35.0, 0.0, 20.0)];
    let edit = Edit::Supports {
        object: CUBE,
        profile: Some("medium".to_owned()),
        at_mm: ends.to_vec(),
        only_at: true,
        cavity: false,
        shown_at: Vec::new(),
        clean_surface: false,
    };
    apply(&mut document, &edit, Cancel::never()).expect("the slab's ends are held up");
    (document, ends)
}

#[test]
fn two_supports_named_by_their_contacts_become_one_tree_carrying_both() {
    let (mut document, [one, other]) = slab_on_two_supports();

    let applied = apply(&mut document, &merge(vec![one, other]), Cancel::never())
        .expect("two supports under a slab join");

    assert_eq!(applied.report, Report::Merged);
    let trees = trees_on_the_plate(&document);
    let carrying = |at: Vec3| support_near(&trees, at).expect("each contact is still carried");
    assert_eq!(
        carrying(one),
        carrying(other),
        "one tree carries both {one} and {other}"
    );
    assert_eq!(trees.len(), 1, "the two supports are one");
}

#[test]
fn a_point_near_no_contact_names_no_support() {
    let (mut document, contacts) = supported_cube();
    let far = Vec3::new(100.0, 100.0, 100.0);

    let refused = apply(
        &mut document,
        &merge(vec![contacts[0], far]),
        Cancel::never(),
    );

    assert!(
        matches!(refused, Err(ApplyError::NoSupportNear { at_mm, .. }) if at_mm == far),
        "{refused:?}"
    );
}

#[test]
fn points_naming_one_support_are_no_merge() {
    let (mut document, contacts) = supported_cube();

    let refused = apply(
        &mut document,
        &merge(vec![contacts[0], contacts[0]]),
        Cancel::never(),
    );

    assert!(
        matches!(refused, Err(ApplyError::OneSupportToMerge { .. })),
        "{refused:?}"
    );
}

#[test]
fn supports_the_cube_stands_in_the_way_of_are_refused_with_why() {
    let (mut document, contacts) = supported_cube();
    let (corner, next) = (contacts[0], contacts[1]);

    let refused = apply(&mut document, &merge(vec![corner, next]), Cancel::never());

    assert!(
        matches!(
            refused,
            Err(ApplyError::MergeRefused {
                why: core_supports::Refused::Collides,
                ..
            })
        ),
        "the corner's two supports would join through the cube: {refused:?}"
    );
}
