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
        tree.landing(),
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
        tree.landing(),
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
    // through the solid: it has nowhere to stand, or may not stand there.
    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Foot],
        Vec3::new(-15.0, 0.0, 0.0),
        &placed,
        &profile(),
    );
    assert!(moved.is_none_or(|moved| allowed(&tree, &moved, &placed, &profile()).is_err()));
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
        tree.landing(),
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
    let moved = tip_carried(
        &tree,
        tip,
        Vec3::new(8.0, 5.0, 6.0),
        &placed,
        Profiles::single(&profile()),
    )
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

    assert_eq!(
        tip_carried(
            &tree,
            root,
            Vec3::new(8.0, 5.0, 10.0),
            &placed,
            Profiles::single(&profile())
        )
        .err(),
        Some(Refused::NoHold)
    );
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

    assert!(
        carried(
            &tree,
            tree.landing(),
            &[Part::Foot],
            Vec3::X,
            &placed,
            &profile()
        )
        .is_none()
    );
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

#[test]
fn a_foot_dragged_aside_stands_its_trunk_straight_on_a_knee_at_45_degrees() {
    let model = box_mesh(Vec3::new(0.0, 0.0, 20.0), Vec3::new(10.0, 10.0, 22.0));
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = trees_on(
        &[SupportPoint::new(Vec3::new(5.0, 5.0, 20.0))],
        &model,
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("a column under the slab stands");
    let neck = tree.root().position;
    // Dragged in two steps, as a hand drags it: the knee follows rather than doubling.
    let once = carried(
        &tree,
        tree.landing(),
        &[Part::Foot],
        Vec3::X * 4.0,
        &placed,
        &profile(),
    )
    .expect("the foot moves clear");
    let moved = carried(
        &once,
        once.landing(),
        &[Part::Foot],
        Vec3::X * 4.0,
        &placed,
        &profile(),
    )
    .expect("and on");

    let (base, knee) = (moved.landing().base, moved.root().position);
    assert_eq!(moved.nodes().len(), tree.nodes().len() + 1, "one knee");
    assert!(
        (knee - base).truncate().length() < 1e-4,
        "the trunk stands straight over its foot {base}, from {knee}"
    );
    assert!(
        ((neck.z - knee.z) - (neck - knee).truncate().length()).abs() < 1e-3,
        "the neck {neck} leans down to the knee {knee} at 45 degrees"
    );
}

/// Whether every strut of `tree` under its heads leans no more than 45 degrees, and its trunk
/// stands straight over its foot.
fn upright_below_its_heads(tree: &crate::SupportTree) -> bool {
    let nodes = tree.nodes();
    let struts = tree
        .struts()
        .filter(|(child, _)| !nodes[*child].is_leaf())
        .all(|(child, parent)| {
            let lean = nodes[child].position - nodes[parent].position;
            lean.truncate().length() <= lean.z.abs() + 1e-3
        });
    let trunk = tree.root().position - tree.landing().base;
    struts && trunk.truncate().length() < 1e-3
}

#[test]
fn a_foot_dragged_far_leans_its_support_no_more_than_45_degrees() {
    let model = cube();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let tree = beside(&model, 20.0);

    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Foot],
        Vec3::new(60.0, 0.0, 0.0),
        &placed,
        &profile(),
    )
    .expect("a foot dragged into clear air stands as far out as a 45 degree lean reaches");

    assert!(
        upright_below_its_heads(&moved),
        "no strut lies flatter than 45 degrees, got {:?}",
        moved.nodes()
    );
    assert!(
        moved.landing().base.x > tree.landing().base.x + 5.0,
        "the foot still went the way it was dragged, got {}",
        moved.landing().base
    );
}

