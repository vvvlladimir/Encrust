use super::carriers::Carriers;
use super::sound::{halves, only_sound};
use super::*;
use crate::Landing;
use crate::SupportPoint;
use crate::base::braceable;
use crate::column::Column;
use crate::fixtures::{box_mesh, profile, ramp};
use crate::group::Profiles;
use crate::mesher::{Fault, faults, sound};
use crate::placed::Placed;
use crate::tree::{SupportTree, TreeNode};
use core_geometry::{Bvh, Mesh, Transform};
use core_geometry::{Scalar, Vec2, Vec3};
use printer_profiles::SupportProfile;

#[test]
fn a_support_never_stands_through_a_thin_rod_under_it() {
    // A slab underside at z = 10 and a 0.1 mm square rod 6 mm tall, 0.4 mm off the contact:
    // parallel to every ray down the trunk, inside the 0.6 mm pillar from the neck.
    let mut model = box_mesh(Vec3::new(-6.0, -6.0, 10.0), Vec3::new(6.0, 6.0, 12.0));
    let rod = box_mesh(Vec3::new(0.35, -0.05, 0.0), Vec3::new(0.45, 0.05, 6.0));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&rod.vertices);
    model
        .faces
        .extend(rod.faces.iter().map(|face| face.map(|at| at + offset)));
    let fixture = profile();
    let profiles = Profiles::single(&fixture);
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let forest = forest_of(&[Vec3::new(0.0, 0.0, 10.0)], &model, profiles);
    assert_eq!(forest.len(), 1, "it still stands, beside the rod");
    let tree = &forest[0];
    assert!(sound(tree, profiles, &placed), "got {tree:?}");
    for step in 0..=12 {
        let z = step as Scalar * 0.5;
        let axis = crate::mesher::trunk_axis_at(tree, &fixture, z);
        let apart = axis.truncate().distance(Vec2::new(0.4, 0.0));
        let radius_mm = crate::mesher::trunk_radius_at_mm(tree, &fixture, z);
        assert!(
            apart >= radius_mm + 0.05,
            "at z = {z} the trunk's axis is {apart} mm off the rod, inside its {radius_mm} mm"
        );
    }
}

#[test]
fn a_tree_splits_at_its_lowest_joint_into_the_tips_of_each_branch() {
    // Tips 0 and 1 meet at node 4, 2 and 3 at node 5; those two meet at node 6, which
    // stands on a knee at node 7.
    let node = |x: Scalar, z: Scalar, parent, point| TreeNode {
        position: Vec3::new(x, 0.0, z),
        radius_mm: 0.5,
        parent,
        point,
        head: None,
    };
    let nodes = vec![
        node(0.0, 20.0, Some(4), Some(0)),
        node(2.0, 20.0, Some(4), Some(1)),
        node(6.0, 20.0, Some(5), Some(2)),
        node(8.0, 20.0, Some(5), Some(3)),
        node(1.0, 18.0, Some(6), None),
        node(7.0, 18.0, Some(6), None),
        node(4.0, 15.0, Some(7), None),
        node(4.0, 12.0, None, None),
    ];
    let landing = Landing {
        base: Vec3::new(4.0, 0.0, 0.0),
        on_model: false,
        pad_offset: Vec2::ZERO,
        pad_cut: None,
        on_support: false,
    };
    let tree = SupportTree::new(nodes, 7, landing);

    let mut split = halves(&tree);
    for half in &mut split {
        half.sort_unstable();
    }
    split.sort();
    assert_eq!(split, vec![vec![0, 1], vec![2, 3]]);
}

#[test]
fn a_support_blocked_by_a_thin_rod_leans_no_further_than_it_needs() {
    // A slab underside at z = 30 and a 0.1 mm rod 0.4 mm off the contact, 26 mm tall: a 1 mm step
    // aside clears the pillar and its 0.5 mm of air.
    let mut model = box_mesh(Vec3::new(-6.0, -6.0, 30.0), Vec3::new(6.0, 6.0, 32.0));
    let rod = box_mesh(Vec3::new(0.35, -0.05, 0.0), Vec3::new(0.45, 0.05, 26.0));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&rod.vertices);
    model
        .faces
        .extend(rod.faces.iter().map(|face| face.map(|at| at + offset)));
    let forest = forest_of(
        &[Vec3::new(0.0, 0.0, 30.0)],
        &model,
        Profiles::single(&profile()),
    );
    let aside = forest[0].landing().base.truncate().length();
    assert!(aside <= 2.0 + 1e-4, "it stood {aside} mm aside");
}

