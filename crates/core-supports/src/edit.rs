use core_geometry::{Mat4, Scalar, Vec3};
use printer_profiles::SupportProfile;

use crate::Landing;
use crate::clear::{Beam, clear};
use crate::column::{MAX_LEAN_PER_DROP, foot_clear, has_head, landing, neck_of, swept_bottom_z};
use crate::group::Profiles;
use crate::mesher::sound;
use crate::pick::Part;
use crate::placed::Placed;
use crate::tree::SupportTree;

mod column;
mod joint;
mod riders;
pub use riders::{followed, standing_on};

use column::carry_column;
use joint::{carry_node, settle, shift};

/// Whether every body of `tree` (plate coordinates) keeps out of the model, tip apart: swept as
/// beams with the profile's clearance (ADR 0077), then drawn bodies are tested too (ADR 0337).
pub fn fits(tree: &SupportTree, placed: &Placed, profile: &SupportProfile) -> bool {
    tree.struts()
        .all(|(child, parent)| strut_fits(tree, child, parent, placed, profile))
        && trunk_fits(tree, placed, profile)
        && sound(tree, Profiles::single(profile), placed)
}

/// Whether `after`, carried from `before` (both model space), may stand: it [`fits`], or
/// `before` did not and nothing the carry moved runs into the model or leans too far (ADR 0438).
pub fn allowed(
    before: &SupportTree,
    after: &SupportTree,
    placed: &Placed,
    profile: &SupportProfile,
) -> bool {
    let matrix = placed.transform.to_matrix();
    let (was, now) = (before.moved(matrix), after.moved(matrix));
    if fits(&now, placed, profile) {
        return true;
    }
    if fits(&was, placed, profile) {
        return false;
    }
    let at = |tree: &SupportTree, node: usize| tree.nodes().get(node).map(|node| node.position);
    let moved = |node: usize| at(&was, node) != at(&now, node);
    let struts = now
        .struts()
        .filter(|(child, parent)| moved(*child) || moved(*parent))
        .all(|(child, parent)| strut_fits(&now, child, parent, placed, profile));
    let trunk_moved = moved(now.root_index()) || was.landing() != now.landing();
    struts && (!trunk_moved || trunk_fits(&now, placed, profile))
}

/// Whether the strut from `child` down to `parent` of `tree` (plate) keeps out of the model and,
/// under a head, leans no further than [`MAX_LEAN_PER_DROP`] (ADR 0437).
fn strut_fits(
    tree: &SupportTree,
    child: usize,
    parent: usize,
    placed: &Placed,
    profile: &SupportProfile,
) -> bool {
    let nodes = tree.nodes();
    let (top, bottom) = (nodes[child], nodes[parent]);
    let beam = Beam {
        from: top.position,
        to: bottom.position,
        from_radius_mm: top.radius_mm,
        to_radius_mm: bottom.radius_mm,
    }
    .with_clearance(profile.clearance_mm)
    .trimmed(if top.is_leaf() { head_mm(profile) } else { 0.0 });
    let lean = top.position - bottom.position;
    let upright = top.is_leaf()
        || lean.truncate().length() <= lean.z.abs() * MAX_LEAN_PER_DROP + LEAN_SLACK_MM;
    upright && clear(placed, &beam)
}

/// Whether the trunk of `tree` (plate) keeps out of the model, stands straight over a foot on
/// the plate (ADR 0437), and that foot has room.
fn trunk_fits(tree: &SupportTree, placed: &Placed, profile: &SupportProfile) -> bool {
    let root = tree.root();
    let landing = tree.landing();
    let trunk = Beam {
        from: root.position,
        to: landing
            .base
            .with_z(swept_bottom_z(placed, &landing, root.radius_mm, profile)),
        from_radius_mm: root.radius_mm,
        to_radius_mm: root.radius_mm,
    }
    .with_clearance(profile.clearance_mm)
    .trimmed(if root.is_leaf() {
        head_mm(profile)
    } else {
        0.0
    });
    let on_plate = !landing.on_model && !landing.in_plate;
    let upright = (root.position - landing.base).truncate().length() < LEAN_SLACK_MM;
    (!on_plate || (upright && foot_clear(placed, landing.pad_centre(), profile)))
        && clear(placed, &trunk)
}

/// How much of a tip's strut is its head, which is meant to touch the model, mm.
fn head_mm(profile: &SupportProfile) -> Scalar {
    profile.top.length_mm + profile.tip.contact_depth_mm
}

/// How far past a 45-degree lean a strut may run, mm: float noise from the model's transform.
const LEAN_SLACK_MM: Scalar = 1e-3;