#[test]
fn a_support_with_a_strut_lying_flat_does_not_fit() {
    let model = cube();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let mut tree = beside(&model, 20.0);
    let root = tree.root().position;
    tree.stand_on_knee(Vec3::new(root.x + 20.0, root.y, 2.0), tree.root().radius_mm);
    tree.move_landing(Vec3::new(root.x + 20.0, root.y, 0.0));

    assert!(
        !fits(&tree, &placed, &profile()),
        "a strut 20 mm across for {} mm down is no support",
        root.z - 2.0
    );
}

/// Two tips 3 mm apart in clear air, grown into one tree whose root is the joint they meet at.
fn merged_pair() -> crate::SupportTree {
    let tree = trees_on(
        &[
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(13.0, 10.0, 20.0)),
        ],
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("two tips in clear air share a trunk");
    let root = tree.root_index();
    let children = tree.struts().filter(|(_, parent)| *parent == root).count();
    assert_eq!(children, 2, "the root is the joint the two meet at");
    tree
}

#[test]
fn a_trunk_carried_aside_stands_on_a_knee_under_the_joint_and_leaves_the_tips() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let tree = merged_pair();
    let joint = tree.root().position;
    let step = Vec3::new(0.0, 2.0, 0.0);

    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Trunk],
        step,
        &placed,
        &profile(),
    )
    .expect("2 mm aside in clear air stands");

    let (knee, base) = (moved.root().position, moved.landing().base);
    assert!(
        (base - (tree.landing().base + step)).length() < 1e-4,
        "the foot took the step, got {base}"
    );
    assert!(
        (knee - base).truncate().length() < 1e-4,
        "the trunk stands upright"
    );
    assert!(
        ((joint.z - knee.z) - 2.0).abs() < 1e-3,
        "the joint {joint} leans down to the knee {knee} at 45 degrees"
    );
    assert!(
        moved.contacts().eq(tree.contacts()),
        "the tips stay where they hold"
    );
}

#[test]
fn a_column_raised_lifts_its_joint_and_leaves_its_foot() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let tree = merged_pair();

    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Trunk],
        Vec3::new(0.0, 0.0, -1.0),
        &placed,
        &profile(),
    )
    .expect("a joint lowered a millimetre leans its branches less");

    let drop = tree.root().position.z - moved.root().position.z;
    assert!(
        (drop - 1.0).abs() < 1e-4,
        "the joint went 1 mm down, went {drop}"
    );
    assert_eq!(moved.landing().base, tree.landing().base, "the foot stays");
}

#[test]
fn a_column_carried_off_the_slab_it_stood_on_stands_on_the_plate() {
    let model = shelf_over_a_slab();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    // Near the edge of the shelf, so a 45 degree lean reaches well past the slab under it.
    let tree = trees_on(
        &[SupportPoint::new(Vec3::new(9.0, 5.0, 10.0))],
        &model,
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("a column under the shelf stands");
    assert!(tree.landing().on_model, "the slab is under the column");

    // Far enough for the 4 mm pad to clear the slab's edge at x = 10.
    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Foot],
        Vec3::new(5.0, 0.0, 0.0),
        &placed,
        &profile(),
    )
    .expect("5 mm out the pad clears the slab");

    let landing = moved.landing();
    assert!(
        !landing.on_model && landing.base.z.abs() < 1e-4 && landing.base.x > 10.0,
        "it stands on the plate beside the slab, got {landing:?}"
    );
}

/// Two tips close enough to meet and a third further off that joins them lower down, in clear
/// air: a joint that is not the root, with branches above it.
pub(super) fn branched_twice() -> crate::SupportTree {
    trees_on(
        &[(10.0, 10.0), (13.0, 10.0), (11.5, 16.0)]
            .map(|(x, y)| SupportPoint::new(Vec3::new(x, y, 20.0))),
        &Mesh::default(),
        Transform::default(),
        Profiles::single(&profile()),
    )
    .pop()
    .expect("the three tips share a trunk")
}

