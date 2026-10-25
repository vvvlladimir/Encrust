use super::*;
use crate::fixtures::{box_mesh, profile, trees_on};
use crate::{Profiles, SupportPoint};
use core_geometry::{Bvh, Mesh, Scalar, Transform};

/// A cube spanning 0..10 on every axis, standing on the plate.
fn cube() -> Mesh {
    box_mesh(Vec3::ZERO, Vec3::splat(10.0))
}

/// One support standing in clear air beside the cube, 20 mm up.
fn beside(model: &Mesh, x: Scalar) -> crate::SupportTree {
    trees_on(
        &[SupportPoint::new(Vec3::new(x, 5.0, 20.0))],
        model,
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("a column in clear air stands")
}

#[test]
fn a_support_standing_clear_of_the_model_fits() {
    let model = cube();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    assert!(fits(&beside(&model, 20.0), &placed, &profile()));
}

#[test]
fn a_support_carried_into_the_model_does_not_fit() {
    let model = cube();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());

    // Standing clear, then its foot dragged under the cube: the trunk now runs
    // through the solid.
    let mut tree = beside(&model, 20.0);
    tree.move_landing(Vec3::new(5.0, 5.0, 0.0));

    assert!(
        !fits(&tree, &placed, &profile()),
        "a trunk through the model is what the automatic run refuses too"
    );
}

#[test]
fn a_tip_is_pulled_onto_the_surface_it_holds() {
    let model = cube();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());

    let on = on_model(&placed, Vec3::new(5.0, 5.0, 14.0)).expect("the cube is under it");
    assert!(
        (on.z - 10.0).abs() < 1e-4 && (on.x - 5.0).abs() < 1e-4,
        "the nearest surface is the lid at z = 10, got {on}"
    );
}

#[test]
fn a_tip_is_not_pulled_onto_a_blocked_face() {
    let model = cube();
    let bvh = Bvh::build(&model);
    let mut region = core_geometry::Region::default();
    for face in 0..model.faces.len() {
        region.set(face, true);
    }
    let blocked =
        crate::Blocked::new(&model, &region, Transform::default()).expect("the cube was painted");
    let placed = Placed::new(&model, &bvh, Transform::default()).blocking(Some(&blocked));

    assert!(on_model(&placed, Vec3::new(5.0, 5.0, 14.0)).is_none());
}

/// A slab floating 10 mm over the plate, held up from below.
fn shelf() -> Mesh {
    box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0))
}

/// The one support under the middle of the shelf, touching its underside.
fn under_the_shelf(model: &Mesh) -> crate::SupportTree {
    trees_on(
        &[SupportPoint::new(Vec3::new(5.0, 5.0, 10.0))],
        model,
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("a column under the shelf stands")
}

/// Where the one tip of a single column sits in the tree.
fn tip_of(tree: &crate::SupportTree) -> usize {
    tree.nodes()
        .iter()
        .position(crate::TreeNode::is_leaf)
        .expect("a column has a tip")
}

#[test]
fn a_foot_carried_across_the_plate_stays_on_it() {
    let model = shelf();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = under_the_shelf(&model);

    let moved = carried(
        &tree,
        &[Part::Foot],
        Vec3::new(3.0, 0.0, 4.0),
        &placed,
        &profile(),
    )
    .expect("a foot slid along the plate under the shelf stands clear");

    let base = moved.landing().base;
    assert!(
        base.z.abs() < 1e-4,
        "a foot carried up still stands on the plate at z = 0, got {base}"
    );
    assert!(
        (base.x - (tree.landing().base.x + 3.0)).abs() < 1e-4,
        "the foot took the step across, got {base}"
    );
}

#[test]
fn a_tip_carried_off_the_model_is_pulled_back_onto_it() {
    let model = shelf();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = under_the_shelf(&model);
    let tip = tip_of(&tree);

    let moved = carried(
        &tree,
        &[Part::Node(tip)],
        Vec3::new(0.0, 0.0, -2.0),
        &placed,
        &profile(),
    )
    .expect("a tip pulled back onto the shelf holds it up again");

    let at = moved.nodes()[tip].position;
    assert!(
        (at.z - 10.0).abs() < 1e-4,
        "the nearest surface is the shelf's underside at z = 10, got {at}"
    );
}

#[test]
fn a_part_carried_into_the_model_is_refused() {
    let model = cube();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = beside(&model, 20.0);

    // The foot dragged 15 mm back lands under the cube, so the trunk would run
    // through the solid.
    assert!(
        carried(
            &tree,
            &[Part::Foot],
            Vec3::new(-15.0, 0.0, 0.0),
            &placed,
            &profile()
        )
        .is_none()
    );
}

/// The shelf with a slab on the plate under it, so a column from the shelf lands on
/// the model and not on the plate.
fn shelf_over_a_slab() -> Mesh {
    let mut model = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 2.0));
    let upper = shelf();
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&upper.vertices);
    model
        .faces
        .extend(upper.faces.iter().map(|face| face.map(|at| at + offset)));
    model
}

