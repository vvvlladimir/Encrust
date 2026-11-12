use core_geometry::{Mat4, Scalar, Vec3};
use printer_profiles::SupportProfile;

use crate::Landing;
use crate::clear::{Beam, clear};
use crate::column::{MAX_LEAN_PER_DROP, foot_clear, has_head, head_of, swept_bottom_z};
use crate::group::Profiles;
use crate::lost::Unstood;
use crate::mesher::sound;
use crate::pick::Part;
use crate::placed::Placed;
use crate::tree::SupportTree;

mod column;
mod footing;
mod give_way;
mod held;
mod joint;
mod pillar;
mod refused;
mod riders;
pub use give_way::eased;
pub use held::{Carried, carried_held, with_riders};
pub use refused::Refused;
pub(crate) use riders::{carrying, unseated};
pub use riders::{followed, standing_on};

use column::carry_column;
use footing::{footing_near, footing_under};
use joint::{carry_node, settle, shift};
use pillar::{is_pillar, regrown};

/// Whether every body of `tree` (plate coordinates) keeps out of the model, tip apart: swept as
/// beams with the profile's clearance (ADR 0077), then drawn bodies are tested too (ADR 0337).
pub fn fits(tree: &SupportTree, placed: &Placed, profile: &SupportProfile) -> bool {
    fit(tree, placed, profile).is_ok()
}

/// [`fits`], or the first reason `tree` (plate) does not.
fn fit(tree: &SupportTree, placed: &Placed, profile: &SupportProfile) -> Result<(), Refused> {
    for (child, parent) in tree.struts() {
        strut_fit(tree, child, parent, placed, profile)?;
    }
    trunk_fit(tree, placed, profile)?;
    if sound(tree, Profiles::single(profile), placed) {
        Ok(())
    } else {
        Err(Refused::Collides)
    }
}

/// Whether `after`, carried from `before` (both model space), may stand: it [`fits`], or
/// `before` did not and nothing the carry moved runs into the model or leans too far (ADR 0438);
/// else why not (ADR 0441).
pub fn allowed(
    before: &SupportTree,
    after: &SupportTree,
    placed: &Placed,
    profile: &SupportProfile,
) -> Result<(), Refused> {
    let matrix = placed.transform.to_matrix();
    let (was, now) = (before.moved(matrix), after.moved(matrix));
    let fitted = fit(&now, placed, profile);
    if fitted.is_ok() || fits(&was, placed, profile) {
        return fitted;
    }
    let at = |tree: &SupportTree, node: usize| tree.nodes().get(node).map(|node| node.position);
    let moved = |node: usize| at(&was, node) != at(&now, node);
    for (child, parent) in now
        .struts()
        .filter(|(child, parent)| moved(*child) || moved(*parent))
    {
        strut_fit(&now, child, parent, placed, profile)?;
    }
    let trunk_moved = moved(now.root_index()) || was.landing() != now.landing();
    if trunk_moved {
        trunk_fit(&now, placed, profile)?;
    }
    Ok(())
}

/// Whether the strut from `child` down to `parent` of `tree` (plate) keeps out of the model and,
/// under a head, leans no further than [`MAX_LEAN_PER_DROP`] (ADR 0437).
fn strut_fit(
    tree: &SupportTree,
    child: usize,
    parent: usize,
    placed: &Placed,
    profile: &SupportProfile,
) -> Result<(), Refused> {
    let nodes = tree.nodes();
    let (top, bottom) = (nodes[child], nodes[parent]);
    let lean = top.position - bottom.position;
    let upright = top.is_leaf()
        || lean.truncate().length() <= lean.z.abs() * MAX_LEAN_PER_DROP + LEAN_SLACK_MM;
    if !upright {
        return Err(Refused::Leans);
    }
    let beam = Beam {
        from: top.position,
        to: bottom.position,
        from_radius_mm: top.radius_mm,
        to_radius_mm: bottom.radius_mm,
    }
    .with_clearance(profile.clearance_mm)
    .trimmed(if top.is_leaf() { head_mm(profile) } else { 0.0 });
    clear(placed, &beam).then_some(()).ok_or(Refused::Collides)
}

