use core_geometry::Vec2;
use egui::{PointerButton, Sense};

use crate::camera::OrbitCamera;
use crate::drain::Placing;
use crate::panels::{Window, section};
use crate::plate::BuildPlate;
use crate::render::{Banding, Shading, ViewportCallback};
use crate::scene::{ObjectId, Scene};
use crate::state::View;
use crate::viewport_input::{Drag, Pointer};
use crate::workspace::Tool;

mod hollow;
mod overlays;
mod paint;
mod picking;
mod supports;

use hollow::{
    add_channel_point_under_cursor, place_blocker_under_cursor, place_drain_under_cursor,
    remove_blocker_under_cursor, remove_drain_under_cursor,
};
use overlays::{cut_line, draw_bounds, draw_cut_plane, draw_facet, draw_measure, draw_picked};
use paint::{draw_brush, paint_stroke};
use picking::{
    lay_face_under_cursor, measure_under_cursor, point_at_face, select_under_cursor, takes_the_pick,
};
use supports::{drag_support, place_support_under_cursor, remove_support_under_cursor};

/// Radians of orbit per point of cursor movement. A drag across a 600 point wide panel is
/// a little under a full turn.
const ORBIT_RAD_PER_POINT: f32 = 0.009;

/// Zoom is exponential in the scroll delta so that a notch changes the view by the same
/// proportion however close the camera already is.
const ZOOM_PER_SCROLL_POINT: f32 = 0.0015;

/// Draws the plate and everything on it, and returns the rectangle it took, which is what
/// the cards floating over it are anchored to.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) -> egui::Rect {
    // Hover only: the camera reads the raw pointer instead of this response, because the
    // gizmo takes the pointer away from it. See `viewport_input.rs`.
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());

    // The sheet is modal: a click meant for it must not orbit the plate or drill a hole
    // in the model. The pointer is read from the raw input, which no modal layer covers.
    let pointer = if window.view.options.sheet {
        Pointer::idle()
    } else {
        pointer_state(ui, rect)
    };
    // What the gizmo reports is one frame old, because it is drawn after the input that
    // has to yield to it is read. That only matters on the frame a handle is crossed.
    let gesture = window
        .view
        .input
        .update(&pointer, window.view.gizmo.is_focused());

    // The brush and the tip being dragged take the primary button only for a gesture that
    // began on something of theirs, so a drag that starts on empty plate still turns the
    // camera while either is out.
    let painting = paint_stroke(ui, window, rect, &pointer) || drag_support(window, rect, &pointer);

    steer_camera(window.view, rect, &pointer, gesture.drag, painting);
    if gesture.clicked
        && let Some(position) = pointer.position
    {
        click(ui, window, rect, egui::Pos2::new(position.x, position.y));
    }
    if pointer.over_viewport
        && ui.input(|i| i.pointer.button_double_clicked(PointerButton::Primary))
    {
        frame_view(
            &window.doc.scene,
            &window.doc.plate,
            &mut window.view.camera,
        );
    }

    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return rect;
    }
    paint_plate(ui, window, rect);
    draw_tool_overlays(ui, window, rect, &pointer);
    rect
}

/// Orbits, pans and zooms the camera.
fn steer_camera(view: &mut View, rect: egui::Rect, pointer: &Pointer, drag: Drag, painting: bool) {
    match drag {
        Drag::Orbit if painting => {}
        Drag::Orbit => view.camera.orbit(
            -pointer.delta.x * ORBIT_RAD_PER_POINT,
            pointer.delta.y * ORBIT_RAD_PER_POINT,
        ),
        Drag::Pan => view.camera.pan(pointer.delta, rect.height()),
        Drag::None => {}
    }

    if pointer.over_viewport && pointer.scroll != 0.0 {
        view.camera
            .zoom((-pointer.scroll * ZOOM_PER_SCROLL_POINT).exp());
    }
}