#[test]
fn a_merged_tree_through_the_model_is_stood_again_tip_by_tip() {
    // Two contacts 2 mm apart under a slab at z = 10, merged by hand at z = 7 over a
    // rod standing 0.3 mm off the trunk's axis, between its rays.
    let mut model = box_mesh(Vec3::new(-6.0, -6.0, 10.0), Vec3::new(6.0, 6.0, 12.0));
    let rod = box_mesh(Vec3::new(0.25, -0.05, 0.0), Vec3::new(0.35, 0.05, 6.0));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&rod.vertices);
    model
        .faces
        .extend(rod.faces.iter().map(|face| face.map(|at| at + offset)));
    let fixture = profile();
    let profiles = Profiles::single(&fixture);
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let points = [
        SupportPoint::new(Vec3::new(-1.0, 0.0, 10.0)),
        SupportPoint::new(Vec3::new(1.0, 0.0, 10.0)),
    ];
    let columns = crate::columns(&points, &placed, profiles);
    let radius_mm = fixture.pillar_radius_mm();
    let node = |position, parent, point| TreeNode {
        position,
        radius_mm,
        parent,
        point,
        head: None,
    };
    let nodes = vec![
        node(columns[0].contact, Some(2), Some(0)),
        node(columns[1].contact, Some(3), Some(1)),
        node(columns[0].neck, Some(4), None),
        node(columns[1].neck, Some(4), None),
        node(Vec3::new(0.0, 0.0, 7.0), None, None),
    ];
    let landing = Landing {
        base: Vec3::ZERO,
        on_model: false,
        pad_offset: Vec2::ZERO,
        pad_cut: None,
        on_support: false,
    };
    let merged = SupportTree::new(nodes, 4, landing);
    assert!(
        !sound(&merged, profiles, &placed),
        "the trunk runs through the rod"
    );

    let carriers = Carriers::new(fixture.branching.max_merge_distance_mm);
    let stood = only_sound(vec![merged], &columns, &placed, profiles, &carriers);
    let mut held: Vec<usize> = stood.iter().flat_map(SupportTree::points).collect();
    held.sort_unstable();
    assert_eq!(held, [0, 1], "both tips stand, each on its own");
    assert!(stood.iter().all(|tree| sound(tree, profiles, &placed)));
}

#[test]
fn a_support_over_a_wide_block_leans_further_than_a_merge_for_the_plate() {
    // A slab's underside at z = 30 over a block 25 mm square and 5 mm tall, which the
    // profile will not stand on: the plate is more than a merge's 12 mm away.
    let mut model = box_mesh(Vec3::new(-20.0, -20.0, 30.0), Vec3::new(20.0, 20.0, 32.0));
    let block = box_mesh(Vec3::new(-12.5, -12.5, 0.0), Vec3::new(12.5, 12.5, 5.0));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&block.vertices);
    model
        .faces
        .extend(block.faces.iter().map(|face| face.map(|at| at + offset)));
    let mut fixture = profile();
    fixture.land_on_model = false;
    let forest = forest_of(
        &[Vec3::new(0.0, 0.0, 30.0)],
        &model,
        Profiles::single(&fixture),
    );
    assert_eq!(forest.len(), 1, "it stands");
    let landing = forest[0].landing();
    assert!(!landing.on_model);
    let across = landing.base.truncate().length();
    assert!(
        across > fixture.branching.max_merge_distance_mm,
        "it leant {across} mm, past what a merge reaches"
    );
}

#[test]
fn a_tip_whose_head_meets_the_model_leaves_the_rest_of_its_tree_merged() {
    // Three contacts under a slab at z = 10, merged by hand; a rod hangs 0.3 mm beside
    // the third one's contact, inside its head.
    let mut model = box_mesh(Vec3::new(-6.0, -6.0, 10.0), Vec3::new(6.0, 6.0, 12.0));
    let rod = box_mesh(Vec3::new(3.25, -0.05, 8.5), Vec3::new(3.35, 0.05, 10.0));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&rod.vertices);
    model
        .faces
        .extend(rod.faces.iter().map(|face| face.map(|at| at + offset)));
    let fixture = profile();
    let profiles = Profiles::single(&fixture);
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let points = [-1.0, 1.0, 3.0].map(|x| SupportPoint::new(Vec3::new(x, 0.0, 10.0)));
    let mut columns = crate::columns(&points, &placed, profiles);
    // Straight down, as it would have stood without the rod, which refuses its head there.
    columns.retain(|column| column.point != 2);
    let contact = points[2].contact;
    columns.push(Column {
        point: 2,
        neck: contact - Vec3::Z * fixture.top.length_mm,
        contact,
        ..columns[1]
    });
    let radius_mm = fixture.pillar_radius_mm();
    let node = |position, parent, point| TreeNode {
        position,
        radius_mm,
        parent,
        point,
        head: None,
    };
    let nodes = vec![
        node(columns[0].contact, Some(3), Some(0)),
        node(columns[1].contact, Some(4), Some(1)),
        node(columns[2].contact, Some(5), Some(2)),
        node(columns[0].neck, Some(6), None),
        node(columns[1].neck, Some(6), None),
        node(columns[2].neck, Some(6), None),
        node(Vec3::new(1.0, 0.0, 6.0), None, None),
    ];
    let landing = Landing {
        base: Vec3::new(1.0, 0.0, 0.0),
        on_model: false,
        pad_offset: Vec2::ZERO,
        pad_cut: None,
        on_support: false,
    };
    let merged = SupportTree::new(nodes, 6, landing);
    assert_eq!(
        faults(&merged, profiles, &placed),
        Some(Fault::Heads(vec![2])),
        "only the third head"
    );

    let carriers = Carriers::new(fixture.branching.max_merge_distance_mm);
    let stood = only_sound(vec![merged], &columns, &placed, profiles, &carriers);
    assert!(
        stood.iter().any(|tree| {
            let held: Vec<usize> = tree.points().collect();
            held.contains(&0) && held.contains(&1)
        }),
        "the two good tips still share a trunk"
    );
    assert!(stood.iter().all(|tree| sound(tree, profiles, &placed)));
}

/// The trees a set of contacts grows into, over an empty plate unless a model is
/// given.
fn forest_of(contacts: &[Vec3], model: &Mesh, profiles: Profiles) -> Vec<SupportTree> {
    let bvh = Bvh::build(model);
    let points: Vec<SupportPoint> = contacts.iter().copied().map(SupportPoint::new).collect();
    let columns = crate::columns(
        &points,
        &Placed::new(model, &bvh, Transform::default()),
        profiles,
    );
    grow(
        &columns,
        &Placed::new(model, &bvh, Transform::default()),
        profiles,
    )
}

