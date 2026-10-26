use crate::relief::ReliefMode;
use core_geometry::{PlacedHit, Vec2};
use egui::{PointerButton, Sense};

use crate::camera::{CameraTurn, OrbitCamera};
use crate::drain::Placing;
use crate::panels::{Window, section};
use crate::plate::BuildPlate;
use crate::render::{Banding, Ghost, Shading, ViewportCallback};
use crate::scene::{ObjectId, Scene};
use crate::state::View;
use crate::ui::theme;
use crate::viewport_input::{Drag, Pointer};
use crate::workspace::Tool;

mod hollow;
mod levels;
mod overlays;
mod paint;
mod picking;
mod support_ghost;
mod supports;
mod text;

use hollow::{
    add_channel_point_under_cursor, blocker_ghost, channel_point_ghost, drain_ghost,
    place_blocker_under_cursor, place_drain_under_cursor, remove_blocker_under_cursor,
    remove_drain_under_cursor,
};
use overlays::{
    cut_line, draw_aimed, draw_axis_letters, draw_bounds, draw_cut_plane, draw_facet, draw_measure,
    draw_part_mark, draw_picked,
};
use paint::{draw_brush, paint_stroke};
use picking::{
    aims_elsewhere, lay_face_under_cursor, mark_part_under_cursor, material_under,
    measure_under_cursor, point_at_face, printed_under, select_under_cursor, surface_under,
    takes_the_pick,
};
use support_ghost::cached_support_ghost;
use supports::{drag_support, place_support_under_cursor, remove_support_under_cursor};

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
    // Only Place reads a shift-click, as a line of supports; elsewhere shift pans.
    let shift_clicks = *window.tool == Tool::Paint && !window.tools.supports.placing.paints();
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
    let drag = if carrying { Drag::None } else { gesture.drag };

    fly_to_found(ui, window, rect);
    follow_turn(ui, window.view);
    let invert = window.machine.general.invert_zoom;
    steer_camera(window.view, rect, &pointer, drag, painting, invert);
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

/// Orbits, pans and zooms the camera, the wheel the other way round when `invert`. Taking
/// hold of it ends any swing still under way.
fn steer_camera(
    view: &mut View,
    rect: egui::Rect,
    pointer: &Pointer,
    drag: Drag,
    painting: bool,
    invert: bool,
) {
    let scrolled = pointer.over_viewport && pointer.scroll != 0.0;
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
        view.camera.zoom((scroll * ZOOM_PER_SCROLL_POINT).exp());
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
        // Alt is the opposite of a click in modelling tools; other buttons belong to the camera.
        // A brush stroke has already painted whatever the click landed on.
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
        Tool::Hollow => place_blocker_under_cursor(window, rect, cursor),
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
        Tool::Position if window.tools.orient.picking_face => {
            lay_face_under_cursor(window, rect, cursor);
        }
        Tool::Cut if window.tools.cut.picking_part => mark_part_under_cursor(window, rect, cursor),
        _ => select_under_cursor(window, rect, cursor, false),
    }
}

/// Whether this click adds the model under it to the selection or takes it out, in every
/// tool: Cmd held, or Ctrl off a Mac.
fn picks_instead(ui: &egui::Ui) -> bool {
    ui.input(|input| input.modifiers.command)
}

/// What a click would put down under a resting cursor, drawn see-through as not placed yet.
/// Alt shows nothing: it takes away instead.
fn ghost(ui: &egui::Ui, window: &Window, rect: egui::Rect, pointer: &Pointer) -> Option<Ghost> {
    let removing = ui.input(|input| input.modifiers.alt) || picks_instead(ui);
    let line = ui.input(|input| input.modifiers.shift);
    let resting = !pointer.primary_down && !pointer.pan_button_down;
    if removing || !resting || !pointer.over_viewport {
        return None;
    }
    let cursor = pointer.position.map(|at| egui::pos2(at.x, at.y))?;
    if *window.tool == Tool::Paint && !window.tools.supports.placing.paints() {
        let (id, hit) = surface_under(window, rect, cursor)?;
        return faded(&window.doc.scene, id, || {
            cached_support_ghost(ui.ctx(), window, id, &hit, line)
        });
    }
    if *window.tool == Tool::Drain && window.tools.drain.placing != Placing::Channel {
        let (id, printed, hit) = printed_under(window, rect, cursor)?;
        return faded(&window.doc.scene, id, || {
            drain_ghost(window, id, printed, &hit)
        });
    }
    type Under = fn(&Window, egui::Rect, egui::Pos2) -> Option<(ObjectId, PlacedHit)>;
    type Shows = fn(&Window, ObjectId, &PlacedHit) -> Option<Ghost>;
    // A blocker is a volume, so it can be put inside the material through the cap of a cut.
    let (under, shows): (Under, Shows) = match *window.tool {
        Tool::Hollow => (material_under, blocker_ghost),
        Tool::Drain if window.tools.drain.placing == Placing::Channel => {
            (surface_under, channel_point_ghost)
        }
        Tool::Relief if window.tools.relief.mode == ReliefMode::Text => {
            (surface_under, text::text_ghost)
        }
        _ => return None,
    };
    let (id, hit) = under(window, rect, cursor)?;
    faded(&window.doc.scene, id, || shows(window, id, &hit))
}

