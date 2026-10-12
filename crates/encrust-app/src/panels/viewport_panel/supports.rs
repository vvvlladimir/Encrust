//! Placing a support by hand, taking hold of one and carrying it. The rule a carried
//! support is held to lives in `core_supports::carried`; see ADR 0095 and 0208.

use core_geometry::Vec3;
use core_supports::{Part, Placed, Profiles, support_under};

use crate::camera::OrbitCamera;
use crate::panels::Window;
use crate::pick::{pick_surface, ray_through};
use crate::scene::ObjectId;
use crate::state::{Doc, Tools};
use crate::supports::Picked;
use crate::viewport_input::Pointer;
use crate::workspace::Tool;

use super::picking::grab_under_cursor;

/// A ray flatter than this against a plane has no crossing worth using: a part dragged
/// edge-on to it would shoot off to the horizon.
const PLATE_GRAZE: f32 = 1e-3;

/// Stands a support where the model was clicked, and selects what it was put on so that
/// the inspector is talking about the same object.
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
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.supports.add(hit.point, transform, group);
    }
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
            let table = window.tools.supports.table();
            let Some(profiles) = Profiles::new(&table) else {
                return;
            };
            if let Some(point) = support_under(object.supports.trees(), &ray, profiles) {
                object.supports.remove(point);
            }
        }
    }
}

/// Picks and carries the parts of a support the Edit mode is working on, and answers
/// whether it has the pointer.
///
/// The press picks: one that lands on a part of a support takes hold of it — with shift,
/// alongside what was already held — and drags it until the button comes up; one that
/// misses lets go and leaves the drag to the camera.
pub(super) fn drag_support(window: &mut Window, viewport: egui::Rect, pointer: &Pointer) -> bool {
    let out = *window.tool == Tool::Supports;
    let Some(position) = pointer.position.filter(|_| {
        out && pointer.primary_down && pointer.over_viewport && !window.view.gizmo.is_focused()
    }) else {
        if out && !pointer.primary_down {
            window.tools.supports.stroke = false;
            window.tools.supports.anchor = None;
        }
        return false;
    };

    let cursor = egui::Pos2::new(position.x, position.y);
    if pointer.pressed {
        let carries = take_hold(window, viewport, cursor, pointer.shift);
        // The pointer is held whenever the press landed on a support, so picking one
        // never turns the camera; only a press on a part already picked carries it.
        window.tools.supports.stroke = carries;
        return carries || !window.tools.supports.picked.is_empty();
    }
    if !window.tools.supports.stroke {
        return false;
    }

    // Everything held moves by the step the part under the cursor took, measured on the
    // plane through where it stood, facing the camera.
    if let Some(anchor) = window.tools.supports.anchor
        && let Some(now) = on_camera_plane(&window.view.camera, viewport, cursor, anchor)
    {
        // The anchor only follows the cursor when the step was taken: a step the model
        // refused must not leave the two drifting apart.
        if carry_parts(window, now - anchor) {
            window.tools.supports.anchor = Some(now);
            rebuild_held(window);
        }
    }
    true
}

/// Takes hold of the part under the cursor, freezing the support it belongs to so that
/// the next rebuild cannot undo what is about to be done to it. Answers whether the drag
/// that follows carries anything.
///
/// A part is picked first and carried second: a press on something not yet picked only
/// picks it, and the press after that is the one that moves it. The pointer is held
/// either way, so choosing a part never spins the camera.
fn take_hold(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2, add: bool) -> bool {
    let Some((id, hit)) = grab_under_cursor(window, viewport, cursor) else {
        if !add {
            window.tools.supports.picked.clear();
        }
        window.tools.supports.anchor = None;
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
    let carries = held.contains(&picked) && !add;
    if add {
        if let Some(at) = held.iter().position(|other| *other == picked) {
            held.remove(at);
        } else {
            held.push(picked);
        }
    } else if !carries {
        held.clear();
        held.push(picked);
    }

    // The freeze renumbered the trees, so where the part stands is asked of the frozen
    // tree rather than of what was hit.
    window.tools.supports.anchor = carries.then(|| part_position(window.doc, picked)).flatten();
    window.tools.supports.anchor.is_some()
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
    let table = window.tools.supports.table();
    let Some(profiles) = Profiles::new(&table) else {
        return false;
    };

    let mut moved = false;
    for (id, frozen) in held_trees(window.tools) {
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
    let table = window.tools.supports.table();
    let mut done: Vec<crate::scene::ObjectId> = Vec::new();
    for picked in window.tools.supports.picked.clone() {
        if done.contains(&picked.id) {
            continue;
        }
        done.push(picked.id);
        if let Some(object) = window.doc.scene.get_mut(picked.id) {
            let mesh = std::sync::Arc::clone(&object.mesh);
            let bvh = std::sync::Arc::clone(&object.bvh);
            object
                .supports
                .refresh(&mesh, &bvh, object.transform, &table);
        }
    }
}
