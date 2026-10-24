use core_geometry::{Mat4, Vec3};
use printer_profiles::SupportProfile;

use crate::Landing;
use crate::clear::{Beam, clear};
use crate::column::{foot_clear, has_head, landing, neck_of, swept_bottom_z};
use crate::group::Profiles;
use crate::mesher::sound;
use crate::pick::Part;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// Whether every body of `tree` (plate coordinates) keeps out of the model, tip apart: swept as
/// beams with the profile's clearance (ADR 0077), then drawn bodies are tested too (ADR 0341).
pub fn fits(tree: &SupportTree, placed: &Placed, profile: &SupportProfile) -> bool {
    let head_mm = profile.top.length_mm + profile.tip.contact_depth_mm;
    let struts = tree.struts().all(|(child, parent)| {
        let nodes = tree.nodes();
        let beam = Beam {
            from: nodes[child].position,
            to: nodes[parent].position,
            from_radius_mm: nodes[child].radius_mm,
            to_radius_mm: nodes[parent].radius_mm,
        }
        .with_clearance(profile.clearance_mm)
        .trimmed(if nodes[child].is_leaf() { head_mm } else { 0.0 });
        clear(placed, &beam)
    });

    let root = tree.root();
    let landing = tree.landing();
    let trunk = Beam {
        from: root.position,
        to: Vec3::new(
            landing.base.x,
            landing.base.y,
            swept_bottom_z(placed, &landing, root.radius_mm, profile),
        ),
        from_radius_mm: root.radius_mm,
        to_radius_mm: root.radius_mm,
    }
    .with_clearance(profile.clearance_mm)
    .trimmed(if root.is_leaf() { head_mm } else { 0.0 });

    struts
        && clear(placed, &trunk)
        && (landing.on_model || foot_clear(placed, landing.pad_centre(), profile))
        && sound(tree, Profiles::single(profile), placed)
}

/// The nearest point of the model to `at`, in plate coordinates, for a tip that must keep touching
/// what it holds; `None` if nowhere near or the surface is painted out (ADR 0093).
pub fn on_model(placed: &Placed, at: Vec3) -> Option<Vec3> {
    let matrix = placed.transform.to_matrix();
    let inverse = placed.transform.inverse_matrix()?;
    let closest = matrix.transform_point3(placed.bvh.closest(inverse.transform_point3(at))?.point);
    (!placed.blocks_at(closest)).then_some(closest)
}

/// `tree` (model space, as the answer) carried by `step`, plate mm, or `None` if the model
/// refuses: tips pull back onto the nearest surface and the whole is swept (ADR 0095).
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
        carry_part(&mut carried, *part, matrix, inverse, step);
    }
    snap_tips(&mut carried, placed, matrix, inverse);
    fits(&carried.moved(matrix), placed, profile).then_some(carried)
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
    let matrix = placed.transform.to_matrix();
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
            carried.stand_on(Landing {
                base: inverse.transform_point3(under.base),
                pad_offset: inverse
                    .transform_vector3(under.pad_offset.extend(0.0))
                    .truncate(),
                pad_cut: under.pad_cut.map(|cut| cut.moved(inverse)),
                ..under
            });
        }
    }
    fits(&carried.moved(matrix), placed, profile).then_some(carried)
}

/// Moves one part of `tree`, which is kept in the model's own space while the step is a
/// step across the plate.
fn carry_part(tree: &mut SupportTree, part: Part, matrix: Mat4, inverse: Mat4, step: Vec3) {
    let carried = |tree: &mut SupportTree, node: usize| {
        let Some(from) = tree.nodes().get(node).map(|node| node.position) else {
            return;
        };
        tree.move_node(
            node,
            inverse.transform_point3(matrix.transform_point3(from) + step),
        );
    };

    match part {
        Part::Node(node) => carried(tree, node),
        Part::Strut(child) => {
            if let Some(parent) = tree.nodes().get(child).and_then(|node| node.parent) {
                carried(tree, parent);
            }
            carried(tree, child);
        }
        Part::Trunk => {
            carried(tree, tree.root_index());
            move_foot(tree, matrix, inverse, step);
        }
        Part::Foot => move_foot(tree, matrix, inverse, step),
    }
}

/// Carries a tree's foot by `step`, keeping it on the plate: a foot in the air holds
/// nothing up.
fn move_foot(tree: &mut SupportTree, matrix: Mat4, inverse: Mat4, step: Vec3) {
    let standing = matrix.transform_point3(tree.landing().base) + step;
    tree.move_landing(inverse.transform_point3(Vec3::new(standing.x, standing.y, 0.0)));
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
