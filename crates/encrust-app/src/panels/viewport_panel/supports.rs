//! Placing a support by hand, taking hold of one and carrying it. The rule a carried
//! support is held to lives in `core_supports::carried`; see ADR 0095 and 0208.

use core_geometry::Vec3;
use core_supports::{Part, Placed, Preview, Profiles, preview, stands, support_under};

use crate::camera::OrbitCamera;
use crate::panels::Window;
use crate::pick::{pick_surface, ray_through};
use crate::scene::ObjectId;
use crate::state::{Doc, Tools};
use crate::supports::{Picked, with_own};
use crate::viewport_input::Pointer;
use crate::workspace::Tool;

use super::picking::{aim_slack_mm, aims_elsewhere, grab_under_cursor};

/// A ray flatter than this against a plane has no crossing worth using: a part dragged
/// edge-on to it would shoot off to the horizon.
const PLATE_GRAZE: f32 = 1e-3;

/// Stands a support where the model was clicked, and selects what it was put on so that
/// the inspector is talking about the same object. A support that would not stand there
/// is not put down: the preview has already shown it red.
pub(super) fn place_support_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    window.doc.scene.select(Some(id));
    let group = window.tools.supports.active;
    let Some(object) = window.doc.scene.get(id) else {
        return;
    };
    let table = window.tools.supports.table_for(object);
    let Some(profiles) = Profiles::new(&table) else {
        return;
    };
    let keep_out = object.supports.keep_out(&object.mesh, object.transform);
    let placed =
        Placed::new(&object.mesh, &object.bvh, object.transform).blocking(keep_out.as_ref());
    if !stands(&placed, hit.point, group, profiles) {
        return;
    }
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.supports.add(hit.point, transform, group);
    }
}

/// What a click would stand where the cursor is, or `None` when it would put nothing
/// down: off every model, or on one the click would only pick.
pub(super) fn preview_under_cursor(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<Preview> {
    let (id, hit) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)?;
    if aims_elsewhere(&window.doc.scene, Some(id)) {
        return None;
    }
    let object = window.doc.scene.get(id)?;
    let table = window.tools.supports.table_for(object);
    let profiles = Profiles::new(&table)?;
    let keep_out = object.supports.keep_out(&object.mesh, object.transform);
    let placed =
        Placed::new(&object.mesh, &object.bvh, object.transform).blocking(keep_out.as_ref());
    Some(preview(
        &placed,
        hit.point,
        window.tools.supports.active,
        profiles,
    ))
}

/// Takes away the support under the cursor. Objects are searched in the order they were
/// imported, which is the order the viewport draws them in.
pub(super) fn remove_support_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) {
    let Some((id, hit)) = grab_under_cursor(window, viewport, cursor) else {
        return;
    };
    let Some(object) = window.doc.scene.get_mut(id) else {
        return;
    };

    // A frozen support is geometry of its own and goes whole; one still grown from points
    // goes by the point it grew from, and a shared trunk names no single one of them.
    match object.supports.frozen_of(hit.tree) {
        Some(frozen) => {
            object.supports.remove_frozen(frozen);
            window.tools.supports.picked.retain(|picked| {
                picked.id != id || (picked.frozen != frozen && picked.frozen < frozen)
            });
        }
        None => {
            let Some(ray) = ray_through(&window.view.camera, viewport, cursor) else {
                return;
            };
            let table = window.tools.supports.table_for(object);
            let Some(profiles) = Profiles::new(&table) else {
                return;
            };
            let slack_mm = aim_slack_mm(&window.view.camera, viewport, cursor);
            if let Some(point) = support_under(object.supports.trees(), &ray, profiles, slack_mm) {
                object.supports.remove(point);
            }
        }
    }
}

/// Picks and carries the parts of a support the Edit mode is working on, and answers
/// whether it has the pointer.
///
/// A press that lands on a part of a support takes hold of it and the same drag carries
/// it, with everything else picked, until the button comes up — wherever the cursor goes
/// meanwhile, so the camera never turns under a support in hand. With shift the press
/// only adds the part to what is picked or takes it back out. A press that misses lets go
/// and leaves the drag to the camera.
pub(super) fn drag_support(window: &mut Window, viewport: egui::Rect, pointer: &Pointer) -> bool {
    if *window.tool != Tool::Supports {
        return false;
    }
    if !pointer.primary_down {
        window.tools.supports.stroke = false;
        window.tools.supports.anchor = None;
        return false;
    }
    let Some(cursor) = pointer.position.map(|at| egui::Pos2::new(at.x, at.y)) else {
        return window.tools.supports.stroke;
    };

    if pointer.pressed {
        let on_plate = pointer.over_viewport && !window.view.gizmo.is_focused();
        window.tools.supports.stroke =
            on_plate && take_hold(window, viewport, cursor, pointer.shift);
        return window.tools.supports.stroke;
    }
    if !window.tools.supports.stroke {
        return false;
    }

    if let Some(tip) = lone_tip(window) {
        if carry_tip(window, viewport, cursor, tip) {
            rebuild_held(window);
        }
        return true;
    }

    // Everything held moves by the step the cursor took since the last one kept, on the
    // plane through where it was pressed, facing the camera.
    if let Some(anchor) = window.tools.supports.anchor
        && let Some(now) = on_camera_plane(&window.view.camera, viewport, cursor, anchor)
        && let Some(taken) = carry_as_far_as_allowed(window, now - anchor)
    {
        // The anchor follows by what was taken and no more, so a step the model refused
        // is asked again with the next one rather than lost.
        window.tools.supports.anchor = Some(anchor + taken);
        rebuild_held(window);
    }
    true
}

