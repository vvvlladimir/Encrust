//! Placing a support by hand and taking one away; carrying one is `carry`.

use std::sync::Arc;

use core_geometry::{PlacedHit, Scalar, Transform, Vec3};
use core_supports::{Profiles, WallSide, inline, preview, stands, support_under};
use core_volume::markers;

use crate::panels::Window;
use crate::pick::ray_through;
use crate::render::{Ghost, GhostShape};
use crate::scene::{ObjectId, SceneObject};
use crate::ui::theme;

use super::picking::{Aimed, aim_slack, grab_under_cursor, surface_under};

mod carry;
pub(super) use carry::{carried_ghost, drag_support, show_carrying};

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
    let Some(Aimed { id, hit, .. }) = grab_under_cursor(window, viewport, cursor) else {
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
            // What stood on it goes back to the automatic run, which renumbers every frozen one.
            let thawed = window.tools.supports.thaw_unseated(object) > 0;
            window
                .tools
                .supports
                .picked
                .retain(|picked| picked.id != id || (!thawed && picked.frozen < frozen));
        }
        None => {
            let Some(ray) = ray_through(&window.view.camera, viewport, cursor) else {
                return;
            };
            let table = window.tools.supports.table_for(object);
            let Some(profiles) = Profiles::new(&table) else {
                return;
            };
            let slack = aim_slack(&window.view.camera, viewport, cursor);
            if let Some(point) = support_under(object.supports.trees(), &ray, profiles, slack) {
                object.supports.remove(point);
            }
        }
    }
}
