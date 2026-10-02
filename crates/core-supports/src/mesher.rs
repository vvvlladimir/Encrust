use crate::placed::Placed;
use core_geometry::{Mesh, Scalar, Vec3};
use printer_profiles::{ContactShape, SupportProfile};

use crate::base::{braces, raft, skate, skate_half_width_mm, toe_of};
use crate::group::Profiles;
use crate::tree::{SupportTree, TreeNode};

mod tube;

use tube::{RING_EPSILON_MM, Ring, Tube, ball, empty_tube, push, sweep};

/// How many rings a sphere tip is drawn with, as a fraction of the profile's facets.
/// A tip is under a millimetre across, so the silhouette needs far fewer rings than the
/// circumference needs sides.
const SPHERE_RINGS: u32 = 6;

/// How far a tapering foot narrows on the way up to the support it carries: it never
/// closes in past this fraction of its own radius, so the rim stays a rim.
const FOOT_TAPER: Scalar = 0.45;

/// How far the pad's wall pulls in on the way down to the plate, as a fraction of its own
/// height. The whole wall is the bevel, so this is the tangent of its lean: 0.6 stands it
/// at 59 degrees off the plate. Every foot has one: a pad that meets the plate on a sharp
/// rim is the one a blade gets under. See `docs/decisions/0044` and `docs/decisions/0130`.
pub const FOOT_BEVEL: Scalar = 0.6;

/// How far a trunk reaches into the foot under it, as a fraction of the foot's height.
/// Far enough that the two solids overlap, not so far that the trunk shows through the
/// bottom of the pad.
const FOOT_BITE: Scalar = 0.5;

/// How many rings a joint's ball is drawn with. It only has to cover the seam where the
/// struts meet, so the silhouette needs far fewer rings than the circumference needs
/// sides. An even count puts one of them on the equator, which is where the ball has to
/// be at its full width.
const JOINT_RINGS: u32 = 6;

/// How much wider than the trunk a joint's ball is. A strut ending at the centre of a
/// ball of its own radius has its end cap exactly on the surface, and the flats of a
/// polygon fall inside the circle they stand for; a little swell puts both safely under.
const JOINT_SWELL: Scalar = 1.15;

/// How far a strut reaches past a node it runs straight through, millimetres. Two tubes
/// that only met on a plane would leave the fill rule a seam to argue about.
const JOINT_OVERLAP_MM: Scalar = 0.1;

/// Two directions this close are one direction: half a degree, which is finer than the
/// facets either tube is drawn with.
const COLLINEAR: Scalar = 0.999_961;

/// Which set of measurements a support is built from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Style {
    /// The thin model-to-model strut of `[small_pillar]` rather than a regular support.
    small: bool,
}

impl Style {
    /// A support is small when it runs between two parts of the model and carries one
    /// tip. A trunk that several tips hang off carries their weight and keeps its own
    /// width whatever it stands on.
    fn of(tree: &SupportTree, profile: &SupportProfile) -> Self {
        Self {
            small: profile.small_pillar.enabled && tree.landing().on_model && tree.tip_count() == 1,
        }
    }

    /// How far the tip bites into the model, millimetres.
    fn bite_mm(self, profile: &SupportProfile) -> Scalar {
        if self.small {
            profile.small_pillar.upper_depth_mm
        } else {
            profile.contact_depth_mm()
        }
    }

    /// Width of the body, millimetres, given what the tree itself asked for.
    fn body_radius_mm(self, node_radius_mm: Scalar, profile: &SupportProfile) -> Scalar {
        if self.small {
            profile.small_pillar.radius_mm().min(node_radius_mm)
        } else {
            node_radius_mm
        }
    }
}

