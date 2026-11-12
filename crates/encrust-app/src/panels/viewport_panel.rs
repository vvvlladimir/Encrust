use crate::relief::ReliefMode;
use core_geometry::{Vec2, Vec3};
use drawing::{ghost, paint_plate};
use egui::{PointerButton, Sense};

use crate::camera::OrbitCamera;
use crate::drain::Placing;
use crate::panels::Window;
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene};
use crate::state::{Tools, View};
use crate::viewport_input::{Drag, Pointer};
use crate::workspace::Tool;

mod bounds;
use bounds::draw_bounds;
mod gizmo_drag;
use gizmo_drag::draw_gizmo_drag;
use refusal::draw_refusal;
mod hollow;
mod levels;
mod marks;
use marks::fly_to_marked;
mod drawing;
mod overlays;
mod paint;
mod picking;
mod refusal;
mod repair;
mod support_ghost;
mod supports;
mod text;

use hollow::{
    add_channel_point_under_cursor, place_blocker_under_cursor, place_drain_under_cursor,
    remove_blocker_under_cursor, remove_drain_under_cursor,
};
use overlays::{draw_aimed, draw_cut_plane, draw_facet, draw_measure, draw_part_mark, draw_picked};
use paint::{draw_brush, paint_stroke};
use picking::{
    lay_face_under_cursor, mark_part_under_cursor, measure_under_cursor, point_at_face,
    select_under_cursor, takes_the_pick, zoom_anchor,
};
use supports::{
    drag_support, place_support_under_cursor, remove_support_under_cursor, show_carrying,
};

/// Radians of orbit per point of cursor movement. A drag across a 600 point wide panel is
/// a little under a full turn.
const ORBIT_RAD_PER_POINT: f32 = 0.009;

/// Zoom is exponential in the scroll delta so that a notch changes the view by the same
/// proportion however close the camera already is.
const ZOOM_PER_SCROLL_POINT: f32 = 0.0015;

/// How opaque what a click would put down is drawn: see-through enough to read as not
/// placed yet, solid enough that its shape still reads.
const GHOST_OPACITY: f32 = 0.6;

/// Draws the plate and everything on it, and returns the rectangle it took, which is what
/// the cards floating over it are anchored to.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) -> egui::Rect {
    // Hover only: the camera reads the raw pointer instead of this response, because the
    // gizmo takes the pointer away from it. See `viewport_input.rs`.
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());

    // The sheet and Find are modal: a click meant for them must not orbit the plate or drill a hole
    // in the model. The pointer is read from the raw input, which no modal layer covers.
    let pointer = if window.view.options.modal() {
        Pointer::idle()
    } else {
        pointer_state(ui, rect)
    };
    // The gizmo's report is one frame old, which matters only on the frame a handle is crossed.
    let shift_clicks = reads_shift_click(*window.tool, window.tools);
    let gesture = window
        .view
        .input
        .update(&pointer, window.view.gizmo.is_focused(), shift_clicks);

    // The brush and the dragged tip take the primary button only for a gesture begun on them,
    // so a drag from empty plate still turns the camera.
    let painting = paint_stroke(ui, window, rect, &pointer);
    // A support in hand has the whole gesture, shift and all: it neither turns the camera
    // nor slides it.
    let carrying = !painting && drag_support(window, rect, &pointer);
    show_carrying(ui.ctx(), window);
    let drag = if carrying { Drag::None } else { gesture.drag };

    fly_to_marked(ui, window, rect);
    follow_turn(ui, window.view);
    let invert = window.machine.general.invert_zoom;
    let anchor = pointer
        .position
        .filter(|_| pointer.zooms())
        .and_then(|at| zoom_anchor(window, rect, egui::Pos2::new(at.x, at.y)));
    steer_camera(window.view, rect, &pointer, drag, painting, invert, anchor);
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
    let ghost = ghost(ui, window, rect, &pointer);
    paint_plate(ui, window, rect, ghost.as_ref());
    draw_tool_overlays(ui, window, rect, &pointer);
    rect
}

/// Moves the camera along the swing the view cube started, until it is over.
fn follow_turn(ui: &egui::Ui, view: &mut View) {
    let Some(turn) = view.turn else {
        return;
    };
    let (camera, over) = turn.at(ui.input(|input| input.time));
    view.camera = camera;
    match over {
        true => view.turn = None,
        false => ui.ctx().request_repaint(),
    }
}