/// The nearest point of the model to `at`, in plate coordinates, for a tip that must keep touching
/// what it holds; `None` if nowhere near or the surface is painted out (ADR 0093).
pub fn on_model(placed: &Placed, at: Vec3) -> Option<Vec3> {
    let matrix = placed.transform.to_matrix();
    let inverse = placed.transform.inverse_matrix()?;
    let closest = matrix.transform_point3(placed.bvh.closest(inverse.transform_point3(at))?.point);
    (!placed.blocks_at(closest)).then_some(closest)
}

/// `tree` (model space, as the answer) carried by `step`, plate mm, tips pulled back onto the
/// nearest surface, or `None` when its column has nothing to stand on; [`allowed`] says whether
/// it may stand there (ADR 0095, 0438).
pub fn carried(
    tree: &SupportTree,
    parts: &[Part],
    step: Vec3,
    placed: &Placed,
    profile: &SupportProfile,
) -> Option<SupportTree> {
    let matrix = placed.transform.to_matrix();
    let inverse = placed.transform.inverse_matrix()?;

    let mut carried = tree.clone();
    for part in parts {
        carry_part(&mut carried, *part, (matrix, inverse), step, profile);
    }
    snap_tips(&mut carried, placed, matrix, inverse);
    if carried.landing() != tree.landing() {
        // A column carried stands on whatever is under it there, the model or the plate.
        let root = matrix.transform_point3(carried.root().position);
        let under = landing(
            placed,
            root,
            carried.root().radius_mm,
            carried.tip_count(),
            profile,
        )?;
        carried.stand_on(into_model(under, inverse));
    }
    Some(carried)
}

/// `landing`, found in plate coordinates, in the model's own space through `inverse`.
fn into_model(landing: Landing, inverse: Mat4) -> Landing {
    Landing {
        base: inverse.transform_point3(landing.base),
        pad_offset: inverse
            .transform_vector3(landing.pad_offset.extend(0.0))
            .truncate(),
        pad_cut: landing.pad_cut.map(|cut| cut.moved(inverse)),
        ..landing
    }
}

/// `tree` (model space, as the answer) with tip `tip` put down at `to` (plate coordinates, nearest
/// surface), or `None` if refused; the neck goes with the tip and the rest follows [`carried`].
pub fn tip_carried(
    tree: &SupportTree,
    tip: usize,
    to: Vec3,
    placed: &Placed,
    profile: &SupportProfile,
) -> Option<SupportTree> {
    let inverse = placed.transform.inverse_matrix()?;
    let node = tree.nodes().get(tip).filter(|node| node.is_leaf())?;
    let contact = on_model(placed, to)?;
    let neck = neck_of(placed, contact, profile)?;

    let mut carried = tree.clone();
    carried.move_node(tip, inverse.transform_point3(contact));
    if let Some(parent) = node.parent {
        // A head with no room under it would run through the model.
        if !has_head(contact, neck) {
            return None;
        }
        carried.move_node(parent, inverse.transform_point3(neck));
        let alone = parent == tree.root_index() && tree.tip_count() == 1;
        if alone && let Some(under) = landing(placed, neck, tree.root().radius_mm, 1, profile) {
            carried.stand_on(into_model(under, inverse));
        }
    }
    allowed(tree, &carried, placed, profile).then_some(carried)
}

/// Moves one part of `tree`, which is kept in the model's own space while the step is a
/// step across the plate. The root, trunk and foot are one column (ADR 0438).
fn carry_part(
    tree: &mut SupportTree,
    part: Part,
    space: (Mat4, Mat4),
    step: Vec3,
    profile: &SupportProfile,
) {
    let root = tree.root_index();
    match part {
        Part::Node(node) if node != root => carry_node(tree, node, space, step),
        Part::Node(_) | Part::Trunk => carry_column(tree, space, step, profile),
        // A foot stays on what it stands on, so only the step across moves it.
        Part::Foot => carry_column(tree, space, step.with_z(0.0), profile),
        Part::Strut(child) => {
            let parent = tree.nodes().get(child).and_then(|node| node.parent);
            match parent {
                Some(parent) if parent == root => {
                    carry_node(tree, child, space, step);
                    carry_column(tree, space, step, profile);
                }
                Some(parent) => {
                    shift(tree, parent, space, step);
                    carry_node(tree, child, space, step);
                    settle(tree, parent, space);
                }
                None => {}
            }
        }
    }
}

/// Pulls every tip back onto the surface it holds up: a support that has stopped touching
/// the model holds nothing, and one carried into it is not printable.
fn snap_tips(tree: &mut SupportTree, placed: &Placed, matrix: Mat4, inverse: Mat4) {
    let tips: Vec<(usize, Vec3)> = tree
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| node.is_leaf())
        .map(|(index, node)| (index, node.position))
        .collect();
    for (index, position) in tips {
        if let Some(on) = on_model(placed, matrix.transform_point3(position)) {
            tree.move_node(index, inverse.transform_point3(on));
        }
    }
}

#[cfg(test)]
mod tests;
