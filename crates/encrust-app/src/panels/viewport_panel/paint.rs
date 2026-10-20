//! The brush: where it is drawn, and the faces a stroke marks or clears.

use core_geometry::{Adjacency, Scalar, Vec2};
use core_supports::Placed;

use crate::panels::Window;
use crate::supports::Placing;
use crate::ui::theme;
use crate::viewport_input::Pointer;
use crate::workspace::Tool;

use super::overlays::projector;
use super::picking::surface_under;

/// How many segments the brush ring is drawn with.
const BRUSH_SEGMENTS: u32 = 48;

/// How far apart, in panel points, the brush is put down along a drag.
const DAUB_STEP_POINTS: f32 = 4.0;

/// Draws the brush as a ring lying on the surface under the cursor, in the colour of the
/// patch it paints. Chrome over the 3D pass, like a measurement, so nothing hides it.
pub(super) fn draw_brush(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let Some(cursor) = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|position| viewport.contains(*position))
    else {
        return;
    };
    let Some((_, hit)) = surface_under(window, viewport, cursor) else {
        return;
    };

    let on_screen = projector(&window.view.camera, viewport);
    let (across, along) = hit.normal.any_orthonormal_pair();
    let (radius_mm, colour) = brush(ui, window);
    let ring: Vec<egui::Pos2> = (0..BRUSH_SEGMENTS)
        .filter_map(|step| {
            let angle = std::f32::consts::TAU * step as f32 / BRUSH_SEGMENTS as f32;
            on_screen(hit.point + (across * angle.cos() + along * angle.sin()) * radius_mm)
        })
        .collect();
    if ring.len() < BRUSH_SEGMENTS as usize {
        return;
    }

    let painter = ui.painter_at(viewport);
    painter.add(egui::Shape::closed_line(
        ring,
        egui::Stroke::new(1.5, colour),
    ));
}

/// The radius of the brush in hand, and the colour of what it paints: alt over a relief
/// takes the paint off.
fn brush(ui: &egui::Ui, window: &Window) -> (Scalar, egui::Color32) {
    let colors = theme::colors();
    if *window.tool == Tool::Relief {
        let colour = match ui.input(|input| input.modifiers.alt) {
            true => colors.text_mid,
            false => colors.accent,
        };
        return (window.tools.relief.state.brush_radius_mm, colour);
    }
    let colour = match window.tools.supports.placing {
        Placing::Block => colors.danger,
        Placing::Erase => colors.text_mid,
        Placing::Point | Placing::Paint => colors.accent,
    };
    (window.tools.supports.brush_radius_mm, colour)
}

/// Runs the brush for this frame and answers whether it has the pointer.
///
/// The stroke is decided on the frame the button goes down: a press that misses every
/// model leaves the drag to the camera, and one that lands on a model paints until the
/// button comes up.
pub(super) fn paint_stroke(
    ui: &egui::Ui,
    window: &mut Window,
    viewport: egui::Rect,
    pointer: &Pointer,
) -> bool {
    if *window.tool == Tool::Relief {
        return relief_stroke(ui, window, viewport, pointer);
    }
    let out = *window.tool == Tool::Paint && window.tools.supports.placing.paints();
    let Some(position) = pointer.position.filter(|_| {
        out && pointer.primary_down
            && pointer.over_viewport
            // Shift alone pans and Cmd alone picks; the two together fill a whole surface.
            && pointer.shift == pointer.command
            && !window.view.gizmo.is_focused()
    }) else {
        // Only the brush lets go here: the Edit mode keeps its own hold of the pointer.
        if out {
            window.tools.supports.stroke = false;
            window.tools.supports.last_daub = None;
        }
        return false;
    };

    let cursor = egui::Pos2::new(position.x, position.y);
    if pointer.pressed {
        window.tools.supports.stroke = surface_under(window, viewport, cursor).is_some();
        window.tools.supports.last_daub = None;
    }
    if !window.tools.supports.stroke {
        return false;
    }

    let modifiers = ui.input(|input| input.modifiers);
    // A drag moves further in one frame than the brush is wide, so the stroke is painted
    // along the line the cursor drew rather than only where the frames fell.
    let from = window.tools.supports.last_daub.unwrap_or(position);
    for along in daubs(from, position) {
        paint_under_cursor(window, viewport, along, modifiers.command, !modifiers.alt);
    }
    window.tools.supports.last_daub = Some(position);

    // Once for the frame rather than once a daub, and before the viewport is drawn
    // further down: a brush that shows up only when the button comes up cannot be aimed.
    for object in window.doc.scene.here_mut() {
        let mesh = std::sync::Arc::clone(&object.mesh);
        object.supports.refresh_patches(&mesh, object.transform);
    }
    true
}

