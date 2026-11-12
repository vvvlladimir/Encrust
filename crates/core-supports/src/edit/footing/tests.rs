use crate::edit::{Refused, allowed, carried, eased, fit};
use crate::fixtures::{box_mesh, profile, ramp, trees_on};
use crate::pick::Part;
use crate::placed::Placed;
use crate::tree::SupportTree;
use crate::{Profiles, SupportPoint};
use core_geometry::{Bvh, Mesh, Transform, Vec3};

/// A cube 0..10 standing on the plate under a slab 20..22 reaching 2 mm past it along x.
fn cube_under_a_slab() -> Mesh {
    let mut model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    model.append(&box_mesh(
        Vec3::new(0.0, 0.0, 20.0),
        Vec3::new(12.0, 10.0, 22.0),
    ));
    model
}

/// The one tree grown under `points` (model space) of `model`.
fn grown(model: &Mesh, points: &[SupportPoint]) -> SupportTree {
    trees_on(
        points,
        model,
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("the points have room under them")
}

#[test]
fn a_foot_carried_onto_a_face_growing_takes_as_a_last_resort_stands_on_it() {
    // The ramp at x = 0..4 leans 68 degrees: past what a run chooses, within its last resort.
    let mut model = ramp(4.0, 10.0);
    model.append(&box_mesh(
        Vec3::new(-10.0, -5.0, 20.0),
        Vec3::new(6.0, 5.0, 22.0),
    ));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = grown(&model, &[SupportPoint::new(Vec3::new(-6.0, 0.0, 20.0))]);
    assert!(!tree.landing().on_model, "grown in clear air, on the plate");

    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Foot],
        Vec3::X * 8.0,
        &placed,
        &profile(),
    )
    .expect("the ramp under the foot takes it as growing would");

    assert!(moved.landing().on_model, "it stands on the ramp at x = 2");
    assert_eq!(allowed(&tree, &moved, &placed, &profile()), Ok(()));
}

#[test]
fn a_foot_on_the_model_carried_off_its_edge_slides_back_onto_it() {
    let model = cube_under_a_slab();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = grown(&model, &[SupportPoint::new(Vec3::new(9.0, 5.0, 20.0))]);
    assert!(tree.landing().on_model, "grown onto the cube's top");

    // Straight under x = 11 is the plate; the cube's top is within reach beside it.
    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Foot],
        Vec3::X * 2.0,
        &placed,
        &profile(),
    )
    .expect("the cube's top is within reach");

    let base = moved.landing().base;
    assert!(moved.landing().on_model, "it stays on the cube, at {base}");
    assert!(
        base.x <= 10.0 && (base.z - 10.0).abs() < 1e-3,
        "on the cube's top at z = 10, got {base}"
    );
    let neck = tree.root_index();
    assert_eq!(
        moved.nodes()[neck].position,
        tree.nodes()[neck].position,
        "the neck the head hangs from stays, so the head stays square to its face"
    );
}

#[test]
fn a_root_on_the_model_is_lowered_to_make_room() {
    let model = cube_under_a_slab();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    // Merged in clear air, then stood on the cube's top under it.
    let mut tree = grown(
        &Mesh::default(),
        &[(3.0, 5.0), (6.0, 5.0)].map(|(x, y)| SupportPoint::new(Vec3::new(x, y, 20.0))),
    );
    let root = tree.root_index();
    let under = crate::landing(
        &placed,
        tree.root().position,
        tree.root().radius_mm,
        2,
        &profile(),
    )
    .expect("the cube's top is under the joint");
    tree.stand_on(under);
    let neck = (0..tree.nodes().len())
        .find(|node| tree.nodes()[*node].parent == Some(root))
        .expect("the root carries a neck");
    assert!(tree.landing().on_model, "the pair stands on the cube's top");
    let away = (tree.nodes()[neck].position - tree.root().position).with_z(0.0);
    // A millimetre turns the 2 mm head 27 degrees, within the 33 a head may turn.
    let step = away.normalize() * 1.0;
    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Node(neck)],
        step,
        &placed,
        &profile(),
    )
    .expect("a neck in clear air is carried");
    assert_eq!(
        allowed(&tree, &moved, &placed, &profile()),
        Err(Refused::Leans)
    );

    let eased = eased(&tree, &moved, &placed, &profile()).expect("the root may be lowered");

    let (was, now) = (tree.root().position, eased.root().position);
    assert!(
        now.z < was.z && now.truncate() == was.truncate(),
        "the root only went down, from {was} to {now}"
    );
    assert_eq!(
        eased.landing(),
        tree.landing(),
        "its foot stays where it was"
    );
}

#[test]
fn a_trunk_over_a_foot_with_no_room_beside_the_model_has_no_footing() {
    let model = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let mut tree = grown(&model, &[SupportPoint::new(Vec3::new(20.0, 5.0, 20.0))]);

    // The trunk at x = 13 clears the cube; a pad 8 mm across under it does not.
    let at = Vec3::new(13.0, 5.0, tree.root().position.z);
    tree.move_node(tree.root_index(), at);
    tree.move_landing(at.with_z(0.0));

    assert_eq!(fit(&tree, &placed, &profile()), Err(Refused::NoFooting));
}

#[test]
fn a_foot_dragged_over_an_edge_keeps_to_what_it_last_stood_on_while_in_reach() {
    let model = cube_under_a_slab();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = grown(&model, &[SupportPoint::new(Vec3::new(11.0, 5.0, 20.0))]);
    assert!(
        !tree.landing().on_model,
        "grown onto the plate beside the cube"
    );
    let on_top = crate::landing(&placed, Vec3::new(9.0, 5.0, 15.0), 0.6, 1, &profile())
        .expect("the cube's top is under x = 9");

    // Over the cube's edge nothing takes the foot; its top and the plate are both within 2 mm.
    let step = Vec3::X * -1.5;
    let from_plate = carried(
        &tree,
        tree.landing(),
        &[Part::Foot],
        step,
        &placed,
        &profile(),
    )
    .expect("the plate is in reach");
    let from_top = carried(&tree, on_top, &[Part::Foot], step, &placed, &profile())
        .expect("the cube's top is under it");

    assert!(
        !from_plate.landing().on_model,
        "last on the plate, it stays there"
    );
    assert!(
        from_top.landing().on_model,
        "last on the cube, it stays there"
    );
}
