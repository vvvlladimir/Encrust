//! Placing a support by hand, taking hold of one and carrying it. The rule a carried
//! support is held to lives in `core_supports::carried`; see ADR 0095 and 0208.

use std::sync::Arc;

use core_geometry::{PlacedHit, Scalar, Transform, Vec3};
use core_supports::{Part, Profiles, WallSide, inline, preview, stands, support_under};
use core_volume::markers;

use crate::camera::OrbitCamera;
use crate::panels::Window;
use crate::pick::ray_through;
use crate::render::{Ghost, GhostShape};
use crate::scene::{ObjectId, SceneObject};
use crate::state::{Doc, Tools};
use crate::supports::Picked;
use crate::ui::theme;
use crate::viewport_input::Pointer;
use crate::workspace::Tool;

use super::picking::{aim_slack_mm, grab_under_cursor, surface_under};

/// A ray flatter than this against a plane has no crossing worth using: a part dragged
/// edge-on to it would shoot off to the horizon.
const PLATE_GRAZE: f32 = 1e-3;

/// Radius of a dot marking where a line of supports would stand, millimetres.
const LINE_DOT_MM: Scalar = 0.4;

/// Stands a support where the model was clicked and selects it; one that would not stand is
/// skipped. With `line`, a line of them runs from the last placed (`docs/decisions/0330`).
pub(super) fn place_support_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    line: bool,
) {
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
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
    let placed = object.support_placed().blocking(keep_out.as_ref());
    let wanted = match line_start(window, id).filter(|_| line) {
        Some(from) => inline(
            &placed,
            from,
            hit.point,
            window.tools.supports.inline_spacing_mm,
            line_side(object, from).as_ref(),
        ),
        None => vec![hit.point],
    };
    let standing: Vec<Vec3> = wanted
        .into_iter()
        .filter(|contact| stands(&placed, *contact, group, profiles))
        .collect();
    let Some(last) = standing.last() else {
        return;
    };
    let into_model = object.transform.to_matrix().inverse();
    window.tools.supports.last_placed = Some((id, into_model.transform_point3(*last)));
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        for contact in standing {
            object.supports.add(contact, transform, group);
        }
    }
}

pub(super) fn line_start(window: &Window, id: ObjectId) -> Option<Vec3> {
    window.tools.supports.line_start(&window.doc.scene, id)
}

/// The side of a hollow model's wall a line starting at `from`, in plate coordinates,
/// keeps to: the one that support stands on.
fn line_side(object: &SceneObject, from: Vec3) -> Option<WallSide> {
    let into_model = object.transform.to_matrix().inverse();
    object.wall_side(object.holds_inside(into_model.transform_point3(from)))
}

/// A dot wherever a shift-click at `hit` would stand a support of the line, in the active
/// group's tint, or the lone support a click would stand when no line starts on that model.
pub(super) fn line_ghost(window: &Window, id: ObjectId, hit: &PlacedHit) -> Option<Ghost> {
    let Some(from) = line_start(window, id) else {
        return support_ghost(window, id, hit);
    };
    let object = window.doc.scene.get(id)?;
    let table = window.tools.supports.table_for(object);
    let profiles = Profiles::new(&table)?;
    let keep_out = object.supports.keep_out(&object.mesh, object.transform);
    let placed = object.support_placed().blocking(keep_out.as_ref());
    let active = window.tools.supports.active;
    let dots = inline(
        &placed,
        from,
        hit.point,
        window.tools.supports.inline_spacing_mm,
        line_side(object, from).as_ref(),
    )
    .into_iter()
    .filter(|contact| stands(&placed, *contact, active, profiles))
    .map(|contact| (contact, LINE_DOT_MM));
    Some(Ghost {
        shape: GhostShape::Body {
            mesh: Arc::new(markers(dots)?),
            transform: Transform::default(),
        },
        colour: theme::support_tint(usize::from(active)),
    })
}

/// The support a click at `hit` on object `id` would stand, in the active group's tint,
/// or in red where it would not stand.
pub(super) fn support_ghost(window: &Window, id: ObjectId, hit: &PlacedHit) -> Option<Ghost> {
    let object = window.doc.scene.get(id)?;
    let table = window.tools.supports.table_for(object);
    let profiles = Profiles::new(&table)?;
    let keep_out = object.supports.keep_out(&object.mesh, object.transform);
    let placed = object.support_placed().blocking(keep_out.as_ref());
    let active = window.tools.supports.active;
    let shown = preview(&placed, hit.point, active, profiles);
    let colour = if shown.stands {
        theme::support_tint(usize::from(active))
    } else {
        theme::colors().danger
    };
    Some(Ghost {
        shape: GhostShape::Body {
            mesh: Arc::new(shown.mesh),
            transform: Transform::default(),
        },
        colour,
    })
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

/// Picks and carries the support parts the Edit mode works on; answers whether it has the
/// pointer. A press on a part carries it and the picked set; shift toggles; a miss is the camera's.
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
        // A Cmd press is the click that picks a model, not a hold on a support.
        let on_plate = pointer.over_viewport && !pointer.command && !window.view.gizmo.is_focused();
        window.tools.supports.stroke =
            on_plate && take_hold(window, viewport, cursor, pointer.shift);
        return window.tools.supports.stroke;
    }
    if !window.tools.supports.stroke {
        return false;
    }

    // A shift press only picks and sets no anchor: it must not carry the tip left in hand.
    let carrying = window.tools.supports.anchor.is_some();
    if let Some(tip) = lone_tip(window).filter(|_| carrying) {
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

/// Puts the held tip where the cursor points on its own model, else on the nearest surface
/// at the tip's depth; answers whether it moved. See `docs/decisions/0244`.
fn carry_tip(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2, tip: Picked) -> bool {
    let Part::Node(node) = tip.part else {
        return false;
    };
    let camera = &window.view.camera;
    let pointed = surface_under(window, viewport, cursor)
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
    let placed = object.support_placed().blocking(keep_out.as_ref());
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

/// Carries what is held by `step`, or as much as the model allows: whole, else across the
/// plate, else its rise alone, so a part slides along an overhang.
fn carry_as_far_as_allowed(window: &mut Window, step: Vec3) -> Option<Vec3> {
    let across = Vec3::new(step.x, step.y, 0.0);
    let rise = Vec3::new(0.0, 0.0, step.z);
    [step, across, rise]
        .into_iter()
        .filter(|part| *part != Vec3::ZERO)
        .find(|part| carry_parts(window, *part))
}

/// Takes hold of the part under the cursor and freezes its support against rebuilds; answers
/// whether the press landed on a support. With `add` the part is only toggled in the set.
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

    // Asked of the frozen tree, which the freeze renumbered, and from the cursor level with
    // the part: a stick taken by its end must not jump to put its middle under the cursor.
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

/// Moves every held part by `step`, plate millimetres; answers whether anything moved. Each
/// support is held to `core_supports::carried`; see `docs/decisions/0095`.
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
        let placed = object.support_placed().blocking(keep_out.as_ref());
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
    let mut done: Vec<crate::scene::ObjectId> = Vec::new();
    for picked in window.tools.supports.picked.clone() {
        if done.contains(&picked.id) {
            continue;
        }
        done.push(picked.id);
        if let Some(object) = window.doc.scene.get_mut(picked.id) {
            let table = window.tools.supports.table_for(object);
            object.refresh_supports(&table);
        }
    }
}