/// The tip in hand, when a tip is the one thing held: it is carried to where the cursor
/// points rather than by the step the cursor took.
fn lone_tip(window: &Window) -> Option<Picked> {
    let [picked] = window.tools.supports.picked.as_slice() else {
        return None;
    };
    let Part::Node(node) = picked.part else {
        return None;
    };
    let tree = window
        .doc
        .scene
        .get(picked.id)?
        .supports
        .frozen()
        .get(picked.frozen)?;
    tree.nodes().get(node)?.is_leaf().then_some(*picked)
}

/// Puts the held tip where the cursor points on its own model, or failing that on the
/// surface nearest the cursor at the depth the tip stands at. Answers whether it moved.
///
/// A tip has to touch what it holds, so it is aimed like a new support rather than slid
/// by a step: the hand follows the cursor, not the surface. The second aim is what keeps
/// it following past the model's outline, and from a view the held face is hidden in.
/// See `docs/decisions/0244`.
fn carry_tip(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2, tip: Picked) -> bool {
    let Part::Node(node) = tip.part else {
        return false;
    };
    let camera = &window.view.camera;
    let pointed = pick_surface(&window.doc.scene, camera, viewport, cursor)
        .filter(|(id, _)| *id == tip.id)
        .map(|(_, hit)| hit.point);
    let beside = part_position(window.doc, tip)
        .and_then(|stood| on_camera_plane(camera, viewport, cursor, stood));

    let Some(table) = window
        .doc
        .scene
        .get(tip.id)
        .map(|object| window.tools.supports.table_for(object))
    else {
        return false;
    };
    let Some(profiles) = Profiles::new(&table) else {
        return false;
    };
    let Some(object) = window.doc.scene.get_mut(tip.id) else {
        return false;
    };
    let Some(standing) = object.supports.frozen().get(tip.frozen).cloned() else {
        return false;
    };
    let keep_out = object.supports.keep_out(&object.mesh, object.transform);
    let placed =
        Placed::new(&object.mesh, &object.bvh, object.transform).blocking(keep_out.as_ref());
    let profile = profiles.of(standing.group());
    let Some(carried) = pointed
        .into_iter()
        .chain(beside)
        .find_map(|to| core_supports::tip_carried(&standing, node, to, &placed, profile))
        .filter(|carried| *carried != standing)
    else {
        return false;
    };
    match object.supports.frozen_mut(tip.frozen) {
        Some(tree) => {
            *tree = carried;
            true
        }
        None => false,
    }
}

/// Carries what is held by `step`, or by as much of it as the model allows: the whole of
/// it, else what it covers across the plate, else its rise alone. Answers what was taken.
///
/// A step is measured on a plane facing the camera, so it nearly always rises or falls a
/// little, and a part under an overhang would otherwise stop dead at the first touch
/// instead of sliding along it.
fn carry_as_far_as_allowed(window: &mut Window, step: Vec3) -> Option<Vec3> {
    let across = Vec3::new(step.x, step.y, 0.0);
    let rise = Vec3::new(0.0, 0.0, step.z);
    [step, across, rise]
        .into_iter()
        .filter(|part| *part != Vec3::ZERO)
        .find(|part| carry_parts(window, *part))
}

/// Takes hold of the part under the cursor, freezing the support it belongs to so that
/// the next rebuild cannot undo what is about to be done to it. Answers whether the press
/// landed on a support, which is whether the drag that follows is this tool's.
///
/// A part not yet picked replaces what was; one already picked keeps the others with it,
/// so several are carried by any one of them. With `add` the part is only put into the
/// set or taken out, and the drag carries nothing.
fn take_hold(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2, add: bool) -> bool {
    window.tools.supports.anchor = None;
    let Some((id, hit)) = grab_under_cursor(window, viewport, cursor) else {
        if !add {
            window.tools.supports.picked.clear();
        }
        return false;
    };

    let Some(object) = window.doc.scene.get_mut(id) else {
        return false;
    };
    let transform = object.transform;
    let Some(frozen) = object.supports.freeze(hit.tree, transform) else {
        return false;
    };
    window.doc.scene.select(Some(id));

    let picked = Picked {
        id,
        frozen,
        part: hit.part,
    };
    let held = &mut window.tools.supports.picked;
    let at = held.iter().position(|other| *other == picked);
    match (add, at) {
        (true, Some(at)) => {
            held.remove(at);
        }
        (true, None) => held.push(picked),
        (false, Some(_)) => {}
        (false, None) => {
            held.clear();
            held.push(picked);
        }
    }

    // The freeze renumbered the trees, so where the part stands is asked of the frozen
    // tree rather than of what was hit. The drag is measured from the point of the cursor
    // level with the part, not from the part's middle: a stick taken by its end must not
    // jump to put its middle under the cursor.
    if !add {
        window.tools.supports.anchor = part_position(window.doc, picked)
            .and_then(|part| on_camera_plane(&window.view.camera, viewport, cursor, part));
    }
    true
}