/// A thick support of group 0 at x = 10 and a thin one of group 1 `apart` along x,
/// both 20 mm over an empty plate.
fn thick_and_thin(apart: Scalar) -> Vec<SupportTree> {
    let points = [
        SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
        SupportPoint::new(Vec3::new(10.0 + apart, 10.0, 20.0)).in_group(1),
    ];
    grown_over_plate(&points, &[profile(), thinner()])
}

/// The profile of the fixture with its pillar half as wide.
fn thinner() -> SupportProfile {
    let mut thin = profile();
    thin.middle.diameter_mm *= 0.5;
    thin
}

/// The trees `points` grow over an empty plate, each built to its group in `table`.
fn grown_over_plate(points: &[SupportPoint], table: &[SupportProfile]) -> Vec<SupportTree> {
    let model = Mesh::default();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let profiles = Profiles::new(table).expect("a group at least");
    grow(
        &crate::columns(points, &placed, profiles),
        &placed,
        profiles,
    )
}

#[test]
fn two_tips_of_different_groups_stay_two_trees_of_their_own_shape() {
    let forest = thick_and_thin(4.0);
    assert_eq!(forest.len(), 2, "a tree's tips are all one shape");
    assert_eq!(
        forest.iter().map(SupportTree::group).collect::<Vec<_>>(),
        vec![0, 1],
        "each tree is stamped with the group it was built to"
    );
}

#[test]
fn a_thinner_support_runs_into_the_trunk_of_a_thicker_one_beside_it() {
    let forest = thick_and_thin(4.0);
    let (thick, thin) = (&forest[0], &forest[1]);

    assert!(
        !thick.landing().on_model,
        "the thicker one carries, and stands on the plate"
    );
    let landing = thin.landing();
    assert!(
        landing.on_support && landing.on_model,
        "the thinner one has no foot"
    );
    assert!(
        (landing.base.truncate() - Vec2::new(10.0, 10.0)).length() < 1e-3,
        "it ends on the axis of the thick trunk, got {}",
        landing.base
    );
    let root = thin.root().position;
    let across = (root - landing.base).truncate().length();
    let lean = across / (root.z - landing.base.z);
    assert!(
        lean <= profile().branching.reach_per_drop() + 1e-3,
        "and leans no further than a branch may: {lean} across per unit of drop"
    );
    assert!(
        landing.base.z > thick.landing().base.z + 1.0 && landing.base.z < root.z,
        "it meets the trunk between its foot and its top, at z = {}",
        landing.base.z
    );
}

/// Two groups of one width, the second under the heavier head, a tip of each 4 mm
/// apart 20 mm over an empty plate, the light one at x = 14 and the heavy at x = 10.
fn light_and_heavy() -> (Vec<SupportTree>, [SupportProfile; 2]) {
    let mut heavy = profile();
    heavy.tip.contact_diameter_mm += 0.2;
    let table = [profile(), heavy];
    let points = [
        SupportPoint::new(Vec3::new(14.0, 10.0, 20.0)),
        SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)).in_group(1),
    ];
    let model = Mesh::default();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let profiles = Profiles::new(&table).expect("two groups");
    let forest = grow(
        &crate::columns(&points, &placed, profiles),
        &placed,
        profiles,
    );
    (forest, table)
}

#[test]
fn tips_of_two_groups_of_one_width_merge_into_one_trunk_built_to_the_heavier() {
    let (forest, _) = light_and_heavy();
    assert_eq!(forest.len(), 1, "one trunk carries both tips");
    let tree = &forest[0];
    assert_eq!(tree.tip_count(), 2);
    assert_eq!(tree.group(), 1, "the trunk is the heavier head's group's");
    assert!(!tree.landing().on_model, "and stands on the plate");
    let mut heads: Vec<u16> = tree.tip_groups().collect();
    heads.sort_unstable();
    assert_eq!(heads, vec![0, 1], "each tip keeps its own group");
}

#[test]
fn a_tip_merged_into_another_groups_trunk_is_meshed_with_its_own_head() {
    let (forest, table) = light_and_heavy();
    let mesh = crate::fixtures::meshed(&forest, Profiles::new(&table).expect("two groups"));
    // A plane tip is pushed its 0.2 mm bite into the face, so its disc is above z = 20.
    let widest_at = |x: Scalar| {
        mesh.vertices
            .iter()
            .filter(|vertex| vertex.z > 20.1 && (vertex.x - x).abs() < 1.0)
            .map(|vertex| (vertex.truncate() - Vec2::new(x, 10.0)).length())
            .fold(0.0, Scalar::max)
    };
    assert!(
        (widest_at(14.0) - 0.2).abs() < 1e-3,
        "the light tip touches 0.4 mm across, got a radius of {}",
        widest_at(14.0)
    );
    assert!(
        (widest_at(10.0) - 0.3).abs() < 1e-3,
        "the heavy tip touches 0.6 mm across, got a radius of {}",
        widest_at(10.0)
    );
}

#[test]
fn a_merged_tip_is_drawn_with_its_own_group_down_to_the_joint() {
    let (forest, table) = light_and_heavy();
    let model = Mesh::default();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let meshes = crate::mesh_groups(&forest, &placed, Profiles::new(&table).expect("two"));
    let joint = forest[0].root().position;
    let light_tip = |mesh: &Mesh| {
        mesh.vertices
            .iter()
            .any(|vertex| vertex.z > 19.0 && (vertex.x - 14.0).abs() < 0.5)
    };
    assert!(
        light_tip(&meshes[0]),
        "the light tip is drawn as the light group"
    );
    assert!(!light_tip(&meshes[1]), "and not as the heavy one");
    assert!(
        meshes[0]
            .vertices
            .iter()
            .all(|vertex| vertex.z > joint.z - 1.5),
        "nothing of the light group is drawn below the joint at {joint}"
    );
    assert!(
        meshes[1].vertices.iter().any(|vertex| vertex.z < 1.0),
        "the trunk and the foot are the heavy group's"
    );
}