#[test]
fn a_support_standing_on_the_model_fits_and_can_be_carried() {
    let model = shelf_over_a_slab();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = under_the_shelf(&model);
    assert!(tree.landing().on_model, "the slab is under the column");

    assert!(
        fits(&tree, &placed, &profile()),
        "a trunk sunk into what it stands on is how it is built, not a collision"
    );
    let moved = carried(
        &tree,
        &[Part::Node(tree.root_index())],
        Vec3::new(1.0, 0.0, 0.0),
        &placed,
        &profile(),
    )
    .expect("a joint carried a millimetre through clear air stands");
    assert!(
        (moved.root().position.x - (tree.root().position.x + 1.0)).abs() < 1e-4,
        "the joint took the step"
    );
}

#[test]
fn a_support_stood_on_a_steep_face_as_a_last_resort_still_fits() {
    // The ramp of the landing's own test, 68 degrees, under a column grown in clear air.
    let model = crate::fixtures::ramp(4.0, 10.0);
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let mut tree = trees_on(
        &[SupportPoint::new(Vec3::new(2.0, 0.0, 20.0))],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("a column in clear air stands");
    let root = *tree.root();
    let steep = crate::column::landing_within(
        &placed,
        root.position,
        root.radius_mm,
        1,
        &profile(),
        crate::column::LAST_RESORT_SLOPE_DEG,
    )
    .expect("the ramp is under it");
    tree.stand_on(steep);

    assert!(
        fits(&tree, &placed, &profile()),
        "what the run stood there the hand may take hold of"
    );
}

#[test]
fn a_tip_carried_along_the_surface_takes_its_neck_and_its_trunk_with_it() {
    let model = shelf();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = under_the_shelf(&model);
    let tip = tip_of(&tree);

    // Aimed 3 mm along and well under the shelf: the cursor need not be on the model.
    let moved = tip_carried(&tree, tip, Vec3::new(8.0, 5.0, 6.0), &placed, &profile())
        .expect("the underside of the shelf is clear at x = 8");

    let at = moved.nodes()[tip].position;
    assert!(
        at.abs_diff_eq(Vec3::new(8.0, 5.0, 10.0), 1e-4),
        "the tip sits on the underside straight over where it was aimed, got {at}"
    );
    let neck = moved.root().position;
    assert!(
        neck.abs_diff_eq(Vec3::new(8.0, 5.0, 10.0 - profile().top.length_mm), 1e-4),
        "the neck hangs one top segment under the tip, got {neck}"
    );
    let base = moved.landing().base;
    assert!(
        (base.x - 8.0).abs() < 1e-4 && base.z.abs() < 1e-4,
        "a lone column stands under its neck again, got {base}"
    );
}

#[test]
fn only_a_tip_is_carried_as_one() {
    let model = shelf();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = under_the_shelf(&model);
    let root = tree.root_index();

    assert!(tip_carried(&tree, root, Vec3::new(8.0, 5.0, 10.0), &placed, &profile()).is_none());
}

#[test]
fn a_flattened_model_carries_nothing() {
    let model = shelf();
    let bvh = Bvh::build(&model);
    let flat = Transform {
        scale: Vec3::new(1.0, 1.0, 0.0),
        ..Transform::default()
    };
    let placed = Placed::new(&model, &bvh, flat);
    let tree = under_the_shelf(&model);

    assert!(carried(&tree, &[Part::Foot], Vec3::X, &placed, &profile()).is_none());
}

#[test]
fn a_blocked_patch_holds_on_a_surface_numbered_otherwise() {
    let model = cube();
    let mut region = core_geometry::Region::default();
    // The cube's underside: the first two faces of the model.
    region.set(0, true);
    region.set(1, true);
    let blocked = crate::Blocked::new(&model, &region, Transform::default())
        .expect("the underside was painted");
    // The same cube with its faces the other way round, as a drilled shell renumbers them.
    let mut surface = model.clone();
    surface.faces.reverse();
    let bvh = Bvh::build(&surface);
    let placed = Placed::new(&surface, &bvh, Transform::default()).blocking(Some(&blocked));

    assert!(
        on_model(&placed, Vec3::new(5.0, 5.0, -0.2)).is_none(),
        "under the painted underside nothing may be put"
    );
    assert!(
        on_model(&placed, Vec3::new(5.0, 5.0, 10.2)).is_some(),
        "the lid was not painted"
    );
}