/// What a click on the plate does, which is the tool's to say.
fn click(ui: &egui::Ui, window: &mut Window, rect: egui::Rect, cursor: egui::Pos2) {
    if *window.tool != Tool::Select && takes_the_pick(window, rect, cursor) {
        return;
    }
    match *window.tool {
        // Alt is what every modelling tool uses for "the opposite of this click", and
        // the other buttons are already spoken for by the camera.
        // A stroke of the brush has already painted whatever the click landed on.
        Tool::Supports if window.tools.supports.placing.paints() => {}
        Tool::Supports if ui.input(|input| input.modifiers.alt) => {
            remove_support_under_cursor(window, rect, cursor);
        }
        // The drag has already taken hold of whatever the press landed on.
        Tool::Supports if window.tools.supports.placing.edits() => {}
        Tool::Supports => place_support_under_cursor(window, rect, cursor),
        Tool::Hollow if ui.input(|input| input.modifiers.alt) => {
            remove_blocker_under_cursor(window, rect, cursor);
        }
        Tool::Hollow => place_blocker_under_cursor(window, rect, cursor),
        Tool::Drain if ui.input(|input| input.modifiers.alt) => {
            remove_drain_under_cursor(window, rect, cursor);
        }
        Tool::Drain if window.tools.drain.placing == Placing::Channel => {
            add_channel_point_under_cursor(window, rect, cursor);
        }
        Tool::Drain => place_drain_under_cursor(window, rect, cursor),
        Tool::Measure if ui.input(|input| input.modifiers.alt) => window.tools.measure.clear(),
        Tool::Measure => measure_under_cursor(window, rect, cursor),
        Tool::Select if window.tools.orient.picking_face => {
            lay_face_under_cursor(window, rect, cursor);
        }
        _ => {
            let adding = ui.input(|input| input.modifiers.command || input.modifiers.shift);
            select_under_cursor(
                &mut window.doc.scene,
                &window.view.camera,
                rect,
                cursor,
                adding,
            );
        }
    }
}

/// Hands the 3D pass its frame: the models, the plate, the section and the shading.
fn paint_plate(ui: &egui::Ui, window: &Window, rect: egui::Rect) {
    // What needs holding up is shown while the Supports tool is the one in hand, at the
    // angle its own profile is set to; see `docs/decisions/0034`.
    let overhang_deg =
        (*window.tool == Tool::Supports).then_some(window.tools.supports.profile.max_overhang_deg);
    crate::render::prime_label(ui.painter());
    let callback = ViewportCallback::new(
        &window.doc.scene,
        &window.doc.plate,
        &window.view.camera,
        window.view.options,
        Shading {
            overhang_deg,
            section_mm: section::cut_height(
                *window.mode,
                &window.view.section,
                &window.machine.preview,
            ),
            banding: Banding {
                ranges: &window.machine.slicing.exposure,
                floor_mm: window.machine.slicing.band_floor_mm(),
            },
            textured: *window.tool == Tool::Relief,
            cut_line: cut_line(window),
        },
        rect,
    );
    ui.painter()
        .add(egui_wgpu::Callback::new_paint_callback(rect, callback));
}

/// The chrome each tool draws over the 3D pass: bounds, gizmo, measurement, brush.
fn draw_tool_overlays(ui: &egui::Ui, window: &mut Window, rect: egui::Rect, pointer: &Pointer) {
    if *window.tool == Tool::Select {
        draw_bounds(ui, window, rect);
    }
    if *window.tool == Tool::Select && window.tools.orient.picking_face {
        let hovered = pointer.over_viewport.then_some(pointer.position).flatten();
        point_at_face(window, rect, hovered.map(|at| egui::pos2(at.x, at.y)));
        draw_facet(ui, window, rect);
    }
    // While a face is being picked the click is for the model, not for a handle over it.
    if !window.view.options.sheet
        && *window.tool == Tool::Select
        && !window.tools.orient.picking_face
    {
        show_gizmo(ui, window, rect);
    }
    if *window.tool == Tool::Measure {
        draw_measure(ui, window, rect);
    }
    if *window.tool == Tool::Cut {
        draw_cut_plane(ui, window, rect);
    }
    if *window.tool == Tool::Supports && window.tools.supports.placing.paints() {
        draw_brush(ui, window, rect);
    }
    if *window.tool == Tool::Supports && window.tools.supports.placing.edits() {
        draw_picked(ui, window, rect);
    }
}