#[test]
fn groups_of_two_widths_grow_apart_and_one_of_one_width_together() {
    let mut thin = profile();
    thin.middle.diameter_mm *= 0.5;
    let mut heavy = profile();
    heavy.tip.contact_diameter_mm += 0.2;
    let table = [profile(), thin, heavy];
    let profiles = Profiles::new(&table).expect("three groups");
    assert_eq!(
        families(profiles),
        vec![vec![2, 0], vec![1]],
        "the two of one width share a family, heavier head first"
    );
}

#[test]
fn a_thinner_support_runs_into_a_branch_of_a_thicker_tree_and_not_only_its_trunk() {
    let mut thin = profile();
    thin.middle.diameter_mm *= 0.5;
    let table = [profile(), thin];
    // Two thick tips that share a trunk, and a thin one beside the middle of them:
    // the branches lean towards it, so it meets one before it could meet the trunk.
    let points = [
        SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
        SupportPoint::new(Vec3::new(16.0, 10.0, 20.0)),
        SupportPoint::new(Vec3::new(13.0, 12.0, 20.0)).in_group(1),
    ];
    let model = Mesh::default();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let profiles = Profiles::new(&table).expect("two groups");
    let forest = grow(
        &crate::columns(&points, &placed, profiles),
        &placed,
        profiles,
    );

    assert_eq!(forest.len(), 2, "the two thick tips share a trunk");
    let (thick, thin) = (&forest[0], &forest[1]);
    assert_eq!(thick.tip_count(), 2);
    let joint_z = thick.root().position.z;
    let landing = thin.landing();
    assert!(landing.on_support, "the thin one has something to run into");
    assert!(
        landing.base.z > joint_z,
        "the branch beside it is met at z = {}, above the joint at {joint_z} the trunk \
             begins under",
        landing.base.z
    );
}

#[test]
fn a_support_run_into_another_is_meshed_closed_and_without_a_foot() {
    let mut thin = profile();
    thin.middle.diameter_mm *= 0.5;
    let table = [profile(), thin];
    let profiles = Profiles::new(&table).expect("two groups");
    let forest = thick_and_thin(4.0);

    let model = Mesh::default();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let mesh = crate::mesh_trees(&forest, &placed, profiles);

    assert_eq!(core_geometry::diagnose(&mesh).boundary_edges, 0);
    let join_z = forest[1].landing().base.z;
    let under = mesh
        .vertices
        .iter()
        .filter(|vertex| vertex.z < join_z - 1.0)
        // Past the rim of the thick support's own pad, which reaches x = 14.
        .filter(|vertex| vertex.x > 14.2)
        .count();
    assert_eq!(
        under, 0,
        "nothing of the thin support is built under where it joins the thick one"
    );
}

#[test]
fn a_contact_inside_the_model_is_not_run_out_through_its_skin() {
    // In the middle of a solid block hanging over the plate: straight down is the
    // inside of its underside, 5 mm away.
    let model = box_mesh(Vec3::new(0.0, 0.0, 5.0), Vec3::new(10.0, 10.0, 15.0));
    let forest = forest_of(
        &[Vec3::new(5.0, 5.0, 10.0)],
        &model,
        Profiles::single(&profile()),
    );
    let highest = forest
        .iter()
        .flat_map(SupportTree::nodes)
        .map(|node| node.position.z)
        .fold(Scalar::NEG_INFINITY, Scalar::max);
    assert!(
        highest <= 5.0 + 1e-4,
        "the tip is moved onto the underside at z = 5, not built in the solid, got {highest}"
    );
}

/// The fixture with a pillar too slim to grow beside it or to be run into by it.
fn slimmer() -> SupportProfile {
    let mut slimmer = profile();
    slimmer.middle.diameter_mm -= 0.2;
    slimmer
}

#[test]
fn a_small_pillar_stands_on_the_support_beside_it_and_never_the_other_way_round() {
    // A block with a contact 10 mm over it (a small pillar on the block) and a contact beside it
    // with plate under it; two groups of two widths so they do not merge, the pillar's first.
    let block = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let table = [profile(), slimmer()];
    let points = [
        SupportPoint::new(Vec3::new(9.0, 5.0, 20.0)),
        // Far enough out for its pad to clear the block, so it stands straight.
        SupportPoint::new(Vec3::new(14.5, 5.0, 20.0)).in_group(1),
    ];
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());
    let profiles = Profiles::new(&table).expect("two groups");
    let forest = grow(
        &crate::columns(&points, &placed, profiles),
        &placed,
        profiles,
    );

    let (pillar, column) = (&forest[0], &forest[1]);
    assert!(
        !column.landing().on_model,
        "the regular support stands on the plate, not on the pillar"
    );
    let landing = pillar.landing();
    assert!(
        landing.on_support,
        "the pillar runs into it instead of the block"
    );
    assert!(
        (landing.base.truncate() - Vec2::new(14.5, 5.0)).length() < 1e-3,
        "it ends on the column's axis, got {}",
        landing.base
    );
    let thin = profile().small_pillar.radius_mm();
    assert!(
        pillar
            .nodes()
            .iter()
            .all(|node| (node.radius_mm - thin).abs() < 1e-6),
        "and it is a small pillar's width all the way up"
    );
}