/// Where the brush is put down along a drag from `from` to `to`: a drag moves further in
/// one frame than the brush is wide, so the stroke follows the line the cursor drew.
fn daubs(from: Vec2, to: Vec2) -> impl Iterator<Item = egui::Pos2> {
    let steps = (from.distance(to) / DAUB_STEP_POINTS).ceil().max(1.0) as u32;
    (1..=steps).map(move |step| {
        let along = from.lerp(to, step as f32 / steps as f32);
        egui::Pos2::new(along.x, along.y)
    })
}

/// The Relief brush for this frame, painting where the texture goes and taking it off
/// under alt, and whether it has the pointer. Decided on the press, as the support brush.
fn relief_stroke(
    ui: &egui::Ui,
    window: &mut Window,
    viewport: egui::Rect,
    pointer: &Pointer,
) -> bool {
    let out = window.tools.relief.paints();
    let Some(position) = pointer.position.filter(|_| {
        out && pointer.primary_down
            && pointer.over_viewport
            && !pointer.shift
            && !pointer.command
            && !window.view.gizmo.is_focused()
    }) else {
        window.tools.relief.stroke = false;
        window.tools.relief.last_daub = None;
        return false;
    };

    let cursor = egui::Pos2::new(position.x, position.y);
    if pointer.pressed {
        window.tools.relief.stroke = surface_under(window, viewport, cursor).is_some();
        window.tools.relief.last_daub = None;
    }
    if !window.tools.relief.stroke {
        return false;
    }

    let marked = !ui.input(|input| input.modifiers.alt);
    let radius_mm = window.tools.relief.state.brush_radius_mm;
    let from = window.tools.relief.last_daub.unwrap_or(position);
    for along in daubs(from, position) {
        let Some((id, hit)) = surface_under(window, viewport, along) else {
            continue;
        };
        if let Some(object) = window.doc.scene.get_mut(id) {
            let model = (&*object.mesh, &*object.bvh, object.transform);
            object
                .relief_painted
                .brush(model, hit.point, radius_mm, marked);
        }
    }
    window.tools.relief.last_daub = Some(position);
    true
}

/// Paints the model under the cursor, with the brush or — while `whole_face` is held —
/// with the surface the face it hit belongs to.
///
/// `marked` is false for the alt-click that takes the paint off again. The eraser takes
/// both paints off, whatever is held.
fn paint_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    whole_face: bool,
    marked: bool,
) {
    let Some((id, hit)) = surface_under(window, viewport, cursor) else {
        return;
    };
    let placing = window.tools.supports.placing;
    let (layers, marked): (&[bool], bool) = match placing {
        Placing::Erase => (&[false, true], false),
        _ => (&[placing.blocks()][..], marked),
    };
    let radius_mm = window.tools.supports.brush_radius_mm;
    let angle_deg = window.tools.supports.flood_angle_deg;
    let Some(object) = window.doc.scene.get_mut(id) else {
        return;
    };

    if whole_face {
        let adjacency = Adjacency::of(&object.mesh);
        for blocking in layers {
            object.supports.paint_face(
                &object.mesh,
                &adjacency,
                hit.face,
                angle_deg,
                *blocking,
                marked,
            );
        }
    } else {
        let placed = Placed::new(&object.mesh, &object.bvh, object.transform);
        for blocking in layers {
            object
                .supports
                .paint(&placed, hit.point, radius_mm, *blocking, marked);
        }
    }
}