/// Orbits, pans and zooms the camera, towards `anchor` when there is one and the wheel the
/// other way round when `invert`. Taking hold of it ends any swing still under way.
fn steer_camera(
    view: &mut View,
    rect: egui::Rect,
    pointer: &Pointer,
    drag: Drag,
    painting: bool,
    invert: bool,
    anchor: Option<Vec3>,
) {
    let scrolled = pointer.zooms();
    if scrolled || (drag != Drag::None && !painting) {
        view.turn = None;
    }
    match drag {
        Drag::Orbit | Drag::Pan if painting => {}
        Drag::Orbit => view.camera.orbit(
            -pointer.delta.x * ORBIT_RAD_PER_POINT,
            pointer.delta.y * ORBIT_RAD_PER_POINT,
        ),
        Drag::Pan => view.camera.pan(pointer.delta, rect.height()),
        Drag::None => {}
    }

    if scrolled {
        let scroll = if invert {
            pointer.scroll
        } else {
            -pointer.scroll
        };
        let factor = (scroll * ZOOM_PER_SCROLL_POINT).exp();
        match anchor {
            Some(anchor) => view.camera.zoom_at(factor, anchor),
            None => view.camera.zoom(factor),
        }
    }
}

/// What a click on the plate does, which is the tool's to say.
fn click(ui: &egui::Ui, window: &mut Window, rect: egui::Rect, cursor: egui::Pos2) {
    if picks_instead(ui) {
        select_under_cursor(window, rect, cursor, true);
        return;
    }
    let picking = *window.tool == Tool::Position;
    if !picking && takes_the_pick(window, rect, cursor) {
        return;
    }
    match *window.tool {
        // A brush stroke has already painted whatever the click landed on.
        _ if paint::paints_shown(window) => {}
        // Alt is the opposite of a click in modelling tools; other buttons belong to the camera.
        Tool::Paint if window.tools.supports.placing.paints() => {}
        Tool::Relief if window.tools.relief.paints() => {}
        Tool::Paint if ui.input(|input| input.modifiers.alt) => {
            remove_support_under_cursor(window, rect, cursor);
        }
        Tool::Paint => {
            let line = ui.input(|input| input.modifiers.shift);
            place_support_under_cursor(window, rect, cursor, line);
        }
        // The drag has already taken hold of whatever part the press landed on.
        Tool::Supports => {}
        Tool::Hollow if ui.input(|input| input.modifiers.alt) => {
            remove_blocker_under_cursor(window, rect, cursor);
        }
        Tool::Hollow => {
            let rod = ui.input(|input| input.modifiers.shift);
            place_blocker_under_cursor(window, rect, cursor, rod);
        }
        Tool::Drain if ui.input(|input| input.modifiers.alt) => {
            remove_drain_under_cursor(window, rect, cursor);
        }
        Tool::Drain if window.tools.drain.placing == Placing::Channel => {
            add_channel_point_under_cursor(window, rect, cursor);
        }
        Tool::Drain => place_drain_under_cursor(window, rect, cursor),
        Tool::Relief if window.tools.relief.mode == ReliefMode::Text => {
            text::stamp_text_under_cursor(window, rect, cursor);
        }
        Tool::Measure => {
            let alt = ui.input(|input| input.modifiers.alt);
            measure_under_cursor(window, rect, cursor, alt);
        }
        // A brush stroke has already painted whatever the click landed on.
        Tool::Position if window.tools.orient.picking_face => {
            lay_face_under_cursor(window, rect, cursor);
        }
        Tool::Cut if window.tools.cut.picking_part => mark_part_under_cursor(window, rect, cursor),
        Tool::Repair if paint::picks_edges(window) => {
            repair::pick_edge(ui.ctx(), window, rect, cursor)
        }
        // The brush has already picked whatever faces the click landed on.
        Tool::Repair if paint::picks_faces(window) => {}
        _ => select_under_cursor(window, rect, cursor, false),
    }
}

/// Whether `tool` reads a shift press that stays put as a click — a line of supports, a rod
/// of blockers; elsewhere shift pans.
fn reads_shift_click(tool: Tool, tools: &Tools) -> bool {
    match tool {
        Tool::Paint => !tools.supports.placing.paints(),
        Tool::Hollow => true,
        _ => false,
    }
}

/// Whether this click adds the model under it to the selection or takes it out, in every
/// tool: Cmd held, or Ctrl off a Mac.
fn picks_instead(ui: &egui::Ui) -> bool {
    ui.input(|input| input.modifiers.command)
}

/// The chrome each tool draws over the 3D pass: bounds, gizmo, measurement, brush.
fn draw_tool_overlays(ui: &egui::Ui, window: &mut Window, rect: egui::Rect, pointer: &Pointer) {
    if *window.tool == Tool::Position {
        draw_bounds(ui, window, rect);
    }
    if *window.tool == Tool::Position && window.tools.orient.picking_face {
        let hovered = pointer.over_viewport.then_some(pointer.position).flatten();
        point_at_face(window, rect, hovered.map(|at| egui::pos2(at.x, at.y)));
        draw_facet(ui, window, rect);
    }
    if shows_gizmo(window) {
        show_gizmo(ui, window, rect);
    }
    if *window.tool == Tool::Measure {
        draw_measure(ui, window, rect);
    }
    if *window.tool == Tool::Cut {
        draw_cut_plane(ui, window, rect);
        draw_part_mark(ui, window, rect);
    }
    if (*window.tool == Tool::Paint && window.tools.supports.placing.paints())
        || (*window.tool == Tool::Relief && window.tools.relief.paints())
        || paint::picks_faces(window)
        || paint::paints_shown(window)
    {
        draw_brush(ui, window, rect);
    }
    if *window.tool == Tool::Supports {
        draw_picked(ui, window, rect);
        // A press paints what shows while that brush is out: it takes hold of nothing.
        if !paint::paints_shown(window) {
            draw_aimed(ui, window, rect, pointer);
        }
        draw_refusal(ui, window, rect);
    }
}