#[test]
fn a_small_pillar_stays_on_the_part_when_the_support_beside_it_is_further() {
    // As above, but over a post whose top is 2 mm under the pillar's neck, while the
    // column beside it is over 7 mm away at the steepest lean.
    let block = box_mesh(Vec3::new(8.5, 4.5, 0.0), Vec3::new(9.5, 5.5, 16.0));
    let table = [profile(), slimmer()];
    let points = [
        SupportPoint::new(Vec3::new(9.0, 5.0, 20.0)),
        SupportPoint::new(Vec3::new(14.5, 5.0, 20.0)).in_group(1),
    ];
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());
    let profiles = Profiles::new(&table).expect("two groups");
    let forest = grow(
        &crate::columns(&points, &placed, profiles),
        &placed,
        profiles,
    );

    let landing = forest[0].landing();
    assert!(
        landing.on_model && !landing.on_support,
        "it stands on the post, got {landing:?}"
    );
}

#[test]
fn a_small_pillar_never_stands_on_another_that_has_moved_away() {
    // The block and supports above plus a third group with a thinner pillar beside the first: its
    // nearest carrier would be the trunk the first pillar left when it ran into the column.
    let block = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let mut thinner = profile();
    thinner.small_pillar.diameter_mm -= 0.2;
    thinner.middle.diameter_mm -= 0.4;
    let table = [profile(), slimmer(), thinner];
    let points = [
        SupportPoint::new(Vec3::new(9.0, 5.0, 20.0)),
        SupportPoint::new(Vec3::new(14.5, 5.0, 20.0)).in_group(1),
        SupportPoint::new(Vec3::new(9.0, 3.0, 20.0)).in_group(2),
    ];
    let bvh = Bvh::build(&block);
    let placed = Placed::new(&block, &bvh, Transform::default());
    let profiles = Profiles::new(&table).expect("three groups");
    let forest = grow(
        &crate::columns(&points, &placed, profiles),
        &placed,
        profiles,
    );

    let (moved, thin) = (&forest[0], &forest[2]);
    assert!(moved.landing().on_support, "the first ran into the column");
    let landing = thin.landing();
    assert!(
        !landing.on_support || (landing.base.truncate() - Vec2::new(14.5, 5.0)).length() < 1e-3,
        "nothing stands at {} any more for it to end on",
        landing.base
    );
}

#[test]
fn a_trunk_leans_no_further_than_45_degrees_whatever_a_branch_may() {
    let mut wide = profile();
    wide.branching.max_angle_deg = 70.0;
    assert!(
        (lean_per_drop(&wide) - 1.0).abs() < 1e-6,
        "70 degrees is held to 45"
    );

    let mut narrow = profile();
    narrow.branching.max_angle_deg = 30.0;
    assert!(
        (lean_per_drop(&narrow) - 30.0_f32.to_radians().tan()).abs() < 1e-6,
        "a profile that asks for less keeps what it asked for"
    );
}

#[test]
fn two_tips_a_small_pillar_reaches_from_the_model_stand_apart_rather_than_merge() {
    // Two contacts a millimetre apart, 6 mm over a shelf: within a small pillar's
    // 10 mm, so each is a thin strut of its own rather than one full support.
    let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
    let contacts = [Vec3::new(4.5, 5.0, 16.0), Vec3::new(5.5, 5.0, 16.0)];
    let fixture = profile();
    let forest = forest_of(&contacts, &shelf, Profiles::single(&fixture));

    assert_eq!(forest.len(), 2, "each tip stands alone");
    for tree in &forest {
        assert_eq!(tree.tip_count(), 1);
        assert!(
            tree.landing().on_model,
            "on the shelf, got {:?}",
            tree.landing()
        );
        assert!(
            crate::mesher::is_small(tree, &fixture),
            "and is a small pillar"
        );
    }

    let mut far = fixture.clone();
    far.small_pillar.enabled = false;
    let merged = forest_of(&contacts, &shelf, Profiles::single(&far));
    assert_eq!(merged.len(), 1, "without small pillars the two still merge");
}

#[test]
fn a_tip_over_a_thin_bar_is_run_onto_the_bar_rather_than_through_it() {
    let model = crate::fixtures::under_a_gap_over_a_bar();
    let forest = forest_of(
        &[Vec3::new(0.0, 0.0, 10.5)],
        &model,
        Profiles::single(&profile()),
    );

    assert_eq!(forest.len(), 1, "the contact is held, not left bare");
    let tree = &forest[0];
    assert!(tree.root().is_leaf(), "one strut, from the contact itself");
    let landing = tree.landing();
    assert!(
        landing.on_model && (landing.base.z - 10.0).abs() < 1e-3,
        "it comes down on the bar's top, half a millimetre under it, got {}",
        landing.base
    );
}

#[test]
fn a_tip_with_no_room_for_a_trunk_is_run_straight_onto_the_model_under_it() {
    // A ledge a millimetre over a block: too little for a head and a column.
    let mut model = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 9.0));
    let ledge = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 12.0));
    let offset = model.vertices.len() as u32;
    model.vertices.extend_from_slice(&ledge.vertices);
    model
        .faces
        .extend(ledge.faces.iter().map(|face| face.map(|at| at + offset)));

    let contact = Vec3::new(5.0, 5.0, 10.0);
    let forest = forest_of(&[contact], &model, Profiles::single(&profile()));

    assert_eq!(forest.len(), 1, "the contact is held, not left bare");
    let tree = &forest[0];
    assert_eq!(tree.nodes().len(), 1, "one strut, from the contact itself");
    assert!(tree.root().is_leaf());
    let landing = tree.landing();
    assert!(landing.on_model);
    assert!(
        landing.base.abs_diff_eq(Vec3::new(5.0, 5.0, 9.0), 1e-3),
        "it comes down on the block straight under it, got {}",
        landing.base
    );

    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let mesh = crate::mesh_trees(&forest, &placed, Profiles::single(&profile()));
    assert!(!mesh.is_empty(), "and it is built");
    assert_eq!(core_geometry::diagnose(&mesh).boundary_edges, 0);
}

