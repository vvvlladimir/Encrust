use crate::placed::Placed;
use core_geometry::{Mesh, Ray, Scalar, Vec2, Vec3, raycast_placed};
use printer_profiles::{ContactShape, SupportProfile};

use crate::Landing;
use crate::base::{braces, skate, skate_half_width_mm, toe_of};
use crate::buttress::buttresses;
use crate::column::{SURFACE_EPSILON_MM, face_normal};
use crate::group::Profiles;
use crate::pad::PAD_SECTORS;
use crate::raft::raft;
use crate::tree::{SupportTree, TreeNode};

mod tube;

use tube::{RING_EPSILON_MM, Ring, Tube, ball, dome, empty_tube, push, round_off, sweep};

/// How many rings a sphere tip is drawn with, as a fraction of the profile's facets.
/// A tip is under a millimetre across, so the silhouette needs far fewer rings than the
/// circumference needs sides.
const SPHERE_RINGS: u32 = 6;

/// How far a tapering foot narrows on the way up to the support it carries: it never
/// closes in past this fraction of its own radius, so the rim stays a rim.
pub const FOOT_TAPER: Scalar = 0.45;

/// How far the pad's wall pulls in on the way down to the plate, as a fraction of its own
/// height. The whole wall is the bevel, so this is the tangent of its lean: 0.6 stands it
/// at 59 degrees off the plate. Every foot has one: a pad that meets the plate on a sharp
/// rim is the one a blade gets under. See `docs/decisions/0044` and `docs/decisions/0130`.
pub const FOOT_BEVEL: Scalar = 0.6;