/// Draws the view with or without perspective, ending any swing that would put it back.
pub fn set_orthographic(view: &mut View, orthographic: bool) {
    view.turn = None;
    view.camera.orthographic = orthographic;
}

/// Points the camera at everything on the plate, or at the plate itself when it is empty.
pub fn frame_view(scene: &Scene, plate: &BuildPlate, camera: &mut OrbitCamera) {
    match scene.world_bounds() {
        Some(bounds) => camera.frame(&bounds),
        None => {
            *camera = OrbitCamera {
                orthographic: camera.orthographic,
                ..OrbitCamera::framing_plate(plate)
            }
        }
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
            alt: input.modifiers.alt,
            command: input.modifiers.command,
            delta: Vec2::new(delta.x, delta.y),
            scroll: input.smooth_scroll_delta.y,
        }
    })
}

/// Whether the pointer is on the model rather than on a card, popup, menu or window over it,
/// whose egui layer owns the pointer. See `docs/decisions/0193`.
fn over_the_model(
    rect: egui::Rect,
    position: Option<egui::Pos2>,
    over: Option<egui::LayerId>,
    own: egui::LayerId,
) -> bool {
    position
        .is_some_and(|position| rect.contains(position) && over.is_none_or(|layer| layer == own))
}

/// Whether the picked models carry the handles: in Position, and while Relief combines
/// them. While a face is being picked the click is for the model, not for a handle over it.
pub(crate) fn moves_by_hand(window: &Window) -> bool {
    match *window.tool {
        Tool::Position => handles_out(&window.tools.orient),
        Tool::Relief => window.tools.relief.mode == ReliefMode::Combine,
        _ => false,
    }
}

/// Whether Position has its handles out: not while a click lays a face or the brush
/// paints what shows, which would take the press meant for the model.
fn handles_out(orient: &crate::orient::OrientTool) -> bool {
    !orient.picking_face && !orient.paints_shown(Tool::Position, crate::supports::Placing::Point)
}

/// Whether the handles are out this frame: the tool moves by hand and no modal covers them.
fn shows_gizmo(window: &Window) -> bool {
    !window.view.options.modal() && moves_by_hand(window)
}

/// Where the handles stand on each picked model, in the order of the selection.
fn picked_pivots(window: &Window) -> Vec<core_geometry::Transform> {
    let scene = &window.doc.scene;
    let picked = scene.selection().iter().filter_map(|id| scene.get(*id));
    picked.map(|object| object.pivot()).collect()
}

/// Draws the handles over the selected objects and writes back what the user dragged.
fn show_gizmo(ui: &egui::Ui, window: &mut Window, viewport: egui::Rect) {
    let picked: Vec<ObjectId> = window.doc.scene.selection().to_vec();
    let pivots = picked_pivots(window);
    if pivots.is_empty() {
        return;
    }

    // The crate listens wherever the pointer is, within the clip of the `Ui` it is handed;
    // the stage's would take every press on the layer strip as well.
    let mut over_viewport = egui::Ui::new(
        ui.ctx().clone(),
        ui.id().with("gizmo"),
        egui::UiBuilder::new()
            .layer_id(ui.layer_id())
            .max_rect(viewport),
    );
    over_viewport.set_clip_rect(viewport.intersect(ui.clip_rect()));
    let moved = window
        .view
        .gizmo
        .show(&over_viewport, viewport, &window.view.camera, &pivots);
    draw_gizmo_drag(&over_viewport, window, viewport);
    let Some(moved) = moved else {
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
    fn a_shift_click_is_a_click_where_it_lays_a_line_or_a_rod() {
        let tools = Tools::default();
        assert!(reads_shift_click(Tool::Paint, &tools), "a line of supports");
        assert!(reads_shift_click(Tool::Hollow, &tools), "a rod of blockers");
        assert!(!reads_shift_click(Tool::Position, &tools), "shift pans");
    }

    #[test]
    fn the_handles_are_put_away_while_the_brush_paints_what_shows() {
        let mut orient = crate::orient::OrientTool::default();
        assert!(handles_out(&orient));
        orient.settings.clean_surface = true;
        orient.shown_brush.brushing = true;
        assert!(
            !handles_out(&orient),
            "a press on the handles paints instead"
        );
    }

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