/// Where a held part stands, in plate coordinates.
fn part_position(doc: &Doc, picked: Picked) -> Option<Vec3> {
    let object = doc.scene.get(picked.id)?;
    let matrix = object.transform.to_matrix();
    let tree = object.supports.frozen().get(picked.frozen)?;
    let node = |index: usize| tree.nodes().get(index).map(|node| node.position);

    let local = match picked.part {
        Part::Node(index) => node(index)?,
        Part::Strut(child) => {
            let parent = tree.nodes().get(child)?.parent?;
            node(child)?.lerp(node(parent)?, 0.5)
        }
        Part::Trunk => tree.root().position.lerp(tree.landing().base, 0.5),
        Part::Foot => tree.landing().base,
    };
    Some(matrix.transform_point3(local))
}

/// Where the cursor now points, on the plane through `anchor` that faces the camera.
fn on_camera_plane(
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    anchor: Vec3,
) -> Option<Vec3> {
    let ray = ray_through(camera, viewport, cursor)?;
    let normal = (camera.sight_to(anchor) - anchor).normalize_or_zero();
    let facing = ray.direction.dot(normal);
    if facing.abs() < PLATE_GRAZE {
        return None;
    }
    Some(ray.at((anchor - ray.origin).dot(normal) / facing))
}

/// Moves every held part by `step`, in plate millimetres, and answers whether anything
/// moved.
///
/// Each support is carried as a whole and then held to the rule the automatic run places
/// by, which is `core_supports::carried`: a step the model refuses leaves that support
/// where it was. See `docs/decisions/0095`.
fn carry_parts(window: &mut Window, step: Vec3) -> bool {
    let mut moved = false;
    for (id, frozen) in held_trees(window.tools) {
        let Some(table) = window
            .doc
            .scene
            .get(id)
            .map(|object| window.tools.supports.table_for(object))
        else {
            continue;
        };
        let Some(profiles) = Profiles::new(&table) else {
            continue;
        };
        let parts: Vec<Part> = window
            .tools
            .supports
            .picked
            .iter()
            .filter(|picked| picked.id == id && picked.frozen == frozen)
            .map(|picked| picked.part)
            .collect();
        let Some(object) = window.doc.scene.get_mut(id) else {
            continue;
        };
        let Some(standing) = object.supports.frozen().get(frozen).cloned() else {
            continue;
        };

        let keep_out = object.supports.keep_out(&object.mesh, object.transform);
        let placed =
            Placed::new(&object.mesh, &object.bvh, object.transform).blocking(keep_out.as_ref());
        let Some(carried) = core_supports::carried(
            &standing,
            &parts,
            step,
            &placed,
            profiles.of(standing.group()),
        ) else {
            continue;
        };

        if let Some(tree) = object.supports.frozen_mut(frozen) {
            *tree = carried;
            moved = true;
        }
    }
    moved
}

/// Every frozen tree something is held on, each once.
fn held_trees(tools: &Tools) -> Vec<(ObjectId, usize)> {
    let mut held: Vec<(ObjectId, usize)> = tools
        .supports
        .picked
        .iter()
        .map(|picked| (picked.id, picked.frozen))
        .collect();
    held.sort_unstable();
    held.dedup();
    held
}

/// Rebuilds the supports of every object something is held on, in this frame rather than
/// the next: a support that vanishes under the cursor cannot be aimed.
fn rebuild_held(window: &mut Window) {
    let plate = window.tools.supports.table();
    let raft = window.tools.supports.raft;
    let mut done: Vec<crate::scene::ObjectId> = Vec::new();
    for picked in window.tools.supports.picked.clone() {
        if done.contains(&picked.id) {
            continue;
        }
        done.push(picked.id);
        if let Some(object) = window.doc.scene.get_mut(picked.id) {
            let mesh = std::sync::Arc::clone(&object.mesh);
            let bvh = std::sync::Arc::clone(&object.bvh);
            let table = with_own(plate.clone(), &object.own, raft);
            object
                .supports
                .refresh(&mesh, &bvh, object.transform, &table);
        }
    }
}