/// How far a trunk reaches into the foot under it, as a fraction of the foot's height.
/// Far enough that the two solids overlap, not so far that the trunk shows through the
/// bottom of the pad.
const FOOT_BITE: Scalar = 0.5;

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
    /// A support is small when it runs between two parts of the model and carries one
    /// tip. A trunk that several tips hang off carries their weight and keeps its own
    /// width whatever it stands on.
    fn of(tree: &SupportTree, profile: &SupportProfile) -> Self {
        Self {
            small: profile.small_pillar.enabled
                && tree.landing().on_model
                && !tree.landing().on_support
                && tree.tip_count() == 1
                && run_mm(tree) <= profile.small_pillar.max_length_mm,
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
            profile.contact_depth_mm()
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

/// How far a one-tip tree runs from its contact to where it stands, along its struts,
/// millimetres.
fn run_mm(tree: &SupportTree) -> Scalar {
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

/// Welds every support into one mesh, in plate coordinates.
///
/// Each strut is its own closed tube, and a tube reaches past the joint it ends at so
/// that it overlaps the one below. Overlapping closed solids are what the non-zero
/// winding fill already unions; see `docs/decisions/0008` and `docs/decisions/0041`.
///
/// `placed` puts the mesh the braces have to keep clear of in the same space; every other
/// body was already tested where it was built.
///
/// Each tree is built to its own group's profile, each head on it to the group it names
/// (ADR 0325), and each brace to the group it ties (ADR 0252). The raft under them is
/// built to group 0's: it belongs to the plate rather than to any one support. See
/// `docs/decisions/0094`.
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

/// The same supports as [`mesh_trees`], one mesh per group in group order, so that each
/// can be drawn apart. A strut goes with the heaviest head hanging from it, so a tip merged
/// into another group's trunk is drawn in its own colour down to the joint; the trunk and
/// the foot go with the tree's group, a brace with the group it is built to, and the raft
/// with group 0, whose profile it takes.
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

/// The tip alone, from `contact` out to `neck`: what a support that cannot stand is shown
/// as before it is put down.
pub(crate) fn mesh_head(contact: Vec3, neck: Vec3, profile: &SupportProfile) -> Mesh {
    let radius_mm = profile.pillar_radius_mm();
    let nodes = vec![
        TreeNode {
            position: contact,
            radius_mm,
            parent: Some(1),
            point: Some(0),
            head: None,
        },
        TreeNode {
            position: neck,
            radius_mm,
            parent: None,
            point: None,
            head: None,
        },
    ];
    let landing = Landing {
        base: neck,
        on_model: false,
        pad_offset: Vec2::ZERO,
        pad_cut: None,
        on_support: false,
    };
    let tree = SupportTree::new(nodes, 1, landing);
    let mut mesh = Mesh::default();
    sweep(
        &mut mesh,
        &strut(
            &tree,
            0,
            1,
            profile,
            Style {
                small: false,
                bite_limit_mm: Scalar::INFINITY,
                face: None,
                ground: None,
            },
        ),
    );
    mesh
}

fn mesh_tree(slots: &mut Slots, tree: &SupportTree, profiles: Profiles, placed: &Placed) {
    let profile = profiles.of(tree.group());
    let style = Style::of(tree, profile);
    let nodes = tree.nodes();
    let drawn_in = tree.node_groups(profiles);
    for (child, parent) in tree.struts() {
        let (style, own) = if nodes[child].is_leaf() {
            let into = nodes[child].position - nodes[parent].position;
            let style = Style {
                bite_limit_mm: wall_mm(placed, nodes[child].position, into) * WALL_BITTEN,
                face: face_normal(placed, nodes[child].position),
                ..style
            };
            (style, profiles.of(tree.head_group(child)))
        } else {
            (style, profile)
        };
        sweep(
            slots.of(drawn_in[child]),
            &strut(tree, child, parent, own, style),
        );
    }
    let mesh = slots.of(tree.group());
    let style = Style {
        ground: ground_normal(placed, tree.landing()),
        ..style
    };
    // A tree of one node has its tip on the trunk itself, held to the same half wall.
    let root = tree.root();
    let style = if root.is_leaf() {
        let into = root.position - trunk_bottom(tree, profile);
        Style {
            bite_limit_mm: wall_mm(placed, root.position, into) * WALL_BITTEN,
            face: face_normal(placed, root.position),
            ..style
        }
    } else {
        style
    };
    sweep(mesh, &trunk(tree, profile, style));
    stand_on(mesh, tree, profile);
}

/// How much of the wall behind a contact a tip may bite into: half, so that what is left
/// over it still holds and nothing of the tip shows on the far side.
const WALL_BITTEN: Scalar = 0.5;

/// How thick the model is behind `contact`, going `into` it, millimetres: the distance to
/// where a ray leaves it again, or unbounded where it never does.
///
/// A contact a hair off the face — a sampled one, or one carried — meets that face on
/// the way in first, and the wall is measured from there.
fn wall_mm(placed: &Placed, contact: Vec3, into: Vec3) -> Scalar {
    let Some(direction) = into.try_normalize() else {
        return Scalar::INFINITY;
    };
    let mut from = contact + direction * SURFACE_EPSILON_MM;
    let mut entered_mm = SURFACE_EPSILON_MM;
    for _ in 0..2 {
        let ray = Ray::new(from, direction);
        let Some(hit) = raycast_placed(placed.model, placed.bvh, placed.transform, &ray) else {
            return Scalar::INFINITY;
        };
        if hit.normal.dot(direction) > 0.0 {
            return entered_mm + hit.distance;
        }
        from = hit.point + direction * SURFACE_EPSILON_MM;
        entered_mm = SURFACE_EPSILON_MM;
    }
    Scalar::INFINITY
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

/// Whatever the tree stands on: the flare out of the pillar, and then a round foot swept
/// as rings or a skate. Nothing at all when it lands on the model, and no foot on a raft,
/// which the flare stands on instead; see `docs/decisions/0266`.
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
        seat: None,
        sole: None,
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
    head(&mut rings, &nodes[child], profile, style, length);
    round_off(&mut rings, length, body_mm);

    Tube {
        top: from,
        direction,
        rings,
        sides: profile.facets as usize,
        apex: apex_of(&nodes[child], direction, profile, style),
        tail: Some(to + direction * body_mm),
        seat: seat_of(&nodes[child], profile, style),
        sole: None,
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
    head(&mut rings, root, profile, style, length);
    // A trunk run into another support ends plain, inside the trunk it joins.
    let mut sole = None;
    let tail = if landing.on_model && !landing.on_support && !style.small {
        let pointed = foot_tip(&mut rings, length, base_mm, profile);
        sole = style.ground.filter(|_| !pointed);
        pointed.then(|| top + direction * length)
    } else if style.small && landing.on_model && !landing.on_support {
        // One ring, so the whole of it can be laid onto the face it stands on.
        push(&mut rings, length, base_mm);
        sole = style.ground;
        None
    } else {
        push(&mut rings, length, base_mm);
        push(&mut rings, length + into_foot, base_mm);
        None
    };

    Tube {
        top,
        direction,
        rings,
        sides: profile.facets as usize,
        apex: apex_of(root, direction, profile, style),
        tail,
        seat: seat_of(root, profile, style),
        sole,
    }
}

/// Closes a trunk standing on the model with the tip it would have carried, upside down:
/// what touches the part is a contact rather than the whole trunk.
///
/// The cone leaves the trunk at whatever width the trunk is, so there is no rim hanging
/// off its top. `length` runs from the root to the sunk bottom. Like the tip at the top, a
/// plane tip is pushed its bite into the surface and ends flat there; a sphere meets the
/// surface on its contact and bites in to a point, which is when it answers true. See
/// `docs/decisions/0124` and `docs/decisions/0243`.
fn foot_tip(
    rings: &mut Vec<Ring>,
    length: Scalar,
    body_mm: Scalar,
    profile: &SupportProfile,
) -> bool {
    let pointed = profile.tip.shape == ContactShape::Sphere;
    let end_mm = if pointed {
        (length - profile.landing_depth_mm()).max(0.0)
    } else {
        length
    };
    let taper_mm = profile.top_length_mm().min(end_mm);
    let contact_mm = profile.contact_radius_mm().min(body_mm);

    push(rings, end_mm - taper_mm, body_mm);
    push(rings, end_mm, contact_mm);
    pointed
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

/// How wide the trunk is where it meets what it stands on, millimetres: its own width on
/// the model, where it ends in a tip like any branch; see `docs/decisions/0242`.
fn trunk_base_mm(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Scalar {
    let root = tree.root();
    if tree.landing().on_model {
        return style.body_radius_mm(root.radius_mm, profile);
    }
    let height = (root.position - trunk_bottom(tree, profile)).length();
    let base = profile.trunk_base_radius_mm(height, tree.tip_count());
    style.body_radius_mm(base.max(root.radius_mm), profile)
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

/// The foot the trunk stands on, as a solid of its own so that it can have a different
/// number of sides from the support above it. `None` when the support lands on the model.
fn foot(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Option<Tube> {
    let landing = tree.landing();
    if landing.on_model {
        return None;
    }

    let shape = profile.bottom.shape;
    let rim_mm = profile.base_radius_mm();
    let trunk_mm = trunk_base_mm(tree, profile, style);
    let top_mm = foot_top_radius_mm(profile, trunk_mm);

    let height = profile.base_height_mm();
    let bevel = height * FOOT_BEVEL;
    // The pad stands where the landing found room for it, which is under the trunk unless
    // the model was in the way; see `docs/decisions/0239`.
    let centre = landing.pad_centre();
    let top = Vec3::new(centre.x, centre.y, landing.pillar_bottom_z(profile));
    // A cut pad is drawn with a side to every half sector, so the cut keeps its shape.
    let facets = if landing.pad_cut.is_some() {
        profile.facets.max(2 * PAD_SECTORS as u32)
    } else {
        profile.facets
    };

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
        sides: shape.sides(facets) as usize,
        apex: None,
        tail: None,
        seat: None,
        sole: None,
    })
}

/// Radius of the ring the pad meets the plate on, millimetres: its rim pulled in by the
/// bevel, and never inside the trunk standing in it.
fn plate_radius_mm(profile: &SupportProfile, trunk_mm: Scalar) -> Scalar {
    let bevel = profile.base_height_mm() * FOOT_BEVEL;
    (profile.base_radius_mm() - bevel).max(trunk_mm)
}

/// The cone that widens the pillar out into the pad or raft it stands on, or `None` when the
/// support lands on the model or has no room for one.
///
/// It rises as far as it widens, which is the forty-five degrees every MSLA slicer draws
/// this joint at, and it is drawn with the pillar's own facets rather than the pad's; see
/// `docs/decisions/0130`.
fn flare(tree: &SupportTree, profile: &SupportProfile, style: Style) -> Option<Tube> {
    let landing = tree.landing();
    let trunk_mm = trunk_base_mm(tree, profile, style);
    let upper_mm = trunk_mm;
    // Inside the pad at every depth it reaches, so it never shows through its side.
    let widest_mm = if profile.raft.enabled {
        Scalar::INFINITY
    } else if profile.bottom.shape.is_round() {
        plate_radius_mm(profile, trunk_mm).min(foot_top_radius_mm(profile, trunk_mm))
    } else {
        skate_half_width_mm(profile)
    };
    let lower_mm = profile.flare_lower_radius_mm().min(widest_mm).max(upper_mm);

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
        rise_mm + profile.footing_height_mm() * FOOT_BITE,
        upper_mm + rise_mm,
    );

    Some(Tube {
        top: Vec3::new(landing.base.x, landing.base.y, bottom_z + rise_mm),
        direction: -Vec3::Z,
        rings,
        sides: profile.facets as usize,
        apex: None,
        tail: None,
        seat: None,
        sole: None,
    })
}

/// Radius of what a support standing on the plate meets the ground with, millimetres: its
/// pad's rim, or the bottom of its flare where a raft takes the pad's place.
pub(crate) fn footing_radius_mm(tree: &SupportTree, profile: &SupportProfile) -> Scalar {
    if !profile.raft.enabled {
        return profile.base_radius_mm();
    }
    let trunk_mm = trunk_base_mm(tree, profile, Style::of(tree, profile));
    profile.flare_lower_radius_mm().max(trunk_mm)
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
///
/// `length` is the tube's own, and the top segment is no longer: a neck shortened to
/// leave the body room over the plate would otherwise run on through the pad.
fn head(
    rings: &mut Vec<Ring>,
    node: &TreeNode,
    profile: &SupportProfile,
    style: Style,
    length: Scalar,
) {
    if !node.is_leaf() {
        dome(rings, style.body_radius_mm(node.radius_mm, profile));
        return;
    }

    let contact_mm = profile.contact_radius_mm();
    let top_mm = profile.top_length_mm().min(length);
    let body_mm = style.body_radius_mm(node.radius_mm, profile);
    // A small pillar has no tip: it is one width from the face it holds to the one it
    // stands on, sunk into each.
    if style.small {
        push(rings, -style.bite_mm(profile), body_mm);
        return;
    }
    if profile.tip.shape == ContactShape::Sphere {
        ball(rings, style.bite_mm(profile), contact_mm, SPHERE_RINGS);
        // One cone from the contact out to the body's own width; see `docs/decisions/0238`.
        push(rings, top_mm, body_mm);
        return;
    }

    // The whole tip is pushed its bite into the face, the cone with it; the disc it ends
    // in is what grips. See `docs/decisions/0243`.
    let bite_mm = style.bite_mm(profile).min(top_mm);
    push(rings, -bite_mm, contact_mm);
    push(rings, top_mm - bite_mm, body_mm);
}

/// The outward normal of the face of the model `landing` stands on, if it stands on the
/// model itself.
fn ground_normal(placed: &Placed, landing: Landing) -> Option<Vec3> {
    if !landing.on_model || landing.on_support {
        return None;
    }
    let ray = Ray::new(landing.base + Vec3::Z * SURFACE_EPSILON_MM, -Vec3::Z);
    raycast_placed(placed.model, placed.bvh, placed.transform, &ray)
        .filter(|hit| hit.distance <= 2.0 * SURFACE_EPSILON_MM)
        .map(|hit| hit.normal)
}

/// The face a plane tip or a small pillar is laid onto, for a tube that starts at one; a
/// sphere and a joint have no flat end to lay.
fn seat_of(node: &TreeNode, profile: &SupportProfile, style: Style) -> Option<Vec3> {
    let plane = profile.tip.shape == ContactShape::Plane;
    (node.is_leaf() && (plane || style.small))
        .then_some(style.face)
        .flatten()
}

/// Where the tip pokes into the model, for a tube that starts at a contact, and the pole
/// of the dome for one that starts at a joint.
///
/// It goes back up the tube's own axis, which at a tip is the normal of the face it
/// holds, so the bite drives into the surface square on (ADR 0125). A sphere's upper pole
/// is `bite_mm` in. A plane tip has no apex: the sweep closes it with the flat disc its
/// first ring makes, already pushed in (ADR 0243).
fn apex_of(
    node: &TreeNode,
    direction: Vec3,
    profile: &SupportProfile,
    style: Style,
) -> Option<Vec3> {
    if !node.is_leaf() {
        return Some(node.position - direction * style.body_radius_mm(node.radius_mm, profile));
    }
    if style.small || profile.tip.shape == ContactShape::Plane {
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
    fn a_pad_cut_back_by_a_kerb_is_closed_and_stops_short_of_it() {
        // A kerb 2 mm tall with its face at x = 1, and a trunk 1.9 mm off it: too near to
        // move the pad aside, so it is cut back to keep its 0.5 mm of air.
        let kerb = box_mesh(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 2.0));
        let profile = profile();
        let trees = trees_on(
            &[SupportPoint::new(Vec3::new(2.9, 0.0, 20.0))],
            &kerb,
            Transform::default(),
            Profiles::single(&profile),
        );
        assert!(trees[0].landing().pad_cut.is_some(), "the pad is cut");
        let mesh = meshed(&trees, Profiles::single(&profile));
        let diagnostics = diagnose(&mesh);
        assert_eq!(diagnostics.boundary_edges, 0, "a cut pad has no open edges");
        assert_eq!(diagnostics.non_manifold_edges, 0);
        let nearest = mesh
            .vertices
            .iter()
            .map(|at| at.x)
            .fold(Scalar::INFINITY, Scalar::min);
        assert!(
            nearest >= 1.5 - 1e-3,
            "nothing of it comes within the clearance of the kerb, got x = {nearest}"
        );
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
        let profiles = [
            profile(),
            SupportProfile::shipped("heavy").expect("the heavy preset ships"),
        ];
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
    fn a_brace_is_meshed_with_the_group_it_ties() {
        // Two tall trunks of the second group, close enough to tie, and nothing at all
        // in the first.
        let mut braced = profile();
        braced.branching.enabled = false;
        braced.bracing = printer_profiles::Bracing {
            enabled: true,
            start_height_mm: 5.0,
            rise_mm: 5.0,
            max_spacing_mm: 12.0,
            ..printer_profiles::Bracing::default()
        };
        let table = [profile(), braced];
        let profiles = Profiles::new(&table).expect("two groups");
        let points = [
            SupportPoint::new(Vec3::new(10.0, 10.0, 30.0)).in_group(1),
            SupportPoint::new(Vec3::new(18.0, 10.0, 30.0)).in_group(1),
        ];
        let model = Mesh::default();
        let bvh = core_geometry::Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let trees = trees_on(&points, &model, Transform::default(), profiles);

        let groups = mesh_groups(&trees, &placed, profiles);
        assert!(
            groups[0].is_empty(),
            "the first group holds nothing, so nothing is drawn in its colour"
        );
        let between = groups[1]
            .vertices
            .iter()
            .filter(|vertex| vertex.x > 12.0 && vertex.x < 16.0)
            .count();
        assert!(
            between > 0,
            "the braces between the two trunks are the second group's"
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

        // The tip's bite, neck frustum, pillar cylinder, the flare into the pad and then the
        // pad itself, one frustum in to the rim it meets the plate on. The pillar reaches
        // into the foot far enough to overlap it, and so does the flare, because each is
        // a solid of its own; see `docs/decisions/0041`. That much of them is counted
        // twice, and the closed form says so too.
        // A plane tip is pushed its bite in, which lengthens the pillar by as much.
        let tip = PI * pillar_r * pillar_r * profile.contact_depth_mm();
        let neck = PI * profile.top_length_mm() / 3.0
            * (contact_r * contact_r + contact_r * pillar_r + pillar_r * pillar_r);
        // The neck ends in a dome inside the pillar and the pillar starts under one inside
        // the neck, each drawn as a cone on its pole and two frustums, and both counted
        // on top of the tube they sit in.
        let pillar = PI * pillar_r * pillar_r * (height - profile.top_length_mm() - trunk_bottom);
        let ring = |step: f32| (PI / 2.0 * step / 3.0).sin() * pillar_r;
        let drop = |step: f32| (1.0 - (PI / 2.0 * step / 3.0).cos()) * pillar_r;
        let dome: f32 = (0..3u8)
            .map(f32::from)
            .map(|step| {
                let (upper, lower) = (ring(step), ring(step + 1.0));
                PI * (drop(step + 1.0) - drop(step)) / 3.0
                    * (upper * upper + upper * lower + lower * lower)
            })
            .sum();
        let seam = 2.0 * dome;
        let pad = PI * foot_h / 3.0 * (base_r * base_r + base_r * rim_r + rim_r * rim_r);
        // The flare rises as far as it widens, and sinks into the pad as far as the
        // pillar does.
        let flare_top_r = pillar_r;
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
            "expected {expected} mm3 from frustums, cylinders and a disc, got {volume}"
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
    fn a_neck_shortened_over_the_plate_stops_where_the_body_begins() {
        // A 2 mm top segment under a contact 1.5 mm up: the neck is cut to the room there
        // is, and what is drawn of it must be cut with it.
        let mut profile = profile();
        profile.branching.enabled = false;
        let points = [SupportPoint::new(Vec3::new(0.0, 0.0, 1.5))];
        let mesh = meshed(
            &trees_on(
                &points,
                &Mesh::default(),
                Transform::default(),
                Profiles::single(&profile),
            ),
            Profiles::single(&profile),
        );

        assert!(
            !mesh.is_empty(),
            "a contact 1.5 mm up has room for a support"
        );
        let lowest = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.z)
            .fold(Scalar::INFINITY, Scalar::min);
        assert!(
            lowest >= -1e-4,
            "a support ends on the plate, and this one reaches {lowest} mm"
        );
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

        // A shallow ramp turned over, so the face that looked up is an overhang within 45
        // degrees of level, which a tip still leaves square on.
        let model = ramp(10.0, 4.0);
        let transform = Transform {
            rotation: Quat::from_rotation_x(PI),
            translation: Vec3::new(0.0, 0.0, 30.0),
            ..Transform::default()
        };
        let profile = profile();
        let contact = transform
            .to_matrix()
            .transform_point3(Vec3::new(5.0, 0.0, 2.0));
        let trees = trees_on(
            &[SupportPoint::new(Vec3::new(5.0, 0.0, 2.0))],
            &model,
            transform,
            Profiles::single(&profile),
        );

        let normal = Vec3::new(-4.0, 0.0, -10.0).normalize();
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
        let depth = profile.contact_depth_mm();
        assert!(
            ((apex - contact).dot(-normal) - depth).abs() < 1e-3,
            "the tip goes its bite into the face, got {}",
            (apex - contact).dot(-normal)
        );
        let centre = contact - normal * depth;
        let disc = mesh
            .vertices
            .iter()
            .filter(|vertex| ((**vertex - contact).dot(-normal) - depth).abs() < 1e-3);
        for vertex in disc {
            assert!(
                (*vertex - centre).length() <= profile.contact_radius_mm() + 1e-3,
                "the disc it ends in lies square across the normal, got {vertex}"
            );
        }
    }

    #[test]
    fn nothing_of_a_plain_column_is_wider_than_its_pillar() {
        let profile = profile();
        let trees = column_on_plate(20.0, &profile);
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
    fn a_strut_ends_rounded_inside_its_trunk_and_nothing_swells_round_the_joint() {
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
        let root = tree.root_index();
        let joint = tree.root().position;
        let trunk_mm = tree.root().radius_mm;
        let style = Style::of(tree, &profile);

        for (child, parent) in tree.struts().filter(|(_, parent)| *parent == root) {
            let tube = strut(tree, child, parent, &profile, style);
            let reaches = (joint - tree.nodes()[child].position).length();
            let own_mm = tree.nodes()[child].radius_mm;
            let widest = tube
                .rings
                .iter()
                .map(|ring| ring.radius_mm)
                .fold(0.0, Scalar::max);
            assert!(
                (widest - own_mm).abs() < 1e-5,
                "a strut keeps its own width of {own_mm} mm and does not widen into the \
                 trunk, got {widest}"
            );

            let mut swept = Mesh::default();
            sweep(&mut swept, &tube);
            let past: Vec<Scalar> = swept
                .vertices
                .iter()
                .filter(|vertex| (**vertex - tube.top).dot(tube.direction) > reaches + 1e-4)
                .map(|vertex| (*vertex - joint).length())
                .collect();
            assert!(!past.is_empty(), "the strut is rounded off past the joint");
            assert!(
                past.iter().all(|far| *far <= trunk_mm + 1e-4),
                "what is past the joint is inside the trunk's own dome of {trunk_mm} mm: \
                 {past:?}"
            );
        }

        let mesh = meshed(&trees, Profiles::single(&profile));
        let widest = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.z < joint.z && vertex.z > joint.z - 2.0 * trunk_mm)
            .map(|vertex| Vec2::new(vertex.x - joint.x, vertex.y - joint.y).length())
            .fold(0.0, Scalar::max);
        assert!(
            widest <= trunk_mm + 1e-4,
            "under the joint there is the trunk and nothing swelling round it: {widest} mm \
             against a trunk of {trunk_mm}"
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
        for shape in [ContactShape::Sphere, ContactShape::Plane] {
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
    fn a_plane_tip_is_pushed_its_bite_into_the_model_and_ends_flat() {
        let flat = with_tip(ContactShape::Plane);
        let mesh = one_column(&flat);
        let sunk = 20.0 + flat.contact_depth_mm();
        assert!(
            (ceiling(&mesh) - sunk).abs() < 1e-4,
            "the tip goes its bite into the face at z = 20, got {}",
            ceiling(&mesh)
        );
        let axis = Vec3::new(10.0, 10.0, 0.0);
        assert!(
            (widest_above(&mesh, axis, sunk - 2e-3) - flat.contact_radius_mm()).abs() < 1e-3,
            "it ends on a disc as wide as its contact, not on a point"
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
    fn a_sphere_tip_is_wide_inside_the_model_and_bites_its_depth() {
        let ball = with_tip(ContactShape::Sphere);
        let axis = Vec3::new(10.0, 10.0, 0.0);

        assert!(
            (ceiling(&one_column(&ball)) - (20.0 + ball.contact_depth_mm())).abs() < 1e-4,
            "the top of the ball is its bite into the model"
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

    /// The highest point of the support held under a 40 mm slab from `bottom_mm` up, whose
    /// contact is in the middle of its underside.
    fn tip_top_under_slab(bottom_mm: Scalar, thickness_mm: Scalar) -> Scalar {
        let slab = crate::tests::box_mesh(
            Vec3::new(-20.0, -20.0, bottom_mm),
            Vec3::new(20.0, 20.0, bottom_mm + thickness_mm),
        );
        let mut fixture = profile();
        fixture.branching.enabled = false;
        let points = [crate::SupportPoint::new(Vec3::new(0.0, 0.0, bottom_mm))];
        let trees = crate::tests::trees_on(
            &points,
            &slab,
            Transform::default(),
            Profiles::single(&fixture),
        );
        let bvh = core_geometry::Bvh::build(&slab);
        let mesh = mesh_trees(
            &trees,
            &Placed::new(&slab, &bvh, Transform::default()),
            Profiles::single(&fixture),
        );
        mesh.vertices
            .iter()
            .map(|vertex| vertex.z)
            .fold(Scalar::NEG_INFINITY, Scalar::max)
    }

    #[test]
    fn a_tip_under_a_thin_wall_bites_no_further_than_half_of_it() {
        let top = tip_top_under_slab(10.0, 0.15);
        assert!(
            top < 10.15,
            "a 0.2 mm bite into a 0.15 mm wall would come out of its top, got {top}"
        );
        assert!(
            (top - 10.075).abs() < 0.01,
            "half the wall is bitten, got {top}"
        );
    }

    #[test]
    fn a_strut_straight_from_its_contact_bites_no_further_than_half_the_wall() {
        // A 0.15 mm ledge a millimetre over a block: one strut from the contact itself.
        let mut model = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 9.0));
        let ledge = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 10.15));
        let offset = model.vertices.len() as u32;
        model.vertices.extend_from_slice(&ledge.vertices);
        model
            .faces
            .extend(ledge.faces.iter().map(|face| face.map(|at| at + offset)));
        let fixture = profile();
        let points = [SupportPoint::new(Vec3::new(5.0, 5.0, 10.0))];
        let trees = trees_on(
            &points,
            &model,
            Transform::default(),
            Profiles::single(&fixture),
        );
        assert!(trees[0].root().is_leaf(), "one node, the contact");

        let bvh = core_geometry::Bvh::build(&model);
        let mesh = mesh_trees(
            &trees,
            &Placed::new(&model, &bvh, Transform::default()),
            Profiles::single(&fixture),
        );
        let top = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.z)
            .fold(Scalar::NEG_INFINITY, Scalar::max);
        assert!(
            (top - 10.075).abs() < 0.01,
            "half the 0.15 mm wall is bitten, got {top}"
        );
    }

    #[test]
    fn a_tip_under_a_thick_wall_bites_its_whole_depth() {
        let top = tip_top_under_slab(10.0, 5.0);
        assert!(
            (top - 10.2).abs() < 0.01,
            "the fixture's 0.2 mm bite, got {top}"
        );
    }

    #[test]
    fn a_support_between_two_surfaces_longer_than_the_small_pillar_keeps_its_width() {
        let mut short = profile();
        short.small_pillar.max_length_mm = 0.1;
        let mut regular = profile();
        regular.small_pillar.enabled = false;

        let limited = signed_volume(&meshed(&model_to_model(&short), Profiles::single(&short)));
        let thick = signed_volume(&meshed(
            &model_to_model(&regular),
            Profiles::single(&regular),
        ));
        assert!(
            (limited - thick).abs() < 1e-3,
            "a run past 0.1 mm is built as a regular support: {limited} mm3 against {thick} mm3"
        );
    }

    #[test]
    fn a_small_pillar_meets_sloping_faces_with_the_whole_of_each_end() {
        // Underside z = 12 - x / 2 over top z = 11 - x / 2: faces at 26.6 degrees a
        // millimetre apart, too near for a head, so the strut runs straight down.
        let mut model = Mesh::default();
        let ramp = ramp(10.0, 5.0);
        for place in [
            (|v: Vec3| Vec3::new(v.x, v.y, 12.0 - v.z)) as fn(Vec3) -> Vec3,
            |v: Vec3| Vec3::new(10.0 - v.x, v.y, v.z + 6.0),
        ] {
            let offset = model.vertices.len() as u32;
            model
                .vertices
                .extend(ramp.vertices.iter().map(|v| place(*v)));
            // Both turn the ramp inside out, so its faces are wound back.
            model.faces.extend(
                ramp.faces
                    .iter()
                    .map(|face| [face[0] + offset, face[2] + offset, face[1] + offset]),
            );
        }
        let fixture = profile();
        let points = [SupportPoint::new(Vec3::new(5.0, 0.0, 9.5))];
        let trees = trees_on(
            &points,
            &model,
            Transform::default(),
            Profiles::single(&fixture),
        );
        assert!(
            trees[0].root().is_leaf(),
            "one strut, from the contact itself"
        );
        assert!(Style::of(&trees[0], &fixture).small, "it runs face to face");

        let bvh = core_geometry::Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let mesh = mesh_trees(&trees, &placed, Profiles::single(&fixture));
        // Each end is laid on its face and then sunk along the axis, so every point of
        // it stands the same height off that face.
        for (name, upper) in [("top", true), ("bottom", false)] {
            let heights: Vec<Scalar> = mesh
                .vertices
                .iter()
                .filter(|vertex| (vertex.z > 9.0) == upper)
                .map(|v| {
                    if upper {
                        12.0 - v.x / 2.0 - v.z
                    } else {
                        v.z - 11.0 + v.x / 2.0
                    }
                })
                .collect();
            let low = heights.iter().copied().fold(Scalar::INFINITY, Scalar::min);
            let high = heights
                .iter()
                .copied()
                .fold(Scalar::NEG_INFINITY, Scalar::max);
            assert!(
                !heights.is_empty() && high - low < 1e-3,
                "the {name} end lies along its face, got {low}..{high} off it"
            );
        }
        assert_eq!(diagnose(&mesh).boundary_edges, 0, "it is closed");
    }

    #[test]
    fn a_small_pillar_has_no_tip_and_is_one_width_from_end_to_end() {
        for shape in [ContactShape::Plane, ContactShape::Sphere] {
            let mut profile = profile();
            profile.tip.shape = shape;
            let trees = model_to_model(&profile);
            let mesh = meshed(&trees, Profiles::single(&profile));
            let radius_mm = profile.small_pillar.radius_mm();

            // Within a millimetre of either face: the joint under the neck is a dome
            // inside the body, and is not what touches the model.
            let ends = mesh
                .vertices
                .iter()
                .filter(|vertex| vertex.z > 19.0 || vertex.z < 11.0);
            for vertex in ends {
                let off_axis = Vec2::new(vertex.x - 5.0, vertex.y - 5.0).length();
                // The middle of each flat end sits on the axis; everything else is wall.
                assert!(
                    off_axis < 1e-3 || (off_axis - radius_mm).abs() < 1e-3,
                    "{shape:?}: a vertex {off_axis} mm off the axis of a pillar \
                     {radius_mm} mm in radius is a tip or a taper"
                );
            }
            assert_eq!(diagnose(&mesh).boundary_edges, 0, "{shape:?}: it is closed");
        }
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
    fn a_support_standing_on_the_model_is_pushed_its_bite_into_it() {
        let mut profile = profile();
        profile.small_pillar.enabled = false;
        let trees = model_to_model(&profile);
        assert!(trees[0].landing().on_model, "it stands on the shelf");

        let mesh = meshed(&trees, Profiles::single(&profile));
        let lowest = mesh
            .vertices
            .iter()
            .copied()
            .min_by(|a, b| a.z.total_cmp(&b.z))
            .expect("a support has vertices");
        let sunk = 10.0 - profile.landing_depth_mm();
        assert!(
            (lowest.z - sunk).abs() < 1e-3,
            "the bite goes the tip's own depth into the 10 mm shelf, got {}",
            lowest.z
        );
        let widest = widest_below(&mesh, Vec3::new(5.0, 5.0, 0.0), sunk);
        assert!(
            (widest - profile.contact_radius_mm()).abs() < 1e-3,
            "it ends flat on its contact's disc, as the plane tip at the top does, got {widest}"
        );
        assert_eq!(diagnose(&mesh).boundary_edges, 0);
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
        // Further over the shelf than a small pillar reaches, which would stand each alone.
        let shelf = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let points = [
            SupportPoint::new(Vec3::new(4.0, 5.0, 25.0)),
            SupportPoint::new(Vec3::new(6.0, 5.0, 25.0)),
            SupportPoint::new(Vec3::new(5.0, 6.5, 25.0)),
        ];
        trees_on(
            &points,
            &shelf,
            Transform::default(),
            Profiles::single(profile),
        )
    }

    #[test]
    fn a_trunk_on_the_model_keeps_its_width_down_to_its_tip() {
        let profile = profile();
        let trees = merged_on_the_model(&profile);
        let [tree] = &trees[..] else {
            panic!("three tips this close merge into one trunk");
        };
        assert!(tree.landing().on_model, "it stands on the shelf");
        let trunk_mm = trunk_base_mm(tree, &profile, Style::of(tree, &profile));
        assert!(
            (trunk_mm - profile.pillar_radius_mm()).abs() < 1e-6,
            "a trunk on the model keeps the branches' width down to its tip, however many \
             it carries, got {trunk_mm}"
        );

        let mesh = meshed(&trees, Profiles::single(&profile));
        let axis = tree.landing().base;
        let widths: Vec<Scalar> = mesh
            .vertices
            .iter()
            .filter(|vertex| {
                let shoulder = 10.0 - profile.landing_depth_mm() + profile.top_length_mm();
                (vertex.z - shoulder).abs() < 1e-3
            })
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
    fn a_trunk_widens_from_its_joint_down_to_its_base() {
        let profile = profile();
        let trees = trees_on(
            &[
                SupportPoint::new(Vec3::new(10.0, 10.0, 40.0)),
                SupportPoint::new(Vec3::new(14.0, 10.0, 40.0)),
            ],
            &Mesh::default(),
            Transform::default(),
            Profiles::single(&profile),
        );
        let [tree] = &trees[..] else {
            panic!("two tips this close merge into one trunk");
        };
        let tube = trunk(tree, &profile, Style::of(tree, &profile));
        let pillar = profile.pillar_radius_mm();
        let last = tube.rings[tube.rings.len() - 1].radius_mm;
        let height = (tree.root().position - trunk_bottom(tree, &profile)).length();
        let base = profile.trunk_base_radius_mm(height, 2);
        assert!(
            tube.rings
                .iter()
                .any(|ring| (ring.radius_mm - pillar).abs() < 1e-5),
            "the trunk leaves its joint at the branches' own width of {pillar} mm"
        );
        assert!(
            (last - base).abs() < 1e-5 && last > pillar,
            "it widens to the {base} mm its height and two tips ask for, got {last}"
        );
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

    #[test]
    fn the_cone_under_a_tip_runs_from_the_contact_out_to_the_pillar() {
        let profile = profile();
        let mesh = one_column(&profile);
        let axis = Vec3::new(10.0, 10.0, 0.0);
        let width_at = |z: Scalar| {
            mesh.vertices
                .iter()
                .filter(|vertex| (vertex.z - z).abs() < 1e-3)
                .map(|vertex| Vec2::new(vertex.x - axis.x, vertex.y - axis.y).length())
                .fold(0.0, Scalar::max)
        };

        let sunk = 20.0 + profile.contact_depth_mm();
        assert!(
            (width_at(sunk) - profile.contact_radius_mm()).abs() < 1e-3,
            "the cone starts its bite inside the model as wide as the contact, got {}",
            width_at(sunk)
        );
        let shoulder = sunk - profile.top_length_mm();
        assert!(
            (width_at(shoulder) - profile.pillar_radius_mm()).abs() < 1e-3,
            "and meets the pillar at the pillar's own width, got {}",
            width_at(shoulder)
        );
    }

    #[test]
    fn a_support_on_a_raft_has_no_pad_and_flares_onto_the_raft() {
        let mut profile = profile();
        profile.raft.enabled = true;
        let trees = column_on_plate(20.0, &profile);
        let model = Mesh::default();
        let bvh = core_geometry::Bvh::build(&model);
        let mesh = mesh_alone(
            &trees[0],
            Profiles::single(&profile),
            &Placed::new(&model, &bvh, Transform::default()),
        );
        let thickness = profile.raft.thickness_mm;
        let flare_mm = profile.flare_lower_radius_mm();
        assert!(
            flare_mm < profile.base_radius_mm(),
            "the fixture's flare is narrower than its pad, or this proves nothing"
        );

        let axis = Vec2::new(10.0, 10.0);
        for vertex in &mesh.vertices {
            assert!(
                vertex.z >= thickness * (1.0 - FOOT_BITE) - 1e-4,
                "nothing reaches under the raft's top by more than the bite, got {vertex}"
            );
            let reach = (vertex.truncate() - axis).length();
            assert!(
                reach <= flare_mm + 1e-3,
                "nothing is wider than the flare, got {reach} mm out at {vertex}"
            );
        }
        let style = Style::of(&trees[0], &profile);
        let tube = flare(&trees[0], &profile, style).expect("a column on a raft flares");
        let rises = tube.rings[1].along_mm - tube.rings[0].along_mm;
        assert!(
            (tube.rings[1].radius_mm - flare_mm).abs() < 1e-5,
            "the flare widens to its own lower diameter, not to what a pad would allow"
        );
        assert!(
            (tube.top.z - (thickness + rises)).abs() < 1e-5,
            "the flare stands on the top of the raft"
        );
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