/// Welds every support into one mesh, in plate coordinates.
///
/// Each strut is its own closed tube, and a tube reaches past the joint it ends at so
/// that it overlaps the one below. Overlapping closed solids are what the non-zero
/// winding fill already unions; see `docs/decisions/0008` and `docs/decisions/0041`.
///
/// `placed` puts the mesh the braces have to keep clear of in the same space; every other
/// body was already tested where it was built.
///
/// Each tree is built to its own group's profile, while the raft under them and the
/// bracing between them are built to group 0's: they belong to the plate rather than to
/// any one support. See `docs/decisions/0094`.
pub fn mesh_trees(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Mesh {
    let mut mesh = Mesh::default();
    for tree in trees {
        mesh_tree(&mut mesh, tree, profiles.of(tree.group()));
    }
    mesh_ground(&mut mesh, trees, placed, profiles.ground());
    mesh
}

/// The same supports as [`mesh_trees`], one mesh per group in group order, so that each
/// can be drawn apart. The raft and the bracing go with group 0, whose profile they take.
pub fn mesh_groups(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Vec<Mesh> {
    let mut meshes = vec![Mesh::default(); profiles.groups().count()];
    for tree in trees {
        let slot = usize::from(tree.group());
        let slot = if slot < meshes.len() { slot } else { 0 };
        mesh_tree(&mut meshes[slot], tree, profiles.of(tree.group()));
    }
    mesh_ground(&mut meshes[0], trees, placed, profiles.ground());
    meshes
}

fn mesh_tree(mesh: &mut Mesh, tree: &SupportTree, profile: &SupportProfile) {
    let style = Style::of(tree, profile);
    let covered = covers(tree, profile);
    for (child, parent) in tree.struts() {
        let overlap_mm = if covered[parent] {
            0.0
        } else {
            JOINT_OVERLAP_MM
        };
        sweep(
            mesh,
            &strut(tree, child, parent, profile, style, overlap_mm),
        );
    }
    sweep(mesh, &trunk(tree, profile, style));
    for (node, _) in covered.iter().enumerate().filter(|(_, covered)| **covered) {
        sweep(mesh, &joint(&tree.nodes()[node], profile, style));
    }
    stand_on(mesh, tree, profile);
}

fn mesh_ground(mesh: &mut Mesh, trees: &[SupportTree], placed: &Placed, ground: &SupportProfile) {
    for (from, to) in braces(trees, placed, ground) {
        sweep(mesh, &brace(from, to, ground));
    }
    if let Some(slab) = raft(trees, ground) {
        append(mesh, &slab);
    }
}

/// Adds `other` to `mesh` as a solid of its own, renumbering its faces.
fn append(mesh: &mut Mesh, other: &Mesh) {
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&other.vertices);
    mesh.faces.extend(
        other
            .faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}

/// Whatever the tree stands on: the flare out of the pillar, and then a round foot swept
/// as rings or a skate. Nothing at all when it lands on the model.
fn stand_on(mesh: &mut Mesh, tree: &SupportTree, profile: &SupportProfile) {
    if tree.landing().on_model {
        return;
    }
    let style = Style::of(tree, profile);
    if let Some(tube) = flare(tree, profile, style) {
        sweep(mesh, &tube);
    }
    if profile.bottom.shape.is_round() {
        if let Some(tube) = foot(tree, profile, style) {
            sweep(mesh, &tube);
        }
        return;
    }

    let base = tree.landing().base;
    skate(mesh, Vec3::new(base.x, base.y, 0.0), toe_of(tree), profile);
}

/// One brace, as a tube between two points on neighbouring trunks.
fn brace(from: Vec3, to: Vec3, profile: &SupportProfile) -> Tube {
    let offset = to - from;
    let length = offset.length();
    if length < RING_EPSILON_MM {
        return empty_tube(from);
    }

    let radius_mm = profile.bracing.radius_mm();
    let mut rings = Vec::with_capacity(2);
    push(&mut rings, 0.0, radius_mm);
    push(&mut rings, length, radius_mm);

    Tube {
        top: from,
        direction: offset / length,
        rings,
        sides: profile.facets as usize,
        apex: None,
    }
}

/// The tube of one strut, from `child` down past `parent` into the joint below it.
fn strut(
    tree: &SupportTree,
    child: usize,
    parent: usize,
    profile: &SupportProfile,
    style: Style,
    overlap_mm: Scalar,
) -> Tube {
    let nodes = tree.nodes();
    let (from, to) = (nodes[child].position, nodes[parent].position);
    let offset = to - from;
    let length = offset.length();
    if length < RING_EPSILON_MM {
        return empty_tube(from);
    }
    let direction = offset / length;
    let body_mm = style.body_radius_mm(nodes[parent].radius_mm, profile);

    // The strut stops dead at the joint. What covers the seam is the joint's own ball,
    // which is wider than the strut and swallows its end cap; see `docs/decisions/0044`.
    // A straight run through the joint has no ball, and reaches past it instead.
    let mut rings = Vec::with_capacity(8);
    head(&mut rings, &nodes[child], profile, style);
    push(&mut rings, length + overlap_mm, body_mm);

    Tube {
        top: from,
        direction,
        rings,
        sides: profile.facets as usize,
        apex: apex_of(&nodes[child], direction, profile, style),
    }
}

/// The tube under the root: the body, run all the way down through whatever foot there
/// is so the two overlap.
fn trunk(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Tube {
    let root = tree.root();
    let landing = tree.landing();
    let top = root.position;
    let body_mm = style.body_radius_mm(root.radius_mm, profile);

    // Into the foot far enough to overlap it, and no further: a trunk run all the way to
    // the plate shows through the bottom of the pad.
    let into_foot = if landing.on_model {
        0.0
    } else {
        profile.base_height_mm() * FOOT_BITE
    };

    // Aimed at the foot rather than straight down: a foot dragged out from under its tip
    // leans the trunk: a foot can be put by hand, and then it is not under its tip.
    let offset = trunk_bottom(tree, profile) - top;
    let length = offset.length();
    let direction = if length < RING_EPSILON_MM {
        -Vec3::Z
    } else {
        offset / length
    };

    let mut rings = Vec::with_capacity(8);
    head(&mut rings, root, profile, style);
    if landing.on_model && !style.small {
        foot_tip(&mut rings, length, body_mm, profile);
    } else {
        push(&mut rings, length + into_foot, body_mm);
    }

    Tube {
        top,
        direction,
        rings,
        sides: profile.facets as usize,
        apex: apex_of(root, direction, profile, style),
    }
}

/// Closes a trunk standing on the model with the tip it would have carried, upside down:
/// what touches the part is a contact rather than the whole trunk.
///
/// The cone leaves the trunk at whatever width the trunk is, so there is no rim hanging
/// off its top. `length` runs from the root to the sunk bottom. See `docs/decisions/0124`.
fn foot_tip(rings: &mut Vec<Ring>, length: Scalar, body_mm: Scalar, profile: &SupportProfile) {
    let surface_mm = (length - profile.landing_depth_mm()).max(0.0);
    let taper_mm = profile.top_length_mm().min(surface_mm);
    let neck_mm = profile.top_upper_radius_mm().min(body_mm);
    let contact_mm = profile.contact_radius_mm().min(neck_mm);

    push(rings, surface_mm - taper_mm, body_mm);
    push(rings, surface_mm, neck_mm);
    push(rings, surface_mm, contact_mm);
    push(rings, length, contact_mm);
}

/// Which nodes need a ball over their seam: every node two or more struts meet at, and
/// every node one strut arrives at and turns a corner. A straight run through a node
/// needs none, and the strut above it reaches `JOINT_OVERLAP_MM` past it instead — which
/// is what keeps a tip's own neck from wearing a bulge (ADR 0125).
fn covers(tree: &SupportTree, profile: &SupportProfile) -> Vec<bool> {
    let nodes = tree.nodes();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (child, parent) in tree.struts() {
        children[parent].push(child);
    }

    (0..nodes.len())
        .map(|node| match children[node].as_slice() {
            [] => false,
            [only] => {
                let arriving = nodes[node].position - nodes[*only].position;
                let leaving = under(tree, node, profile) - nodes[node].position;
                arriving
                    .normalize_or_zero()
                    .dot(leaving.normalize_or_zero())
                    < COLLINEAR
            }
            _ => true,
        })
        .collect()
}

/// Where the body under `node` is aimed: the node it hangs from, or the bottom of the
/// trunk when it is the root.
fn under(tree: &SupportTree, node: usize, profile: &SupportProfile) -> Vec3 {
    match tree.nodes()[node].parent {
        Some(parent) => tree.nodes()[parent].position,
        None => trunk_bottom(tree, profile),
    }
}

/// Where the trunk under the root comes to rest, in plate coordinates.
fn trunk_bottom(tree: &SupportTree, profile: &SupportProfile) -> Vec3 {
    let landing = tree.landing();
    Vec3::new(
        landing.base.x,
        landing.base.y,
        landing.pillar_bottom_z(profile),
    )
}

/// The ball that covers a joint, as wide as the trunk leaving it.
///
/// Struts arrive at the joint from different directions and stop dead there. A ball of
/// the trunk's own radius centred on the node contains every one of their end caps, so
/// the joint needs no fitted geometry and cannot invert; see `docs/decisions/0044`.
fn joint(node: &TreeNode, profile: &SupportProfile, style: Style) -> Tube {
    let radius_mm = style.body_radius_mm(node.radius_mm, profile) * JOINT_SWELL;
    let mut rings = Vec::with_capacity(JOINT_RINGS as usize);
    ball(&mut rings, radius_mm, radius_mm, JOINT_RINGS);

    Tube {
        top: node.position,
        direction: -Vec3::Z,
        rings,
        sides: profile.facets as usize,
        apex: Some(node.position + Vec3::Z * radius_mm),
    }
}

/// The foot the trunk stands on, as a solid of its own so that it can have a different
/// number of sides from the support above it. `None` when the support lands on the model.
fn foot(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Option<Tube> {
    let landing = tree.landing();
    if landing.on_model {
        return None;
    }

    let shape = profile.bottom.shape;
    let rim_mm = profile.base_radius_mm();
    let trunk_mm = style.body_radius_mm(tree.root().radius_mm, profile);
    let top_mm = foot_top_radius_mm(profile, trunk_mm);

    let height = profile.base_height_mm();
    let bevel = height * FOOT_BEVEL;
    let top = Vec3::new(
        landing.base.x,
        landing.base.y,
        landing.pillar_bottom_z(profile),
    );

    // The bevel is what a blade gets under: the pad meets the plate on a rim pulled in
    // from its widest point. See `docs/decisions/0044` and `docs/decisions/0130`.
    let mut rings = Vec::with_capacity(3);
    push(&mut rings, 0.0, top_mm);
    if shape.tapers() {
        push(&mut rings, height - bevel, rim_mm);
    }
    push(&mut rings, height, plate_radius_mm(profile, trunk_mm));

    Some(Tube {
        top,
        direction: -Vec3::Z,
        rings,
        sides: shape.sides(profile.facets) as usize,
        apex: None,
    })
}

/// Radius of the ring the pad meets the plate on, millimetres: its rim pulled in by the
/// bevel, and never inside the trunk standing in it.
fn plate_radius_mm(profile: &SupportProfile, trunk_mm: Scalar) -> Scalar {
    let bevel = profile.base_height_mm() * FOOT_BEVEL;
    (profile.base_radius_mm() - bevel).max(trunk_mm)
}

/// The cone that widens the pillar out into the pad it stands on, or `None` when the
/// support lands on the model or has no room for one.
///
/// It rises as far as it widens, which is the forty-five degrees every MSLA slicer draws
/// this joint at, and it is drawn with the pillar's own facets rather than the pad's; see
/// `docs/decisions/0130`.
fn flare(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Option<Tube> {
    let landing = tree.landing();
    let trunk_mm = style.body_radius_mm(tree.root().radius_mm, profile);
    let upper_mm = profile.flare_upper_radius_mm().max(trunk_mm);
    // Inside the pad at every depth it reaches, so it never shows through its side.
    let widest_mm = if profile.bottom.shape.is_round() {
        plate_radius_mm(profile, trunk_mm).min(foot_top_radius_mm(profile, trunk_mm))
    } else {
        skate_half_width_mm(profile)
    };
    let lower_mm = profile.flare_lower_radius_mm().clamp(upper_mm, widest_mm);

    let bottom_z = landing.pillar_bottom_z(profile);
    let room_mm = (tree.root().position.z - bottom_z).max(0.0);
    let rise_mm = (lower_mm - upper_mm).min(room_mm);
    if rise_mm < RING_EPSILON_MM {
        return None;
    }

    let mut rings = Vec::with_capacity(3);
    push(&mut rings, 0.0, upper_mm);
    push(&mut rings, rise_mm, upper_mm + rise_mm);
    push(
        &mut rings,
        rise_mm + profile.base_height_mm() * FOOT_BITE,
        upper_mm + rise_mm,
    );

    Some(Tube {
        top: Vec3::new(landing.base.x, landing.base.y, bottom_z + rise_mm),
        direction: -Vec3::Z,
        rings,
        sides: profile.facets as usize,
        apex: None,
    })
}

/// Radius of the pad's own top ring, millimetres: its rim, or the narrowed one a
/// tapering foot carries the support on. A tapering foot is still wider than the trunk
/// standing in it, or the trunk shows through its side.
fn foot_top_radius_mm(profile: &SupportProfile, trunk_mm: Scalar) -> Scalar {
    let rim_mm = profile.base_radius_mm();
    if profile.bottom.shape.tapers() {
        (rim_mm * FOOT_TAPER).max(trunk_mm)
    } else {
        rim_mm
    }
}

/// The rings a tube starts with: the tip and the widening top segment at a contact, or
/// the joint's own width anywhere else.
fn head(rings: &mut Vec<Ring>, node: &TreeNode, profile: &SupportProfile, style: Style) {
    if !node.is_leaf() {
        push(rings, 0.0, style.body_radius_mm(node.radius_mm, profile));
        return;
    }

    let contact_mm = profile.contact_radius_mm();
    if profile.tip.shape == ContactShape::Sphere {
        ball(rings, style.bite_mm(profile), contact_mm, SPHERE_RINGS);
    } else {
        push(rings, 0.0, contact_mm);
    }

    push(rings, 0.0, profile.top_upper_radius_mm());
    push(
        rings,
        profile.top_length_mm(),
        style.body_radius_mm(profile.top_lower_radius_mm(), profile),
    );
    // The top segment ends where the body begins: the two are separate measurements, so
    // the body keeps its own width rather than carrying the segment's on down.
    push(
        rings,
        profile.top_length_mm(),
        style.body_radius_mm(node.radius_mm, profile),
    );
}

/// Where the tip pokes into the model, for a tube that starts at a contact.
///
/// It goes back up the tube's own axis, which at a tip is the normal of the face it
/// holds, so the bite drives into the surface square on (ADR 0125). A cone's apex and a
/// sphere's upper pole are the same point, `bite_mm` in. A plane tip bites nothing, so
/// the sweep closes it with the flat disc it gets when there is no apex at all.
fn apex_of(
    node: &TreeNode,
    direction: Vec3,
    profile: &SupportProfile,
    style: Style,
) -> Option<Vec3> {
    if !node.is_leaf() || profile.tip.shape == ContactShape::Plane {
        return None;
    }
    Some(node.position - direction * style.bite_mm(profile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SupportPoint;
    use crate::tests::{box_mesh, meshed, profile, ramp, trees_on};
    use core_geometry::{Mesh, Transform};
    use core_geometry::{Vec2, diagnose, signed_volume};
    use printer_profiles::ContactShape;
    use std::f32::consts::PI;

    /// One column standing on the plate under a contact `height` millimetres up, with
    /// nothing for it to merge with.
    fn column_on_plate(height: Scalar, profile: &SupportProfile) -> Vec<SupportTree> {
        trees_on(
            &[SupportPoint::new(Vec3::new(10.0, 10.0, height))],
            &Mesh::default(),
            Transform::default(),
            Profiles::single(profile),
        )
    }

    #[test]
    fn a_column_is_closed_and_wound_outwards() {
        let profile = profile();
        let mesh = meshed(&column_on_plate(20.0, &profile), Profiles::single(&profile));

        let diagnostics = diagnose(&mesh);
        assert_eq!(
            diagnostics.boundary_edges, 0,
            "a printable column has no open edges"
        );
        assert_eq!(diagnostics.non_manifold_edges, 0);
        assert_eq!(diagnostics.degenerate_faces, 0);
        assert!(
            signed_volume(&mesh) > 0.0,
            "outward winding gives a positive volume"
        );
    }

    #[test]
    fn each_group_is_meshed_apart_and_together_they_are_the_whole() {
        let profiles = [profile(), SupportProfile::heavy()];
        let profiles = Profiles::new(&profiles).expect("two groups");
        let points = [
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(40.0, 10.0, 20.0)).in_group(1),
        ];
        let model = Mesh::default();
        let bvh = core_geometry::Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let trees = trees_on(&points, &model, Transform::default(), profiles);

        let groups = mesh_groups(&trees, &placed, profiles);
        let whole = mesh_trees(&trees, &placed, profiles);
        assert_eq!(groups.len(), 2);
        assert!(
            groups.iter().all(|mesh| !mesh.is_empty()),
            "one column each"
        );
        assert_eq!(
            groups.iter().map(|mesh| mesh.faces.len()).sum::<usize>(),
            whole.faces.len(),
            "split by group, nothing added and nothing lost"
        );
    }

    #[test]
    fn a_column_landing_on_the_model_is_closed_too() {
        let profile = profile();
        let cube = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let trees = trees_on(
            &[SupportPoint::new(Vec3::new(5.0, 5.0, 25.0))],
            &cube,
            Transform::default(),
            Profiles::single(&profile),
        );
        assert!(trees[0].landing().on_model);

        let diagnostics = diagnose(&meshed(&trees, Profiles::single(&profile)));
        assert_eq!(diagnostics.boundary_edges, 0);
        assert_eq!(diagnostics.non_manifold_edges, 0);
        assert_eq!(diagnostics.degenerate_faces, 0);
    }

    #[test]
    fn a_column_shorter_than_its_neck_is_still_closed() {
        let profile = profile();
        // Barely taller than the foot, so the neck is squeezed down to nothing.
        let trees = column_on_plate(profile.base_height_mm() + 0.3, &profile);
        assert_eq!(trees.len(), 1);

        let diagnostics = diagnose(&meshed(&trees, Profiles::single(&profile)));
        assert_eq!(diagnostics.boundary_edges, 0);
        assert_eq!(diagnostics.degenerate_faces, 0);
    }

    #[test]
    fn the_volume_matches_the_cones_and_cylinders_it_is_made_of() {
        let profile = SupportProfile {
            // A round number of facets does not change the analysis, but many of them
            // brings the polygon's area close to the circle's.
            facets: 512,
            ..profile()
        };
        let height = 20.0;
        let mesh = meshed(
            &column_on_plate(height, &profile),
            Profiles::single(&profile),
        );

        let contact_r = profile.contact_radius_mm();
        let pillar_r = profile.pillar_radius_mm();
        let base_r = profile.base_radius_mm();
        let foot_h = profile.base_height_mm();
        let bevel = foot_h * FOOT_BEVEL;
        let rim_r = base_r - bevel;
        let trunk_bottom = profile.base_height_mm() - foot_h * FOOT_BITE;

        // Tip cone, neck frustum, pillar cylinder, the flare into the pad and then the
        // pad itself, one frustum in to the rim it meets the plate on. The pillar reaches
        // into the foot far enough to overlap it, and so does the flare, because each is
        // a solid of its own; see `docs/decisions/0041`. That much of them is counted
        // twice, and the closed form says so too.
        let tip = PI * contact_r * contact_r * profile.contact_depth_mm() / 3.0;
        let neck = PI * profile.top_length_mm() / 3.0
            * (contact_r * contact_r + contact_r * pillar_r + pillar_r * pillar_r);
        // The neck runs straight into the pillar, so the two tubes overlap rather than
        // meeting on a plane, and that overlap is counted twice as well.
        let pillar = PI * pillar_r * pillar_r * (height - profile.top_length_mm() - trunk_bottom);
        let seam = PI * pillar_r * pillar_r * JOINT_OVERLAP_MM;
        let pad = PI * foot_h / 3.0 * (base_r * base_r + base_r * rim_r + rim_r * rim_r);
        // The flare rises as far as it widens, and sinks into the pad as far as the
        // pillar does.
        let flare_top_r = profile.flare_upper_radius_mm().max(pillar_r);
        let flare_bottom_r = profile.flare_lower_radius_mm();
        let rise = flare_bottom_r - flare_top_r;
        let flare = PI * rise / 3.0
            * (flare_top_r * flare_top_r
                + flare_top_r * flare_bottom_r
                + flare_bottom_r * flare_bottom_r)
            + PI * flare_bottom_r * flare_bottom_r * foot_h * FOOT_BITE;
        let expected = tip + neck + pillar + seam + pad + flare;

        let volume = signed_volume(&mesh);
        assert!(
            (volume - expected).abs() / expected < 0.001,
            "expected {expected} mm3 from a cone, a frustum, a cylinder and a disc, got {volume}"
        );
    }

    #[test]
    fn the_pillar_flares_into_its_pad_at_forty_five_degrees() {
        let profile = profile();
        let trees = column_on_plate(20.0, &profile);
        let style = Style::of(&trees[0], &profile);
        let tube = flare(&trees[0], &profile, style).expect("a column on the plate flares");

        let (top, bottom) = (tube.rings[0], tube.rings[1]);
        let widens = bottom.radius_mm - top.radius_mm;
        let rises = bottom.along_mm - top.along_mm;
        assert!(
            (widens - rises).abs() < 1e-5,
            "forty-five degrees is a flare that rises as far as it widens: {rises} mm up              against {widens} mm out"
        );
        assert!(
            (tube.top.z - (profile.base_height_mm() + rises)).abs() < 1e-5,
            "the flare stands on the top of the pad"
        );
    }

    #[test]
    fn a_flare_never_rises_above_the_pillar_it_widens() {
        let profile = profile();
        // Barely taller than its own foot, so there is next to no pillar to widen in.
        let trees = column_on_plate(profile.base_height_mm() + 0.3, &profile);
        let style = Style::of(&trees[0], &profile);
        let root_z = trees[0].root().position.z;
        if let Some(tube) = flare(&trees[0], &profile, style) {
            assert!(
                tube.top.z <= root_z + 1e-5,
                "a flare taller than the pillar would stand out of the top of it: \
                 {} against {root_z}",
                tube.top.z
            );
        }
        assert_eq!(
            diagnose(&meshed(&trees, Profiles::single(&profile))).boundary_edges,
            0
        );
    }

    #[test]
    fn a_straight_pad_is_one_trapezoid_narrowing_to_the_plate() {
        let profile = profile();
        let trees = column_on_plate(20.0, &profile);
        let style = Style::of(&trees[0], &profile);
        let tube = foot(&trees[0], &profile, style).expect("a column on the plate has a pad");

        assert_eq!(
            tube.rings.len(),
            2,
            "a rim at the top and the plate at the bottom"
        );
        assert!(
            tube.rings[1].radius_mm < tube.rings[0].radius_mm,
            "the rim a blade gets under is the narrow one; see `docs/decisions/0044`"
        );
    }

    #[test]
    fn every_column_adds_the_same_number_of_faces() {
        let profile = profile();
        let one = meshed(&column_on_plate(20.0, &profile), Profiles::single(&profile));

        let two = trees_on(
            &[
                SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
                SupportPoint::new(Vec3::new(40.0, 10.0, 20.0)),
            ],
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile),
        );
        assert_eq!(two.len(), 2, "thirty millimetres apart is too far to merge");
        let two = meshed(&two, Profiles::single(&profile));

        assert_eq!(two.faces.len(), one.faces.len() * 2);
        assert_eq!(two.vertices.len(), one.vertices.len() * 2);
        assert_eq!(diagnose(&two).boundary_edges, 0);
    }

    #[test]
    fn a_branched_pair_is_closed_and_solid() {
        let profile = profile();
        let trees = trees_on(
            &[
                SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
                SupportPoint::new(Vec3::new(14.0, 10.0, 20.0)),
            ],
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile),
        );
        assert_eq!(trees.len(), 1);

        let mesh = meshed(&trees, Profiles::single(&profile));
        let diagnostics = diagnose(&mesh);
        assert_eq!(
            diagnostics.boundary_edges, 0,
            "every tube of a branch is closed on its own"
        );
        assert_eq!(diagnostics.non_manifold_edges, 0);
        assert_eq!(diagnostics.degenerate_faces, 0);
        assert!(signed_volume(&mesh) > 0.0);
    }

    #[test]
    fn branching_uses_less_material_than_two_columns() {
        let profile = profile();
        let straight = SupportProfile {
            branching: printer_profiles::Branching {
                enabled: false,
                ..profile.branching
            },
            ..profile.clone()
        };
        let points = [
            SupportPoint::new(Vec3::new(10.0, 10.0, 40.0)),
            SupportPoint::new(Vec3::new(14.0, 10.0, 40.0)),
        ];

        let merged = meshed(
            &trees_on(
                &points,
                &Mesh::default(),
                Transform::default(),
                Profiles::single(&profile),
            ),
            Profiles::single(&profile),
        );
        let apart = meshed(
            &trees_on(
                &points,
                &Mesh::default(),
                Transform::default(),
                Profiles::single(&straight),
            ),
            Profiles::single(&straight),
        );

        assert!(
            signed_volume(&merged) < signed_volume(&apart),
            "one trunk under two tips holds less resin than two full columns: {} against {}",
            signed_volume(&merged),
            signed_volume(&apart)
        );
    }

    #[test]
    fn a_tip_on_a_sloping_face_bites_into_it_along_the_normal() {
        use core_geometry::Quat;
        use std::f32::consts::PI;

        // The test ramp turned over, so the face that looked up is now an overhang.
        let model = ramp(4.0, 10.0);
        let transform = Transform {
            rotation: Quat::from_rotation_x(PI),
            translation: Vec3::new(0.0, 0.0, 30.0),
            ..Transform::default()
        };
        let profile = profile();
        let contact = transform
            .to_matrix()
            .transform_point3(Vec3::new(2.0, 0.0, 5.0));
        let trees = trees_on(
            &[SupportPoint::new(Vec3::new(2.0, 0.0, 5.0))],
            &model,
            transform,
            Profiles::single(&profile),
        );

        let normal = Vec3::new(-10.0, 0.0, -4.0).normalize();
        let mesh = meshed(&trees, Profiles::single(&profile));
        let apex = mesh
            .vertices
            .iter()
            .copied()
            .max_by(|a, b| {
                (*a - contact)
                    .dot(-normal)
                    .total_cmp(&(*b - contact).dot(-normal))
            })
            .expect("the support was meshed");
        assert!(
            apex.abs_diff_eq(contact - normal * profile.contact_depth_mm(), 1e-3),
            "the bite drives square into the face, got {apex} against {}",
            contact - normal * profile.contact_depth_mm()
        );
    }

    #[test]
    fn a_column_that_runs_straight_through_its_neck_wears_no_ball() {
        let profile = profile();
        let trees = column_on_plate(20.0, &profile);
        let covered = covers(&trees[0], &profile);
        assert!(
            covered.iter().all(|covered| !covered),
            "a tip square under a flat face and the pillar under it are one straight run"
        );

        let mesh = meshed(&trees, Profiles::single(&profile));
        let widest = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.z > profile.base_height_mm() + 1.0)
            .map(|vertex| Vec2::new(vertex.x - 10.0, vertex.y - 10.0).length())
            .fold(0.0, Scalar::max);
        assert!(
            widest <= profile.pillar_radius_mm() + 1e-4,
            "nothing above the foot is wider than the pillar: {widest} mm"
        );
    }

    #[test]
    fn a_joint_is_covered_by_a_ball_that_swallows_the_struts_meeting_there() {
        let profile = profile();
        let trees = trees_on(
            &[
                SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
                SupportPoint::new(Vec3::new(14.0, 10.0, 20.0)),
            ],
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile),
        );
        let tree = &trees[0];
        let style = Style::of(tree, &profile);
        let covered = covers(tree, &profile);
        let (child, parent) = tree
            .struts()
            .find(|(_, parent)| covered[*parent])
            .expect("the two necks meet at a joint");
        let tube = strut(tree, child, parent, &profile, style, 0.0);

        let reaches = (tree.nodes()[parent].position - tree.nodes()[child].position).length();
        let stops = tube.rings.last().expect("a strut has rings").along_mm;
        assert!(
            (stops - reaches).abs() < 1e-4,
            "a strut stops dead at the joint it meets, at {reaches} mm, got {stops}"
        );

        assert_eq!(
            covered.iter().filter(|covered| **covered).count(),
            3,
            "the two necks turn a corner into the lean, and the struts meet at a third"
        );
        let ball = joint(&tree.nodes()[parent], &profile, style);
        let widest = ball
            .rings
            .iter()
            .map(|ring| ring.radius_mm)
            .fold(0.0, Scalar::max);
        assert!(
            widest >= tube.rings.last().expect("rings").radius_mm - 1e-4,
            "the ball has to be at least as wide as the strut whose end it hides"
        );
    }

    #[test]
    fn no_trees_make_no_mesh() {
        assert!(meshed(&[], Profiles::single(&profile())).is_empty());
    }

    /// A column of `profile`, meshed.
    fn one_column(profile: &SupportProfile) -> Mesh {
        meshed(&column_on_plate(20.0, profile), Profiles::single(profile))
    }

    /// The highest point the mesh reaches, which is how far the tip bit into the model.
    fn ceiling(mesh: &Mesh) -> Scalar {
        mesh.vertices
            .iter()
            .map(|vertex| vertex.z)
            .fold(Scalar::NEG_INFINITY, Scalar::max)
    }

    /// Vertices sitting on the plate, which is the foot's own ring plus the trunk's.
    fn on_the_plate(mesh: &Mesh) -> usize {
        mesh.vertices
            .iter()
            .filter(|vertex| vertex.z.abs() < 1e-4)
            .count()
    }

    fn with_tip(shape: ContactShape) -> SupportProfile {
        let mut profile = profile();
        profile.tip.shape = shape;
        profile
    }

    fn with_foot(shape: printer_profiles::PlatformShape) -> SupportProfile {
        let mut profile = profile();
        profile.bottom.shape = shape;
        profile
    }

    #[test]
    fn every_tip_shape_meshes_into_a_closed_solid() {
        for shape in [
            ContactShape::Cone,
            ContactShape::Sphere,
            ContactShape::Plane,
        ] {
            let profile = with_tip(shape);
            let diagnostics = diagnose(&one_column(&profile));
            assert_eq!(
                diagnostics.boundary_edges, 0,
                "a {shape:?} tip left the column open"
            );
            assert_eq!(diagnostics.non_manifold_edges, 0, "{shape:?}");
            assert_eq!(diagnostics.degenerate_faces, 0, "{shape:?}");
        }
    }

    #[test]
    fn a_plane_tip_bites_nothing_and_a_cone_tip_bites_its_depth() {
        let flat = with_tip(ContactShape::Plane);
        let point = with_tip(ContactShape::Cone);

        assert!(
            (ceiling(&one_column(&flat)) - 20.0).abs() < 1e-4,
            "a plane tip stops at the surface it is pressed against"
        );
        assert!(
            (ceiling(&one_column(&point)) - (20.0 + point.contact_depth_mm())).abs() < 1e-4,
            "a cone tip reaches its own depth into the model"
        );
    }

    /// How wide the mesh is above `z`, measured from the support's own axis.
    fn widest_above(mesh: &Mesh, axis: Vec3, z: Scalar) -> Scalar {
        mesh.vertices
            .iter()
            .filter(|vertex| vertex.z > z + 1e-3)
            .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
            .fold(0.0, Scalar::max)
    }

    #[test]
    fn a_sphere_tip_is_wide_inside_the_model_where_a_cone_has_narrowed_to_a_point() {
        let ball = with_tip(ContactShape::Sphere);
        let point = with_tip(ContactShape::Cone);
        let axis = Vec3::new(10.0, 10.0, 0.0);

        assert!(
            (ceiling(&one_column(&ball)) - (20.0 + ball.contact_depth_mm())).abs() < 1e-4,
            "the top of the ball is where the cone's apex would have been"
        );
        assert!(
            widest_above(&one_column(&point), axis, 20.0) < 1e-3,
            "a cone is a single point above the contact"
        );
        assert!(
            widest_above(&one_column(&ball), axis, 20.0) > ball.contact_radius_mm() * 0.8,
            "a ball is still most of its own width inside the model, which is the grip \
             it is chosen for"
        );
    }

    #[test]
    fn a_cube_foot_stands_on_four_corners_and_a_cylinder_on_every_facet() {
        use printer_profiles::PlatformShape;
        let cube = with_foot(PlatformShape::Cube);
        let round = with_foot(PlatformShape::Cylinder);

        let difference = on_the_plate(&one_column(&round)) - on_the_plate(&one_column(&cube));
        assert_eq!(
            difference,
            round.facets as usize - 4,
            "a cube foot has four corners where a {}-sided one has {}",
            round.facets,
            round.facets
        );
    }

    #[test]
    fn every_foot_shape_meshes_into_a_closed_solid() {
        use printer_profiles::PlatformShape;
        for shape in [
            PlatformShape::Cylinder,
            PlatformShape::Cone,
            PlatformShape::Prism,
            PlatformShape::Cube,
        ] {
            let diagnostics = diagnose(&one_column(&with_foot(shape)));
            assert_eq!(
                diagnostics.boundary_edges, 0,
                "a {shape:?} foot left the support open"
            );
            assert_eq!(diagnostics.degenerate_faces, 0, "{shape:?}");
        }
    }

    #[test]
    fn a_tapering_foot_holds_less_than_a_straight_one_of_the_same_rim() {
        use printer_profiles::PlatformShape;
        let tapered = one_column(&with_foot(PlatformShape::Cone));
        let straight = one_column(&with_foot(PlatformShape::Cylinder));
        assert!(
            signed_volume(&tapered) < signed_volume(&straight),
            "a foot that narrows towards the support is less resin than a full pad"
        );
    }

    /// A support standing on the model rather than on the plate: a shelf 10 mm up with a
    /// contact 10 mm above that.
    fn model_to_model(profile: &SupportProfile) -> Vec<SupportTree> {
        let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        trees_on(
            &[SupportPoint::new(Vec3::new(5.0, 5.0, 20.0))],
            &shelf,
            Transform::default(),
            Profiles::single(profile),
        )
    }

    #[test]
    fn a_support_between_two_surfaces_is_thinned_to_the_small_pillar() {
        let profile = profile();
        let mut regular = profile.clone();
        regular.small_pillar.enabled = false;

        let trees = model_to_model(&profile);
        assert!(trees[0].landing().on_model, "it stands on the shelf");

        let thin = signed_volume(&meshed(&trees, Profiles::single(&profile)));
        let thick = signed_volume(&meshed(
            &model_to_model(&regular),
            Profiles::single(&regular),
        ));
        assert!(
            thin < thick,
            "a model-to-model strut uses the small pillar's own width: {thin} mm3 \
             against {thick} mm3"
        );
        assert_eq!(
            diagnose(&meshed(&trees, Profiles::single(&profile))).boundary_edges,
            0
        );
    }

    /// How far from `axis` the mesh reaches at or below `z`, which under a support
    /// standing on the model is the plug driven into the surface.
    fn widest_below(mesh: &Mesh, axis: Vec3, z: Scalar) -> Scalar {
        mesh.vertices
            .iter()
            .filter(|vertex| vertex.z < z + 1e-3)
            .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
            .fold(0.0, Scalar::max)
    }

    #[test]
    fn a_support_standing_on_the_model_touches_it_with_a_contact_not_a_trunk() {
        let mut profile = profile();
        profile.small_pillar.enabled = false;
        let trees = model_to_model(&profile);
        assert!(trees[0].landing().on_model, "it stands on the shelf");

        let mesh = meshed(&trees, Profiles::single(&profile));
        let widest = widest_below(&mesh, Vec3::new(5.0, 5.0, 0.0), 10.0);
        assert!(
            widest <= profile.contact_radius_mm() + 1e-3,
            "what enters the shelf is the tip's own contact: {widest} mm against              {} mm",
            profile.contact_radius_mm()
        );
        assert!(
            widest < profile.pillar_radius_mm(),
            "the trunk is not cured into the part at its full width"
        );
        assert_eq!(diagnose(&mesh).boundary_edges, 0);
    }

    /// Three tips over the same shelf, close enough to merge into one trunk wider than
    /// the top segment it ends in.
    fn merged_on_the_model(profile: &SupportProfile) -> Vec<SupportTree> {
        let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let points = [
            SupportPoint::new(Vec3::new(4.0, 5.0, 20.0)),
            SupportPoint::new(Vec3::new(6.0, 5.0, 20.0)),
            SupportPoint::new(Vec3::new(5.0, 6.5, 20.0)),
        ];
        trees_on(
            &points,
            &shelf,
            Transform::default(),
            Profiles::single(profile),
        )
    }

    #[test]
    fn the_cone_under_a_trunk_leaves_it_at_the_trunk_s_own_width() {
        let profile = profile();
        let trees = merged_on_the_model(&profile);
        let [tree] = &trees[..] else {
            panic!("three tips this close merge into one trunk");
        };
        assert!(tree.landing().on_model, "it stands on the shelf");
        let trunk_mm = tree.root().radius_mm;
        assert!(
            trunk_mm > profile.top_lower_radius_mm(),
            "a merged trunk is wider than the segment it ends in, which is the case the              rim used to hang off"
        );

        let mesh = meshed(&trees, Profiles::single(&profile));
        let axis = tree.landing().base;
        let widths: Vec<Scalar> = mesh
            .vertices
            .iter()
            .filter(|vertex| (vertex.z - (10.0 + profile.top_length_mm())).abs() < 1e-3)
            .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
            .collect();
        assert!(!widths.is_empty(), "the taper starts one top segment up");
        for width in widths {
            assert!(
                (width - trunk_mm).abs() < 1e-3,
                "the cone starts at the trunk's width, not at a rim under it: {width} mm                  against {trunk_mm} mm"
            );
        }
    }

    #[test]
    fn a_profile_that_refuses_the_model_leaves_no_support_standing_on_it() {
        let mut profile = profile();
        profile.land_on_model = false;
        let trees = model_to_model(&profile);
        assert!(
            trees.iter().all(|tree| !tree.landing().on_model),
            "a support over the shelf leans for the plate or is dropped"
        );
    }

    #[test]
    fn a_plate_with_skates_a_raft_and_bracing_is_still_closed() {
        use printer_profiles::{Bracing, PlatformShape, Raft};
        let mut profile = profile();
        profile.bottom.shape = PlatformShape::Skate;
        profile.branching.enabled = false;
        profile.raft = Raft {
            enabled: true,
            ..Raft::default()
        };
        profile.bracing = Bracing {
            enabled: true,
            start_height_mm: 5.0,
            rise_mm: 5.0,
            ..Bracing::default()
        };

        let points: Vec<SupportPoint> = [(10.0, 10.0), (18.0, 10.0), (14.0, 18.0)]
            .into_iter()
            .map(|(x, y)| SupportPoint::new(Vec3::new(x, y, 30.0)))
            .collect();
        let trees = trees_on(
            &points,
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile),
        );
        assert_eq!(trees.len(), 3);

        let mesh = meshed(&trees, Profiles::single(&profile));
        let diagnostics = diagnose(&mesh);
        assert_eq!(
            diagnostics.boundary_edges, 0,
            "every piece closes on its own"
        );
        assert_eq!(diagnostics.non_manifold_edges, 0);
        assert_eq!(diagnostics.degenerate_faces, 0);
        assert!(signed_volume(&mesh) > 0.0);

        let bounds = mesh.aabb().expect("the plate has geometry");
        assert!(
            bounds.mins.z.abs() < 1e-4,
            "the raft sits on the plate, got {}",
            bounds.mins.z
        );
    }

    #[test]
    fn nothing_of_a_support_shows_through_the_bottom_of_its_foot() {
        let profile = profile();
        let mesh = one_column(&profile);
        let rim = profile.base_radius_mm() - profile.base_height_mm() * FOOT_BEVEL;
        let axis = Vec3::new(10.0, 10.0, 0.0);

        for vertex in &mesh.vertices {
            assert!(
                vertex.z > -1e-4,
                "nothing reaches under the plate, got {vertex}"
            );
            if vertex.z < 1e-4 {
                let reach = Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length();
                assert!(
                    reach <= rim + 1e-3 || reach < 1e-4,
                    "only the foot's own rim touches the plate, got {reach} mm out"
                );
            }
        }

        let trunk_on_the_plate = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.z < 1e-4)
            .filter(|vertex| {
                let reach = Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length();
                (reach - profile.pillar_radius_mm()).abs() < 1e-3
            })
            .count();
        assert_eq!(
            trunk_on_the_plate, 0,
            "the trunk stops inside the foot, it does not show through its sole"
        );
    }

    /// A column whose body is wider than the segment above it, which is the case the
    /// two measurements only ever differ in.
    fn stepped() -> SupportProfile {
        let mut profile = profile();
        profile.top.lower_diameter_mm = 1.0;
        profile.middle.diameter_mm = 2.4;
        profile
    }

    #[test]
    fn the_body_is_as_wide_as_the_middle_segment_not_as_the_segment_above_it() {
        let profile = stepped();
        let mesh = one_column(&profile);
        let axis = Vec3::new(10.0, 10.0, 0.0);

        // The body is a plain tube, so its width is the ring where it leaves the top
        // segment: that far down the column, nothing is narrower.
        let shoulder = 20.0 - profile.top_length_mm();
        let widest = mesh
            .vertices
            .iter()
            .filter(|vertex| (vertex.z - shoulder).abs() < 1e-3)
            .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
            .fold(0.0, Scalar::max);
        assert!(
            (widest - profile.pillar_radius_mm()).abs() < 1e-3,
            "expected a body of {} mm, got {widest}",
            profile.pillar_radius_mm()
        );
        assert!(
            widest_above(&mesh, axis, shoulder) <= profile.top_lower_radius_mm() + 1e-3,
            "the segment above the body keeps its own width"
        );
    }

    #[test]
    fn a_stepped_column_is_still_closed() {
        let diagnostics = diagnose(&one_column(&stepped()));
        assert_eq!(diagnostics.boundary_edges, 0);
        assert_eq!(diagnostics.non_manifold_edges, 0);
        assert_eq!(diagnostics.degenerate_faces, 0);
    }

    #[test]
    fn a_support_that_reaches_the_plate_is_never_thinned() {
        let profile = profile();
        let mut regular = profile.clone();
        regular.small_pillar.enabled = false;

        let thin = signed_volume(&one_column(&profile));
        let thick = signed_volume(&one_column(&regular));
        assert!(
            (thin - thick).abs() < 1e-3,
            "the small pillar is for struts between surfaces, not for columns"
        );
    }
}