#[test]
fn a_strut_into_a_thicker_support_is_no_longer_than_a_strut_of_a_merge() {
    // Inside the merge distance, and further than the half of it each of two merging
    // fronts would have to cover.
    let far = profile().branching.max_merge_distance_mm * 0.75;
    let forest = thick_and_thin(far);
    assert!(
        forest.iter().all(|tree| !tree.landing().on_model),
        "the thin one stands on the plate rather than reach {far} mm across"
    );
}

#[test]
fn a_strut_into_a_thicker_support_is_held_to_the_longest_strut() {
    // 4 mm across at 45 degrees from the neck: the strut is 5.7 mm long.
    let points = [
        SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
        SupportPoint::new(Vec3::new(14.0, 10.0, 20.0)).in_group(1),
    ];
    let mut thin = thinner();
    thin.branching.max_strut_length_mm = 5.0;
    let forest = grown_over_plate(&points, &[profile(), thin]);
    assert!(
        forest.iter().all(|tree| !tree.landing().on_support),
        "a 5.7 mm strut is past a 5 mm limit, so the thin one stands on the plate"
    );
}

#[test]
fn a_thicker_trunk_takes_on_no_more_tips_than_the_profile_allows() {
    let mut thick = profile();
    thick.branching.max_tips_per_trunk = 2;
    let points = [
        SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
        SupportPoint::new(Vec3::new(12.0, 10.0, 20.0)),
        SupportPoint::new(Vec3::new(11.0, 14.0, 20.0)).in_group(1),
    ];
    let forest = grown_over_plate(&points, &[thick, thinner()]);
    assert!(
        forest.iter().all(|tree| !tree.landing().on_support),
        "the thick trunk already carries its two tips, so the thin one stands alone"
    );
}

#[test]
fn a_thinner_support_out_of_reach_of_a_thicker_one_stands_on_the_plate() {
    let far = profile().branching.max_merge_distance_mm + 5.0;
    let forest = thick_and_thin(far);
    assert!(forest.iter().all(|tree| !tree.landing().on_model));
}

#[test]
fn two_tips_side_by_side_share_one_trunk() {
    let forest = forest_of(
        &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
        &Mesh::default(),
        Profiles::single(&profile()),
    );

    assert_eq!(forest.len(), 1, "both tips hang off one trunk");
    assert_eq!(forest[0].tip_count(), 2);
}

#[test]
fn two_tips_meet_halfway_between_them() {
    let forest = forest_of(
        &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
        &Mesh::default(),
        Profiles::single(&profile()),
    );
    let root = forest[0].root();

    assert!(
        (root.position.x - 12.0).abs() < 1e-4,
        "two tips at the same height meet over the middle, got {}",
        root.position.x
    );
    // Four millimetres apart at 45 degrees is two millimetres of drop, taken from the
    // necks the struts leave rather than from the contacts above them.
    let from_z = 20.0 - profile().top.length_mm;
    assert!(
        (root.position.z - (from_z - 2.0)).abs() < 1e-4,
        "a 45 degree lean drops as far as it reaches sideways, got {}",
        root.position.z
    );
}

#[test]
fn two_tips_whose_struts_would_graze_the_model_drop_past_it_and_merge_below() {
    // A ledge beside where the two struts run down to their meeting point: they pass
    // it by 0.6 mm, which is less than the body and the air it keeps.
    let ledge = box_mesh(Vec3::new(11.5, 10.6, 17.0), Vec3::new(12.5, 12.0, 19.0));
    let fixture = profile();
    let forest = forest_of(
        &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
        &ledge,
        Profiles::single(&fixture),
    );

    assert_eq!(forest.len(), 1, "both tips still share a trunk");
    let joint = forest[0]
        .nodes()
        .iter()
        .enumerate()
        .find(|(index, _)| {
            forest[0]
                .nodes()
                .iter()
                .filter(|node| node.parent == Some(*index))
                .count()
                == 2
        })
        .map(|(_, node)| node.position)
        .expect("a merged tree has a joint");
    assert!(
        joint.z < 17.0,
        "they meet under the ledge, not beside it, got {joint}"
    );
    let bvh = Bvh::build(&ledge);
    let placed = Placed::new(&ledge, &bvh, Transform::default());
    assert!(sound(&forest[0], Profiles::single(&fixture), &placed));
}

#[test]
fn a_tip_with_nothing_under_it_leans_until_it_finds_room() {
    // A face too steep to stand on, with the tip right over it. Straight down there
    // is nowhere to go, so the support bends a knee and steps off the ramp.
    let steep = ramp(4.0, 10.0);
    let contact = Vec3::new(2.0, 0.0, 20.0);
    let forest = forest_of(&[contact], &steep, Profiles::single(&profile()));

    assert_eq!(forest.len(), 1, "the tip is held");
    let tree = &forest[0];
    assert_eq!(tree.tip_count(), 1);
    assert!(!tree.landing().on_model, "it stepped off onto the plate");

    assert!(
        tree.root().point.is_none(),
        "the knee holds no tip of its own"
    );
    let knee = tree.root().position;
    assert!(
        (knee.truncate() - contact.truncate()).length() > 1.0,
        "the knee has to be somewhere else than under the contact, got {knee}"
    );
    assert!(knee.z < contact.z, "a lean descends");
}