/// The ghost `shows` builds on `id`, see-through, or nothing on a model outside the
/// selection: a click there only picks it.
fn faded(scene: &Scene, id: ObjectId, shows: impl FnOnce() -> Option<Ghost>) -> Option<Ghost> {
    if aims_elsewhere(scene, Some(id)) {
        return None;
    }
    let mut ghost = shows()?;
    ghost.colour = ghost.colour.gamma_multiply(GHOST_OPACITY);
    Some(ghost)
}

/// Hands the 3D pass its frame: the models, the plate, the section and the shading.
fn paint_plate(ui: &egui::Ui, window: &mut Window, rect: egui::Rect, ghost: Option<&Ghost>) {
    let textures = (*window.tool == Tool::Relief
        && window.tools.relief.mode == ReliefMode::Texture)
        .then(|| window.tools.relief.textures(&window.doc.scene));
    // What needs holding up is shown while the Supports tool is the one in hand, at the
    // angle its own profile is set to; see `docs/decisions/0034`.
    let overhang_deg = window.tool.shows_overhangs().then_some(
        window
            .tools
            .supports
            .shown(&window.doc.scene)
            .max_overhang_deg,
    );
    crate::render::prime_label(ui.painter());
    ui.painter().rect_filled(rect, 0.0, theme::colors().sunken);
    let hidden_groups = window.tools.supports.hidden();
    let section_mm =
        section::cut_height(*window.mode, &window.view.section, &window.machine.preview);
    let bottom_cut_mm =
        section::floor_height(*window.mode, &window.view.section, &window.machine.preview);
    let capped = section_mm.is_some() || bottom_cut_mm.is_some();
    let height_px = rect.height() * ui.ctx().pixels_per_point();
    let levels = levels::levels(ui, window, capped, height_px);
    let callback = ViewportCallback::new(
        &window.doc.scene,
        &window.doc.plate,
        &window.view.camera,
        window.view.options,
        Shading {
            overhang_deg,
            section_mm,
            bottom_cut_mm,
            banding: Banding {
                ranges: &window.machine.slicing.exposure,
                floor_mm: window.machine.slicing.band_floor_mm(),
            },
            textures: textures.as_ref(),
            cut_line: cut_line(window),
            hidden_groups: &hidden_groups,
            ghost,
            light: Some(&levels.light),
        },
        rect,
    )
    .highlighting(&found_highlight(window))
    .timed(window.view.ui_ms, window.view.budget.report(), levels.faces);
    ui.painter()
        .add(egui_wgpu::Callback::new_paint_callback(rect, callback));
    draw_axis_letters(ui, window, rect);
}

/// What the Repair tool found, marked on the models only while it is the tool in hand.
fn found_highlight(window: &Window) -> crate::highlight::Highlight {
    if *window.tool != Tool::Repair {
        return crate::highlight::Highlight::default();
    }
    window.doc.repairs.found.highlight(&window.doc.scene)
}

/// Flies the camera to the place of a finding the Repair tool's last press went to, on
/// the frame that place is worked out.
fn fly_to_found(ui: &egui::Ui, window: &mut Window, rect: egui::Rect) {
    if *window.tool != Tool::Repair {
        return;
    }
    let scene = &window.doc.scene;
    let Some(spot) = window.doc.repairs.found.take_flight(scene) else {
        return;
    };
    let aspect = rect.width() / rect.height().max(1.0);
    let hidden = |eye, point| crate::pick::occluded(scene, eye, point);
    let to = window.view.camera.focusing(&spot, aspect, hidden);
    let now = ui.input(|input| input.time);
    window.view.turn = Some(CameraTurn::flight(window.view.camera, to, now));
    ui.ctx().request_repaint();
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
    if !window.view.options.modal() && moves_by_hand(window) {
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
    {
        draw_brush(ui, window, rect);
    }
    if *window.tool == Tool::Supports {
        draw_picked(ui, window, rect);
        draw_aimed(ui, window, rect, pointer);
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
fn moves_by_hand(window: &Window) -> bool {
    match *window.tool {
        Tool::Position => !window.tools.orient.picking_face,
        Tool::Relief => window.tools.relief.mode == ReliefMode::Combine,
        _ => false,
    }
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
    let Some(moved) =
        window
            .view
            .gizmo
            .show(&over_viewport, viewport, &window.view.camera, &pivots)
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
