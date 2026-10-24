use crate::placed::Placed;
use core_geometry::{Mesh, Scalar, Vec3};
use printer_profiles::SupportProfile;

use crate::Landing;
use crate::base::{braces, skate, toe_of};
use crate::buttress::buttresses;
use crate::group::Profiles;
use crate::raft::raft;
use crate::tree::SupportTree;

mod tube;

use tube::{RING_EPSILON_MM, Ring, Tube, empty_tube, push, round_off, sweep};

mod foot;
mod head;
mod sound;
pub(crate) use foot::footing_radius_mm;
pub use foot::{FOOT_BEVEL, FOOT_TAPER};
use foot::{FOOT_BITE, flare, foot, foot_tip, trunk_base_mm, trunk_bottom};
pub(crate) use head::ground_normal;
pub(crate) use head::mesh_head;
use head::{apex_of, head};
pub(crate) use sound::{Fault, bodies, faults, sound};

/// Which set of measurements a support is built from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Style {
    /// The thin model-to-model strut of `[small_pillar]` rather than a regular support.
    small: bool,
    /// The deepest a tip may bite, millimetres: half the wall behind its contact, so a
    /// tip on a thin edge is not pushed out through the far side.
    bite_limit_mm: Scalar,
    /// The outward normal of the face a tip sits on, where it sits on one.
    face: Option<Vec3>,
    /// The outward normal of the face of the model a trunk stands on, where it does.
    ground: Option<Vec3>,
}

impl Style {
    /// A support is small when it runs between two parts of the model and carries one tip; a trunk
    /// under several tips keeps its own width.
    fn of(tree: &SupportTree, profile: &SupportProfile) -> Self {
        Self {
            small: profile.small_pillar.enabled
                && tree.landing().on_model
                && !tree.landing().on_support
                && tree.tip_count() == 1
                && run_mm(tree) <= longest_run_mm(profile),
            bite_limit_mm: Scalar::INFINITY,
            face: None,
            ground: None,
        }
    }