#[test]
fn tips_further_apart_than_the_merge_distance_stay_apart() {
    let profile = profile();
    let far = profile.branching.max_merge_distance_mm + 1.0;
    let forest = forest_of(
        &[
            Vec3::new(10.0, 10.0, 20.0),
            Vec3::new(10.0 + far, 10.0, 20.0),
        ],
        &Mesh::default(),
        Profiles::single(&profile),
    );

    assert_eq!(forest.len(), 2);
    assert!(forest.iter().all(|tree| tree.tip_count() == 1));
}

#[test]
fn a_pair_that_would_meet_below_the_plate_stays_apart() {
    // Ten millimetres apart at 45 degrees needs five millimetres of drop, and these
    // tips stand three above the plate.
    let forest = forest_of(
        &[Vec3::new(10.0, 10.0, 3.0), Vec3::new(20.0, 10.0, 3.0)],
        &Mesh::default(),
        Profiles::single(&profile()),
    );
    assert_eq!(forest.len(), 2);
}

#[test]
fn branching_switched_off_leaves_every_tip_its_own_column() {
    let mut profile = profile();
    profile.branching.enabled = false;
    let forest = forest_of(
        &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
        &Mesh::default(),
        Profiles::single(&profile),
    );

    assert_eq!(forest.len(), 2);
    assert!(
        forest.iter().all(|tree| tree.nodes().len() == 2),
        "a contact and the neck under it, and nothing merged onto either"
    );
}

#[test]
fn a_wall_between_two_tips_keeps_them_apart() {
    // A slab standing between the two tips, so neither strut can reach the middle.
    let wall = box_mesh(Vec3::new(11.5, 0.0, 0.0), Vec3::new(12.5, 20.0, 30.0));
    let forest = forest_of(
        &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
        &wall,
        Profiles::single(&profile()),
    );
    assert_eq!(forest.len(), 2, "the slab is in the way of the merge");
}

#[test]
fn a_shallower_angle_merges_lower_down() {
    let steep = SupportProfile {
        branching: printer_profiles::Branching {
            max_angle_deg: 60.0,
            ..profile().branching
        },
        ..profile()
    };
    let shallow = SupportProfile {
        branching: printer_profiles::Branching {
            max_angle_deg: 20.0,
            ..profile().branching
        },
        ..profile()
    };
    let contacts = [Vec3::new(10.0, 10.0, 40.0), Vec3::new(14.0, 10.0, 40.0)];

    let steep = forest_of(&contacts, &Mesh::default(), Profiles::single(&steep));
    let shallow = forest_of(&contacts, &Mesh::default(), Profiles::single(&shallow));
    assert!(
        shallow[0].root().position.z < steep[0].root().position.z,
        "a strut that may not lean far has to drop further to meet"
    );
}

#[test]
fn a_joint_keeps_the_width_of_the_branches_it_joins() {
    let profile = profile();
    let forest = forest_of(
        &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
        &Mesh::default(),
        Profiles::single(&profile),
    );
    assert!(
        (forest[0].root().radius_mm - profile.pillar_radius_mm()).abs() < 1e-6,
        "only the trunk under the joint widens, and it does so towards its base"
    );
}

#[test]
fn four_tips_in_a_square_end_up_on_one_trunk() {
    let forest = forest_of(
        &[
            Vec3::new(10.0, 10.0, 30.0),
            Vec3::new(14.0, 10.0, 30.0),
            Vec3::new(10.0, 14.0, 30.0),
            Vec3::new(14.0, 14.0, 30.0),
        ],
        &Mesh::default(),
        Profiles::single(&profile()),
    );

    assert_eq!(forest.len(), 1);
    assert_eq!(forest[0].tip_count(), 4);
    assert_eq!(
        forest[0].nodes().len(),
        11,
        "four tips, a neck under each and three joints"
    );
}

#[test]
fn a_trunk_carries_no_more_tips_than_the_profile_allows() {
    let mut fixture = profile();
    fixture.branching.max_tips_per_trunk = 2;
    let forest = forest_of(
        &[
            Vec3::new(10.0, 10.0, 30.0),
            Vec3::new(14.0, 10.0, 30.0),
            Vec3::new(10.0, 14.0, 30.0),
            Vec3::new(14.0, 14.0, 30.0),
        ],
        &Mesh::default(),
        Profiles::single(&fixture),
    );

    assert_eq!(
        forest.len(),
        2,
        "the square stands on two trunks of two tips"
    );
    assert!(forest.iter().all(|tree| tree.tip_count() == 2));
}

#[test]
fn tips_whose_struts_would_run_longer_than_the_profile_allows_stand_apart() {
    // 8 mm apart at 45 degrees: each strut to the meeting point is 5.7 mm long.
    let contacts = [Vec3::new(10.0, 10.0, 30.0), Vec3::new(18.0, 10.0, 30.0)];
    let mut fixture = profile();
    fixture.branching.max_strut_length_mm = 5.0;
    assert_eq!(
        forest_of(&contacts, &Mesh::default(), Profiles::single(&fixture)).len(),
        2
    );
    fixture.branching.max_strut_length_mm = 6.0;
    assert_eq!(
        forest_of(&contacts, &Mesh::default(), Profiles::single(&fixture)).len(),
        1
    );
}