/// The joint of `tree` above the root that two branches meet at.
pub(super) fn upper_joint(tree: &SupportTree) -> usize {
    let nodes = tree.nodes();
    (0..nodes.len())
        .find(|node| {
            *node != tree.root_index()
                && nodes
                    .iter()
                    .filter(|above| above.parent == Some(*node) && !above.is_leaf())
                    .count()
                    > 1
        })
        .expect("the pair meets at a joint above the root")
}

/// Whether every strut of `tree` under its heads leans no more than 45 degrees.
fn within_45_degrees(tree: &crate::SupportTree) -> bool {
    let nodes = tree.nodes();
    tree.struts()
        .filter(|(child, _)| !nodes[*child].is_leaf())
        .all(|(child, parent)| {
            let lean = nodes[child].position - nodes[parent].position;
            lean.truncate().length() <= lean.z.abs() + 1e-3
        })
}

#[test]
fn a_joint_carried_aside_drops_as_far_as_its_branches_need() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let tree = branched_twice();
    let nodes = tree.nodes();
    let joint = (0..nodes.len())
        .find(|node| {
            *node != tree.root_index()
                && nodes
                    .iter()
                    .filter(|above| above.parent == Some(*node) && !above.is_leaf())
                    .count()
                    > 1
        })
        .expect("the pair meets at a joint above the root");
    let step = Vec3::new(0.0, 1.0, 0.0);

    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Node(joint)],
        step,
        &placed,
        &profile(),
    )
    .expect("a joint in clear air is carried");

    let (was, now) = (nodes[joint].position, moved.nodes()[joint].position);
    assert!(
        (now.truncate() - (was + step).truncate()).length() < 1e-4,
        "the joint went the way it was carried, got {now}"
    );
    assert!(
        now.z < was.z,
        "it dropped from {was} to {now} to keep its branches upright"
    );
    assert!(
        within_45_degrees(&moved),
        "no branch above it leans past 45 degrees"
    );
}

#[test]
fn a_column_raised_past_its_branches_stops_where_they_lean_45_degrees() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let tree = merged_pair();

    let moved = carried(
        &tree,
        tree.landing(),
        &[Part::Trunk],
        Vec3::Z * 3.0,
        &placed,
        &profile(),
    )
    .expect("a column in clear air is carried");

    assert!(
        moved.root().position.z <= tree.root().position.z + 1e-4,
        "the joint already stood as high as its branches let it, got {}",
        moved.root().position
    );
    assert!(allowed(&tree, &moved, &placed, &profile()).is_ok());
}

#[test]
fn a_support_already_out_of_line_may_be_carried_where_what_moves_is_sound() {
    let empty = Mesh::default();
    let bvh = Bvh::build(&empty);
    let placed = Placed::new(&empty, &bvh, Transform::default());
    let mut tree = merged_pair();
    let nodes = tree.nodes();
    let necks: Vec<usize> = (0..nodes.len())
        .filter(|node| nodes[*node].parent == Some(tree.root_index()))
        .collect();
    // One branch lying flat, as a support from an older build may stand.
    let flat = nodes[necks[0]].position + Vec3::Y * 5.0;
    tree.move_node(necks[0], flat);
    assert!(
        !fits(&tree, &placed, &profile()),
        "the flat branch does not fit"
    );

    let other = necks[1];
    let toward = carried(
        &tree,
        tree.landing(),
        &[Part::Node(other)],
        Vec3::X * -0.5,
        &placed,
        &profile(),
    )
    .expect("a neck in clear air is carried");
    let away = carried(
        &tree,
        tree.landing(),
        &[Part::Node(other)],
        Vec3::X * 1.0,
        &placed,
        &profile(),
    )
    .expect("a neck in clear air is carried");

    assert!(
        allowed(&tree, &toward, &placed, &profile()).is_ok(),
        "the other branch, steepened, is sound, whatever the flat one does"
    );
    assert_eq!(
        allowed(&tree, &away, &placed, &profile()),
        Err(Refused::Leans),
        "flattening the other branch past 45 degrees is not"
    );
}