/// Whether the trunk of `tree` (plate) keeps out of the model, stands straight over a foot on
/// the plate (ADR 0437) or runs into a support within 45 degrees (ADR 0443), and that foot has
/// room.
fn trunk_fit(tree: &SupportTree, placed: &Placed, profile: &SupportProfile) -> Result<(), Refused> {
    let root = tree.root();
    let landing = tree.landing();
    let lean = root.position - landing.base;
    if landing.on_support && lean.truncate().length() > lean.z * MAX_LEAN_PER_DROP + LEAN_SLACK_MM {
        return Err(Refused::Leans);
    }
    let on_plate = !landing.on_model && !landing.in_plate;
    if on_plate {
        if (root.position - landing.base).truncate().length() >= LEAN_SLACK_MM {
            return Err(Refused::Leans);
        }
        if !foot_clear(placed, landing.pad_centre(), profile) {
            return Err(Refused::NoFooting);
        }
    }
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
    clear(placed, &trunk).then_some(()).ok_or(Refused::Collides)
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
/// nearest surface, or `None` when its column has nothing to stand on near there; `last` is
/// what the foot stood on as the carry last left it. [`allowed`] says whether it may stand there
/// (ADR 0095, 0438, 0441).
pub fn carried(
    tree: &SupportTree,
    last: Landing,
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
    snap_tips(&mut carried, tree, placed, matrix, inverse);
    if carried.landing() != tree.landing() {
        // A column carried stands on whatever is under it there or just beside, as growing would.
        let root = matrix.transform_point3(carried.root().position);
        let width = (carried.root().radius_mm, carried.tip_count());
        let last = landing_through(last, matrix);
        let (at, under) = footing_near(placed, root, width, profile, last)?;
        carried.move_node(carried.root_index(), inverse.transform_point3(at));
        carried.stand_on(landing_through(under, inverse));
    }
    Some(carried)
}

/// `landing` moved through `matrix`: between plate coordinates and the model's own space.
fn landing_through(landing: Landing, matrix: Mat4) -> Landing {
    Landing {
        base: matrix.transform_point3(landing.base),
        pad_offset: matrix
            .transform_vector3(landing.pad_offset.extend(0.0))
            .truncate(),
        pad_cut: landing.pad_cut.map(|cut| cut.moved(matrix)),
        ..landing
    }
}

/// `tree` (model space, as the answer) with tip `tip` put down at `to` (plate coordinates, nearest
/// surface), or why not; the neck goes with the tip, the rest follows [`carried`] and the joints
/// beside it give way as [`eased`] lets them. A small pillar is grown again from there (ADR 0442).
pub fn tip_carried(
    tree: &SupportTree,
    tip: usize,
    to: Vec3,
    placed: &Placed,
    profiles: Profiles,
) -> Result<SupportTree, Refused> {
    let inverse = placed.transform.inverse_matrix().ok_or(Refused::NoHold)?;
    let node = tree
        .nodes()
        .get(tip)
        .filter(|node| node.is_leaf())
        .ok_or(Refused::NoHold)?;
    let contact = on_model(placed, to).ok_or(Refused::NoHold)?;
    if is_pillar(tree, placed, profiles) {
        return regrown(tree, contact, placed, profiles);
    }
    let profile = profiles.of(tree.group());
    let neck = head_of(placed, contact, profile).map_err(|why| match why {
        Unstood::FacesUp => Refused::NoHold,
        Unstood::HeadBlocked | Unstood::NoGround => Refused::Collides,
    })?;

    let mut carried = tree.clone();
    carried.move_node(tip, inverse.transform_point3(contact));
    if let Some(parent) = node.parent {
        // A head with no room under it would run through the model.
        if !has_head(contact, neck) {
            return Err(Refused::Collides);
        }
        let shared =
            (tree.nodes().iter()).any(|other| other.parent == Some(parent) && other != node);
        if shared {
            // A neck other branches hang from is their joint: the tip takes a neck of its own.
            carried.bend_under(tip, inverse.transform_point3(neck), node.radius_mm);
        } else {
            carried.move_node(parent, inverse.transform_point3(neck));
        }
        let alone = parent == tree.root_index() && tree.tip_count() == 1;
        let radius_mm = tree.root().radius_mm;
        if alone && let Some(under) = footing_under(placed, neck, radius_mm, 1, profile) {
            carried.stand_on(landing_through(under, inverse));
        }
    }
    eased(tree, &carried, placed, profile)
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

/// Pulls every tip the carry moved from where it stood in `was` back onto the surface it holds
/// up: a support that has stopped touching the model holds nothing, and one carried into it is
/// not printable.
fn snap_tips(
    tree: &mut SupportTree,
    was: &SupportTree,
    placed: &Placed,
    matrix: Mat4,
    inverse: Mat4,
) {
    let tips: Vec<(usize, Vec3)> = tree
        .nodes()
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            node.is_leaf() && was.nodes().get(*index).map(|was| was.position) != Some(node.position)
        })
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