#[test]
fn a_tip_above_another_comes_down_onto_it() {
    // 3 mm apart and 3 or 6 mm higher, or 5 mm apart and 8 mm higher: at 45 degrees the
    // higher one reaches the lower one's column at or above it, so it runs onto it.
    for (across, higher) in [(3.0, 3.0), (3.0, 6.0), (5.0, 8.0)] {
        let forest = forest_of(
            &[
                Vec3::new(10.0, 10.0, 20.0),
                Vec3::new(10.0 + across, 10.0, 20.0 + higher),
            ],
            &Mesh::default(),
            Profiles::single(&profile()),
        );
        assert_eq!(forest.len(), 1, "{across} mm across and {higher} mm higher");
        let lower_neck = Vec3::new(10.0, 10.0, 20.0 - profile().top.length_mm);
        assert!(
            forest[0]
                .nodes()
                .iter()
                .any(|node| node.position.abs_diff_eq(lower_neck, 1e-4) && !node.is_leaf()),
            "the joint is the lower tip's own neck"
        );
    }
}

#[test]
fn every_point_comes_back_exactly_once() {
    let contacts: Vec<Vec3> = (0..16u8)
        .map(|index| {
            Vec3::new(
                10.0 + f32::from(index % 4) * 3.0,
                10.0 + f32::from(index / 4) * 3.0,
                30.0,
            )
        })
        .collect();
    let forest = forest_of(&contacts, &Mesh::default(), Profiles::single(&profile()));

    let mut held: Vec<usize> = forest.iter().flat_map(SupportTree::points).collect();
    held.sort_unstable();
    assert_eq!(held, (0..16).collect::<Vec<_>>());
}

#[test]
fn no_strut_leans_further_than_the_profile_allows() {
    let profile = profile();
    let contacts: Vec<Vec3> = (0..16u8)
        .map(|index| {
            Vec3::new(
                10.0 + f32::from(index % 4) * 3.0,
                10.0 + f32::from(index / 4) * 3.0,
                30.0,
            )
        })
        .collect();

    for tree in forest_of(&contacts, &Mesh::default(), Profiles::single(&profile)) {
        for (child, parent) in tree.struts() {
            let offset = tree.nodes()[parent].position - tree.nodes()[child].position;
            let ground = Vec2::new(offset.x, offset.y).length();
            let drop = -offset.z;
            assert!(drop > 0.0, "a strut always descends, got {drop}");
            assert!(
                ground <= drop * profile.branching.reach_per_drop() + 1e-3,
                "a strut covering {ground} mm of ground must drop at least \
                     {} mm, got {drop}",
                ground / profile.branching.reach_per_drop()
            );
        }
    }
}

/// The fixture with braces spanning 8 mm and branches merging no further than 10.
fn braced_at_8_mm() -> SupportProfile {
    let mut profile = profile();
    profile.branching.max_merge_distance_mm = 10.0;
    profile.bracing = printer_profiles::Bracing {
        enabled: true,
        max_spacing_mm: 8.0,
        start_height_mm: 5.0,
        rise_mm: 5.0,
        ..printer_profiles::Bracing::default()
    };
    profile
}

#[test]
fn a_trunk_is_not_carried_across_a_gap_wider_than_a_branch_reaches_to_meet_a_brace() {
    let profile = braced_at_8_mm();
    let forest = forest_of(
        &[Vec3::new(0.0, 0.0, 40.0), Vec3::new(30.0, 0.0, 40.0)],
        &Mesh::default(),
        Profiles::single(&profile),
    );
    let apart = (forest[0].landing().base - forest[1].landing().base).length();
    assert!(
        (apart - 30.0).abs() < 1e-3,
        "24 mm of lean is a lever, so each stands under its own tip: {apart} mm apart"
    );
}

#[test]
fn a_trunk_leans_towards_one_of_another_group_as_it_would_towards_its_own() {
    let mut thin = braced_at_8_mm();
    thin.middle.diameter_mm *= 0.5;
    let table = [braced_at_8_mm(), thin];
    let points = [
        SupportPoint::new(Vec3::new(0.0, 0.0, 40.0)),
        SupportPoint::new(Vec3::new(11.0, 0.0, 40.0)).in_group(1),
    ];
    let model = Mesh::default();
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let profiles = Profiles::new(&table).expect("two groups");
    let forest = grow(
        &crate::columns(&points, &placed, profiles),
        &placed,
        profiles,
    );

    let (thick, thin) = (&forest[0], &forest[1]);
    assert!(
        (thick.landing().base.truncate()).length() < 1e-3,
        "the thicker one was stood first and stays where it stood"
    );
    let apart = (thick.landing().base - thin.landing().base).length();
    assert!(
        apart <= table[0].bracing.max_spacing_mm,
        "the thinner one leant into a brace's reach of it, {apart} mm apart"
    );
}

#[test]
fn a_tall_trunk_standing_alone_leans_into_reach_of_a_brace() {
    // Two columns 40 mm tall and 11 mm apart: past merging, and past the 8 mm a brace
    // spans, with far more height than the lean costs.
    let profile = braced_at_8_mm();
    let forest = forest_of(
        &[Vec3::new(0.0, 0.0, 40.0), Vec3::new(11.0, 0.0, 40.0)],
        &Mesh::default(),
        Profiles::single(&profile),
    );
    assert_eq!(forest.len(), 2, "11 mm is too far to merge");

    let apart = (forest[0].landing().base - forest[1].landing().base).length();
    assert!(
        apart <= profile.bracing.max_spacing_mm,
        "their trunks stand {apart} mm apart, out of a brace's reach"
    );
    for tree in &forest {
        assert!(
            braceable(tree, &profile),
            "and each is still tall enough to tie"
        );
    }
}

#[test]
fn no_contacts_grow_no_trees() {
    assert!(forest_of(&[], &Mesh::default(), Profiles::single(&profile())).is_empty());
}