/// Points the camera at everything on the plate, or at the plate itself when it is empty.
pub fn frame_view(scene: &Scene, plate: &BuildPlate, camera: &mut OrbitCamera) {
    match scene.world_bounds() {
        Some(bounds) => camera.frame(&bounds),
        None => *camera = OrbitCamera::framing_plate(plate),
    }
}

/// Reads this frame's pointer, in panel points, and whether it is over the 3D view.
fn pointer_state(ui: &egui::Ui, rect: egui::Rect) -> Pointer {
    // Read before the input is borrowed below: asking egui which layer is under a point
    // takes the same lock.
    let position = ui.ctx().input(|input| input.pointer.hover_pos());
    let over_viewport = over_the_model(
        rect,
        position,
        position.and_then(|at| ui.ctx().layer_id_at(at)),
        ui.layer_id(),
    );
    ui.input(|input| {
        let delta = input.pointer.delta();
        Pointer {
            over_viewport,
            position: position.map(|position| Vec2::new(position.x, position.y)),
            pressed: input.pointer.any_pressed(),
            primary_down: input.pointer.button_down(PointerButton::Primary),
            pan_button_down: input.pointer.button_down(PointerButton::Secondary)
                || input.pointer.button_down(PointerButton::Middle),
            shift: input.modifiers.shift,
            delta: Vec2::new(delta.x, delta.y),
            scroll: input.smooth_scroll_delta.y,
        }
    })
}

/// Whether the pointer is on the model rather than on something drawn over it.
///
/// A card, a popup, a menu or a floating window is its own egui layer above the
/// viewport's, and whatever egui has at this point owns the pointer: the camera must not
/// read a drag meant for a field in a popup. See `docs/decisions/0193`.
fn over_the_model(
    rect: egui::Rect,
    position: Option<egui::Pos2>,
    over: Option<egui::LayerId>,
    own: egui::LayerId,
) -> bool {
    position
        .is_some_and(|position| rect.contains(position) && over.is_none_or(|layer| layer == own))
}

/// Draws the handles over the selected objects and writes back what the user dragged.
fn show_gizmo(ui: &egui::Ui, window: &mut Window, viewport: egui::Rect) {
    let picked: Vec<ObjectId> = window.doc.scene.selection().to_vec();
    let pivots: Vec<core_geometry::Transform> = picked
        .iter()
        .filter_map(|id| window.doc.scene.get(*id))
        .map(|object| object.pivot())
        .collect();
    if pivots.is_empty() {
        return;
    }

    let Some(moved) = window
        .view
        .gizmo
        .show(ui, viewport, &window.view.camera, &pivots)
    else {
        return;
    };
    for (id, pivot) in picked.iter().zip(moved) {
        if let Some(object) = window.doc.scene.get_mut(*id) {
            object.settle(pivot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_an_empty_scene_falls_back_to_the_plate() {
        let plate = BuildPlate::default();
        let mut camera = OrbitCamera::default();
        frame_view(&Scene::default(), &plate, &mut camera);
        assert_eq!(camera.target, plate.center());
    }

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(400.0, 300.0))
    }

    #[test]
    fn the_plate_is_under_the_pointer_only_where_nothing_is_drawn_over_it() {
        let stage = egui::LayerId::background();
        let card = egui::LayerId::new(egui::Order::Middle, egui::Id::new("a card"));
        let inside = Some(egui::pos2(200.0, 100.0));

        assert!(over_the_model(viewport(), inside, Some(stage), stage));
        assert!(
            over_the_model(viewport(), inside, None, stage),
            "a point with no layer at all is the plate"
        );
        assert!(
            !over_the_model(viewport(), inside, Some(card), stage),
            "a card over the plate owns the pointer"
        );
        assert!(
            !over_the_model(viewport(), Some(egui::pos2(10.0, 10.0)), Some(stage), stage),
            "a panel beside the viewport is not the viewport"
        );
        assert!(!over_the_model(viewport(), None, None, stage));
    }
}