    /// How far the tip bites into the model, millimetres.
    fn bite_mm(self, profile: &SupportProfile) -> Scalar {
        let depth_mm = if self.small {
            profile.small_pillar.upper_depth_mm
        } else {
            profile.tip.contact_depth_mm
        };
        depth_mm.min(self.bite_limit_mm)
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

/// How many of its own diameters long a small pillar may run: a resin wire about 35 times as long
/// as it is wide starts to wave as it prints, and the peel pulls on it too (ADR 0348).
const SLENDER: Scalar = 30.0;

/// The longest run a small pillar is built thin over, millimetres: the profile's, held to
/// [`SLENDER`] of its diameters so a thin strut stays stiff enough to print.
pub(crate) fn longest_run_mm(profile: &SupportProfile) -> Scalar {
    profile
        .small_pillar
        .max_length_mm
        .min(SLENDER * profile.small_pillar.diameter_mm)
}

/// How far a one-tip tree runs from its contact to where it stands, along its struts,
/// millimetres.
pub(crate) fn run_mm(tree: &SupportTree) -> Scalar {
    let nodes = tree.nodes();
    let struts: Scalar = tree
        .struts()
        .map(|(child, parent)| nodes[child].position.distance(nodes[parent].position))
        .sum();
    struts + tree.root().position.distance(tree.landing().base)
}

/// Whether `tree` is built as the thin strut of `[small_pillar]` and not to the width of
/// its own nodes: what it can carry, and what it can stand on, go by that width.
pub(crate) fn is_small(tree: &SupportTree, profile: &SupportProfile) -> bool {
    Style::of(tree, profile).small
}

/// Welds every support into one plate-coordinate mesh of overlapping closed tubes the winding
/// fill unions; `placed` is kept clear of (ADR 0008, 0041, 0094, 0252, 0325).
pub fn mesh_trees(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Mesh {
    let trees = &propped(trees, placed, profiles);
    let mut mesh = Mesh::default();
    for tree in trees {
        mesh_tree(
            &mut Slots(std::slice::from_mut(&mut mesh)),
            tree,
            profiles,
            placed,
        );
    }
    for (from, to, radius_mm, group) in braces(trees, placed, profiles) {
        sweep(&mut mesh, &brace(from, to, radius_mm, profiles.of(group)));
    }
    mesh_raft(&mut mesh, trees, placed, profiles);
    mesh
}

/// The same supports as [`mesh_trees`], one mesh per group in group order for drawing apart;
/// a strut goes with its heaviest head, a trunk and foot with the tree, a brace with its group.
pub fn mesh_groups(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Vec<Mesh> {
    let trees = &propped(trees, placed, profiles);
    let mut meshes = vec![Mesh::default(); profiles.groups().count()];
    for tree in trees {
        mesh_tree(&mut Slots(&mut meshes), tree, profiles, placed);
    }
    for (from, to, radius_mm, group) in braces(trees, placed, profiles) {
        let slot = usize::from(group);
        let slot = if slot < meshes.len() { slot } else { 0 };
        sweep(
            &mut meshes[slot],
            &brace(from, to, radius_mm, profiles.of(group)),
        );
    }
    mesh_raft(&mut meshes[0], trees, placed, profiles);
    meshes
}

/// `trees` and the buttresses they want, which are meshed, braced and stood on the raft
/// like any other support but never kept: they follow the trunks they prop.
fn propped(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Vec<SupportTree> {
    let mut all = trees.to_vec();
    all.extend(buttresses(trees, placed, profiles));
    all
}

/// One support on its own, without the braces and the raft it would share with others.
pub(crate) fn mesh_alone(tree: &SupportTree, profiles: Profiles, placed: &Placed) -> Mesh {
    let mut mesh = Mesh::default();
    mesh_tree(
        &mut Slots(std::slice::from_mut(&mut mesh)),
        tree,
        profiles,
        placed,
    );
    mesh
}

/// The meshes a tree's bodies go into, one per group, or one for every group.
struct Slots<'a>(&'a mut [Mesh]);

impl Slots<'_> {
    /// The mesh `group` is drawn in: group 0's when there is no slot of its own.
    fn of(&mut self, group: u16) -> &mut Mesh {
        let slot = usize::from(group);
        let slot = if slot < self.0.len() { slot } else { 0 };
        &mut self.0[slot]
    }
}

fn mesh_tree(slots: &mut Slots, tree: &SupportTree, profiles: Profiles, placed: &Placed) {
    for body in bodies(tree, profiles, placed) {
        sweep(slots.of(body.group), &body.tube);
    }
    stand_on(slots.of(tree.group()), tree, profiles.of(tree.group()));
}

fn mesh_raft(mesh: &mut Mesh, trees: &[SupportTree], placed: &Placed, profiles: Profiles) {
    if let Some(slab) = raft(trees, placed, profiles) {
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

/// Whatever the tree stands on: the flare, then a round foot as rings or a skate; nothing on the
/// model, no foot on a raft, which the flare stands on instead (ADR 0266).
fn stand_on(mesh: &mut Mesh, tree: &SupportTree, profile: &SupportProfile) {
    if tree.landing().on_model {
        return;
    }
    let style = Style::of(tree, profile);
    if let Some(tube) = flare(tree, profile, style) {
        sweep(mesh, &tube);
    }
    if profile.raft.enabled {
        return;
    }
    if profile.bottom.shape.is_round() {
        if let Some(tube) = foot(tree, profile, style) {
            let first = mesh.vertices.len();
            sweep(mesh, &tube);
            if let Some(cut) = tree.landing().pad_cut {
                let centre = tree.landing().pad_centre();
                for vertex in &mut mesh.vertices[first..] {
                    *vertex = cut.pull_in(centre, *vertex);
                }
            }
        }
        return;
    }

    let base = tree.landing().base;
    skate(mesh, Vec3::new(base.x, base.y, 0.0), toe_of(tree), profile);
}

/// One brace of `radius_mm`, as a tube between two points on neighbouring trunks.
fn brace(from: Vec3, to: Vec3, radius_mm: Scalar, profile: &SupportProfile) -> Tube {
    let offset = to - from;
    let length = offset.length();
    if length < RING_EPSILON_MM {
        return empty_tube(from);
    }

    let mut rings = Vec::with_capacity(2);
    push(&mut rings, 0.0, radius_mm);
    push(&mut rings, length, radius_mm);

    Tube {
        top: from,
        direction: offset / length,
        rings,
        sides: profile.facets as usize,
        apex: None,
        tail: None,
    }
}

/// The tube of one strut, from `child` down past `parent` into the joint below it.
fn strut(
    tree: &SupportTree,
    child: usize,
    parent: usize,
    profile: &SupportProfile,
    style: Style,
) -> Tube {
    let nodes = tree.nodes();
    let (from, to) = (nodes[child].position, nodes[parent].position);
    let offset = to - from;
    let length = offset.length();
    if length < RING_EPSILON_MM {
        return empty_tube(from);
    }
    let direction = offset / length;
    let body_mm = style.body_radius_mm(nodes[child].radius_mm, profile);

    // One width down to a dome on the joint, under which the next, no narrower, tube
    // starts: every end is inside what it meets. See `docs/decisions/0238`.
    let mut rings = Vec::with_capacity(8);
    head(&mut rings, &nodes[child], direction, profile, style, length);
    round_off(&mut rings, length, body_mm);

    Tube {
        top: from,
        direction,
        rings,
        sides: profile.facets as usize,
        apex: Some(apex_of(&nodes[child], direction, profile, style, length)),
        tail: Some(to + direction * body_mm),
    }
}

/// The tube under the root: a cone from the joint's own width out to the base its height
/// and load ask for, run on down through whatever foot there is so the two overlap.
fn trunk(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Tube {
    let root = tree.root();
    let landing = tree.landing();
    let top = root.position;
    let base_mm = trunk_base_mm(tree, profile, style);

    // Into the foot far enough to overlap it, and no further: a trunk run all the way to
    // the plate shows through the bottom of the pad.
    let into_foot = if landing.on_model {
        0.0
    } else {
        profile.footing_height_mm() * FOOT_BITE
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
    head(&mut rings, root, direction, profile, style, length);
    let end = TrunkEnd {
        length,
        base_mm,
        into_foot,
    };
    let tail_mm = end.close(&mut rings, landing, profile, style);
    Tube {
        top,
        direction,
        rings,
        sides: profile.facets as usize,
        apex: Some(apex_of(root, direction, profile, style, length)),
        tail: tail_mm.map(|along| top + direction * along),
    }
}

/// Where a trunk ends, millimetres along it: its length, its base's radius, and how far
/// it runs on into the foot.
struct TrunkEnd {
    length: Scalar,
    base_mm: Scalar,
    into_foot: Scalar,
}

impl TrunkEnd {
    /// Ends the trunk's rings, and answers how far along its tail pole is, where it has one.
    fn close(
        &self,
        rings: &mut Vec<Ring>,
        landing: Landing,
        profile: &SupportProfile,
        style: Style,
    ) -> Option<Scalar> {
        let on_part = landing.on_model && !landing.on_support;
        if on_part && !style.small {
            let pointed = foot_tip(rings, self.length, self.base_mm, profile);
            let pole_mm = if pointed {
                0.0
            } else {
                profile.contact_radius_mm()
            };
            return Some(self.length + pole_mm);
        }
        if on_part {
            // A half ball of its own width sunk by the lower depth, as its upper end is (ADR 0348).
            let centre_mm = self.length + profile.small_pillar.lower_depth_mm;
            push(rings, centre_mm, self.base_mm);
            round_off(rings, centre_mm, self.base_mm);
            return Some(centre_mm + self.base_mm);
        }
        push(rings, self.length, self.base_mm);
        // A trunk run into another support ends plain, inside the trunk it joins.
        push(rings, self.length + self.into_foot, self.base_mm);
        None
    }
}

/// Radius of `tree`'s trunk at height `z_mm` as it is meshed, millimetres: its joint's
/// width at the root, its base's at the bottom, and straight between.
pub(crate) fn trunk_radius_at_mm(
    tree: &SupportTree,
    profile: &SupportProfile,
    z_mm: Scalar,
) -> Scalar {
    let style = Style::of(tree, profile);
    let (top, bottom) = (tree.root().position.z, trunk_bottom(tree, profile).z);
    let upper = style.body_radius_mm(tree.root().radius_mm, profile);
    let lower = trunk_base_mm(tree, profile, style);
    let along = ((top - z_mm) / (top - bottom).max(RING_EPSILON_MM)).clamp(0.0, 1.0);
    upper + (lower - upper) * along
}

/// Where the axis of `tree`'s trunk is at height `z_mm`: under the root for a trunk that
/// stands straight, and along the lean of one whose foot or root a hand has carried.
pub(crate) fn trunk_axis_at(tree: &SupportTree, profile: &SupportProfile, z_mm: Scalar) -> Vec3 {
    let (top, bottom) = (tree.root().position, trunk_bottom(tree, profile));
    let along = ((top.z - z_mm) / (top.z - bottom.z).max(RING_EPSILON_MM)).clamp(0.0, 1.0);
    top.lerp(bottom, along)
}

#[cfg(test)]
mod tests;
