use core_geometry::{Adjacency, Mat4, Vec2, Vec3};
use core_supports::{Grab, Part, Placed, Profiles, SupportTree, grab, support_under};
use egui::{PointerButton, Sense};

use crate::camera::OrbitCamera;
use crate::drain::Placing;
use crate::measure;
use crate::panels::{Overlays, Window, section};
use crate::pick::{occluded, pick, pick_surface, ray_through};
use crate::plate::BuildPlate;
use crate::render::{Banding, Shading, ViewportCallback};
use crate::scene::Scene;
use crate::state::{Doc, Tools, View};
use crate::supports::Picked;
use crate::ui::theme;
use crate::viewport_input::{Drag, Pointer};
use crate::workspace::Tool;

/// Radians of orbit per point of cursor movement. A drag across a 600 point wide panel is
/// a little under a full turn.
const ORBIT_RAD_PER_POINT: f32 = 0.009;

/// How wide the dot at each end of a measurement is drawn, in points.
const MEASURE_END_R: f32 = 4.0;

/// How far along each edge a bounds bracket reaches, as a fraction of that edge.
const BRACKET_ARM: f32 = 0.15;
/// A face bigger than this is washed only in part; the lay-down still takes all of it.
const MAX_FACET_TRIANGLES: usize = 200_000;
/// How many pieces each arm is tested for being hidden in.
const BRACKET_PIECES: u32 = 6;

/// How many segments the brush ring is drawn with.
const BRUSH_SEGMENTS: u32 = 48;

/// How far apart, in panel points, the brush is put down along a drag.
const DAUB_STEP_POINTS: f32 = 4.0;

/// A ray flatter than this against a plane has no crossing worth using: a part dragged
/// edge-on to it would shoot off to the horizon.
const PLATE_GRAZE: f32 = 1e-3;

/// What the Edit mode is holding is drawn in green: the one colour the palette does not
/// already spend on a model, a support, a selection or a warning.
const PICKED_WIDTH: f32 = 3.0;
const PICKED_DOT_R: f32 = 5.0;

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
        pointer_state(ui, rect, &window.view.overlays)
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
    if *window.tool == Tool::Supports && window.tools.supports.placing.paints() {
        draw_brush(ui, window, rect);
    }
    if *window.tool == Tool::Supports && window.tools.supports.placing.edits() {
        draw_picked(ui, window, rect);
    }
}

/// Draws the brush as a ring lying on the surface under the cursor, in the colour of the
/// patch it paints. Chrome over the 3D pass, like a measurement, so nothing hides it.
fn draw_brush(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let Some(cursor) = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|position| viewport.contains(*position))
    else {
        return;
    };
    let Some((_, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };

    let on_screen = projector(&window.view.camera, viewport);
    let (across, along) = hit.normal.any_orthonormal_pair();
    let radius_mm = window.tools.supports.brush_radius_mm;
    let ring: Vec<egui::Pos2> = (0..BRUSH_SEGMENTS)
        .filter_map(|step| {
            let angle = std::f32::consts::TAU * step as f32 / BRUSH_SEGMENTS as f32;
            on_screen(hit.point + (across * angle.cos() + along * angle.sin()) * radius_mm)
        })
        .collect();
    if ring.len() < BRUSH_SEGMENTS as usize {
        return;
    }

    let colors = theme::colors();
    let colour = if window.tools.supports.placing.blocks() {
        colors.danger
    } else {
        colors.accent
    };
    let painter = ui.painter_at(viewport);
    painter.add(egui::Shape::closed_line(
        ring,
        egui::Stroke::new(1.5, colour),
    ));
}

/// Takes the point the click landed on, snapped to the nearest corner of the model it
/// landed on.
fn measure_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    let Some(object) = window.doc.scene.get(id) else {
        return;
    };
    let point = measure::snap(&object.mesh, &object.bvh, object.transform, hit.point);
    window.tools.measure.pick(point);
}

/// Where a point of the plate lands in the panel, or `None` when it is behind the camera.
fn projector(camera: &OrbitCamera, viewport: egui::Rect) -> impl Fn(Vec3) -> Option<egui::Pos2> {
    let matrix = camera.view_projection(viewport.width() / viewport.height());
    move |point: Vec3| {
        let clip = matrix * point.extend(1.0);
        (clip.w > 0.0).then(|| {
            let ndc = clip.truncate() / clip.w;
            egui::Pos2::new(
                viewport.left() + (ndc.x * 0.5 + 0.5) * viewport.width(),
                viewport.top() + (0.5 - ndc.y * 0.5) * viewport.height(),
            )
        })
    }
}

/// Draws the span over the models rather than into them: a measurement is chrome, and
/// putting it in the 3D pass would hide it inside the very feature being measured.
fn draw_measure(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let on_screen = projector(&window.view.camera, viewport);

    let painter = ui.painter_at(viewport);
    let accent = theme::colors().accent;
    for end in [
        window.tools.measure.start(),
        window.tools.measure.span().map(|(_, to)| to),
    ]
    .into_iter()
    .flatten()
    .filter_map(&on_screen)
    {
        painter.circle_filled(end, MEASURE_END_R, accent);
    }

    let Some((from, to)) = window.tools.measure.span() else {
        return;
    };
    let (Some(from), Some(to)) = (on_screen(from), on_screen(to)) else {
        return;
    };
    painter.line_segment([from, to], egui::Stroke::new(2.0, accent));

    let Some(distance_mm) = window.tools.measure.distance_mm() else {
        return;
    };
    let label = format!("{distance_mm:.2} mm");
    let middle = from.lerp(to, 0.5);
    let galley = painter.layout_no_wrap(label, theme::body(), theme::colors().text_high);
    let box_rect = egui::Rect::from_center_size(middle, galley.size() + egui::vec2(10.0, 6.0));
    painter.rect_filled(box_rect, theme::R_CONTROL, theme::colors().panel);
    painter.galley(box_rect.center() - galley.size() / 2.0, galley, accent);
}

/// Brackets on the corners of each picked model's bounds, and the size of each side, the
/// way a slicer shows what a model measures without hiding it inside a box.
fn draw_bounds(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let on_screen = projector(&window.view.camera, viewport);
    let eye = window.view.camera.eye();
    let seen = |point: Vec3| on_screen(point).filter(|_| !occluded(&window.doc.scene, eye, point));
    let painter = ui.painter_at(viewport);
    let stroke = egui::Stroke::new(1.5, theme::scene().bounds);
    let picked = window.doc.scene.selection().iter();
    let boxes = picked.filter_map(|id| window.doc.scene.get(*id)?.world_bounds());
    for bounds in boxes {
        let (lo, hi) = (bounds.mins, bounds.maxs);
        let arm = (hi - lo) * BRACKET_ARM;
        for corner in 0..8 {
            let pick = |bit: usize, axis: usize| {
                if corner >> bit & 1 == 0 {
                    lo[axis]
                } else {
                    hi[axis]
                }
            };
            let at = Vec3::new(pick(0, 0), pick(1, 1), pick(2, 2));
            let inward = (lo + hi) * 0.5 - at;
            for axis in 0..3 {
                let mut end = at;
                end[axis] += arm[axis].copysign(inward[axis]);
                // Cut into pieces, so an arm half behind the model shows its other half.
                for piece in 0..BRACKET_PIECES {
                    let along = |t: f32| at.lerp(end, t / BRACKET_PIECES as f32);
                    let (from, to) = (along(piece as f32), along(piece as f32 + 1.0));
                    let middle = seen(from.lerp(to, 0.5));
                    if let (Some(_), Some(from), Some(to)) =
                        (middle, on_screen(from), on_screen(to))
                    {
                        painter.line_segment([from, to], stroke);
                    }
                }
            }
        }
        size_labels(&painter, &seen, lo, hi);
    }
}

/// The length of each side, written beside the bottom front edge for X and Y and up the
/// front left edge for Z, in the axis colour its field uses.
fn size_labels(
    painter: &egui::Painter,
    on_screen: &impl Fn(Vec3) -> Option<egui::Pos2>,
    lo: Vec3,
    hi: Vec3,
) {
    let size = hi - lo;
    let middles = [
        Vec3::new((lo.x + hi.x) * 0.5, lo.y, lo.z),
        Vec3::new(hi.x, (lo.y + hi.y) * 0.5, lo.z),
        Vec3::new(lo.x, lo.y, (lo.z + hi.z) * 0.5),
    ];
    for (axis, middle) in middles.into_iter().enumerate() {
        let Some(at) = on_screen(middle) else {
            continue;
        };
        let text = format!("{:.2}", size[axis]);
        let galley = painter.layout_no_wrap(text, theme::mono(11.0), theme::colors().axis[axis]);
        let plate = egui::Rect::from_center_size(at, galley.size() + egui::vec2(8.0, 4.0));
        painter.rect_filled(plate, theme::R_CONTROL, theme::colors().panel);
        painter.galley(
            plate.center() - galley.size() / 2.0,
            galley,
            theme::colors().axis[axis],
        );
    }
}

/// Points the camera at everything on the plate, or at the plate itself when it is empty.
pub fn frame_view(scene: &Scene, plate: &BuildPlate, camera: &mut OrbitCamera) {
    match scene.world_bounds() {
        Some(bounds) => camera.frame(&bounds),
        None => *camera = OrbitCamera::framing_plate(plate),
    }
}

/// Reads this frame's pointer, in panel points, and whether it is over the 3D view. A
/// pointer over a floating card is over the card, not over the model behind it.
fn pointer_state(ui: &egui::Ui, rect: egui::Rect, overlays: &Overlays) -> Pointer {
    ui.input(|input| {
        let position = input.pointer.hover_pos();
        let delta = input.pointer.delta();
        Pointer {
            over_viewport: position
                .is_some_and(|position| rect.contains(position) && !overlays.covers(position)),
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

/// Draws the handles over the selected objects and writes back what the user dragged.
fn show_gizmo(ui: &egui::Ui, window: &mut Window, viewport: egui::Rect) {
    let picked: Vec<crate::scene::ObjectId> = window.doc.scene.selection().to_vec();
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

/// Lays the face under the cursor on the plate, and picks the model it belongs to. A
/// click on empty plate keeps the tool waiting for a face.
fn lay_face_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    let Some(object) = window.doc.scene.get_mut(id) else {
        return;
    };
    window.tools.orient.point_at(object, hit.face);
    let normal = window
        .tools
        .orient
        .facet
        .as_ref()
        .map_or(hit.normal, |facet| object.world_normal(facet.normal));
    if object.lay_face_down(normal) {
        window.doc.scene.select(Some(id));
        window.tools.orient.stop_picking();
    }
}

/// Keeps the flat face under the cursor as the one a click would lay down.
fn point_at_face(window: &mut Window, viewport: egui::Rect, cursor: Option<egui::Pos2>) {
    let hit = cursor
        .and_then(|cursor| pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor));
    match hit.and_then(|(id, hit)| Some((window.doc.scene.get(id)?, hit.face))) {
        Some((object, face)) => window.tools.orient.point_at(object, face),
        None => window.tools.orient.facet = None,
    }
}

/// Washes the face a click would lay down, over the model, so the user sees which one.
fn draw_facet(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let Some(facet) = &window.tools.orient.facet else {
        return;
    };
    let Some(object) = window.doc.scene.get(facet.id) else {
        return;
    };
    let on_screen = projector(&window.view.camera, viewport);
    let matrix = object.transform.to_matrix();
    let color = theme::scene().facet;
    let mut mesh = egui::Mesh::default();
    for face in facet.faces.iter().take(MAX_FACET_TRIANGLES) {
        let Some(corners) = object.mesh.faces.get(*face) else {
            continue;
        };
        let points = corners.map(|corner| {
            on_screen(matrix.transform_point3(object.mesh.vertices[corner as usize]))
        });
        let [Some(a), Some(b), Some(c)] = points else {
            continue;
        };
        let at = mesh.vertices.len() as u32;
        for point in [a, b, c] {
            mesh.colored_vertex(point, color);
        }
        mesh.add_triangle(at, at + 1, at + 2);
    }
    ui.painter_at(viewport).add(mesh);
}

/// A click that was not a drag picks whatever is under it, or clears the selection.
/// Held with a modifier it adds to what is picked, or takes that one back out.
fn select_under_cursor(
    scene: &mut Scene,
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    adding: bool,
) {
    let hit = pick(scene, camera, viewport, cursor);
    match (adding, hit) {
        (true, Some(id)) => scene.toggle_selected(id),
        (true, None) => {}
        (false, hit) => scene.select(hit),
    }
}

/// Stands a support where the model was clicked, and selects what it was put on so that
/// the inspector is talking about the same object.
fn place_support_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
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

/// Runs the brush for this frame and answers whether it has the pointer.
///
/// The stroke is decided on the frame the button goes down: a press that misses every
/// model leaves the drag to the camera, and one that lands on a model paints until the
/// button comes up.
fn paint_stroke(
    ui: &egui::Ui,
    window: &mut Window,
    viewport: egui::Rect,
    pointer: &Pointer,
) -> bool {
    let out = *window.tool == Tool::Supports && window.tools.supports.placing.paints();
    let Some(position) = pointer.position.filter(|_| {
        out && pointer.primary_down
            && pointer.over_viewport
            && !pointer.shift
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
        window.tools.supports.stroke =
            pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor).is_some();
        window.tools.supports.last_daub = None;
    }
    if !window.tools.supports.stroke {
        return false;
    }

    let modifiers = ui.input(|input| input.modifiers);
    // A drag moves further in one frame than the brush is wide, so the stroke is painted
    // along the line the cursor drew rather than only where the frames fell.
    let from = window.tools.supports.last_daub.unwrap_or(position);
    let steps = (from.distance(position) / DAUB_STEP_POINTS).ceil().max(1.0) as u32;
    for step in 1..=steps {
        let along = from.lerp(position, step as f32 / steps as f32);
        paint_under_cursor(
            window,
            viewport,
            egui::Pos2::new(along.x, along.y),
            modifiers.command,
            !modifiers.alt,
        );
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

/// Picks and carries the parts of a support the Edit mode is working on, and answers
/// whether it has the pointer.
///
/// The press picks: one that lands on a part of a support takes hold of it — with shift,
/// alongside what was already held — and drags it until the button comes up; one that
/// misses lets go and leaves the drag to the camera.
fn drag_support(window: &mut Window, viewport: egui::Rect, pointer: &Pointer) -> bool {
    let out = *window.tool == Tool::Supports && window.tools.supports.placing.edits();
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
    let normal = (camera.eye() - anchor).normalize_or_zero();
    let facing = ray.direction.dot(normal);
    if facing.abs() < PLATE_GRAZE {
        return None;
    }
    Some(ray.at((anchor - ray.origin).dot(normal) / facing))
}

/// Moves every held part by `step`, in plate millimetres, and answers whether anything
/// moved.
///
/// Each support is carried as a whole and then tested by the rule the automatic run
/// places by: a tip is pulled back onto the surface it holds, and a step that would sink
/// any body of the support into the model is dropped, so a support cannot be edited into
/// the part it is meant to hold up. See `docs/decisions/0095`.
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
        let matrix = object.transform.to_matrix();
        if matrix.determinant().abs() < f32::EPSILON {
            continue;
        }
        let inverse = matrix.inverse();

        let Some(standing) = object.supports.frozen().get(frozen) else {
            continue;
        };
        let mut candidate = standing.clone();
        for part in parts {
            carry_part(&mut candidate, part, matrix, inverse, step);
        }

        let keep_out = object.supports.keep_out(&object.mesh, object.transform);
        let placed =
            Placed::new(&object.mesh, &object.bvh, object.transform).blocking(keep_out.as_ref());
        let profile = profiles.of(candidate.group());
        snap_tips(&mut candidate, &placed, matrix, inverse);
        if !core_supports::fits(&candidate.moved(matrix), &placed, profile) {
            continue;
        }

        if let Some(tree) = object.supports.frozen_mut(frozen) {
            *tree = candidate;
            moved = true;
        }
    }
    moved
}

/// Every frozen tree something is held on, each once.
fn held_trees(tools: &Tools) -> Vec<(crate::scene::ObjectId, usize)> {
    let mut held: Vec<(crate::scene::ObjectId, usize)> = tools
        .supports
        .picked
        .iter()
        .map(|picked| (picked.id, picked.frozen))
        .collect();
    held.sort_unstable();
    held.dedup();
    held
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
        if let Some(on) = core_supports::on_model(placed, matrix.transform_point3(position)) {
            tree.move_node(index, inverse.transform_point3(on));
        }
    }
}

/// Rebuilds the supports of every object something is held on/// Rebuilds the supports of every object something is held on, in this frame rather than
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

/// Paints the model under the cursor, with the brush or — while `whole_face` is held —
/// with the surface the face it hit belongs to.
///
/// `marked` is false for the alt-click that takes the paint off again.
fn paint_under_cursor(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    whole_face: bool,
    marked: bool,
) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    let blocking = window.tools.supports.placing.blocks();
    let radius_mm = window.tools.supports.brush_radius_mm;
    let angle_deg = window.tools.supports.flood_angle_deg;
    let Some(object) = window.doc.scene.get_mut(id) else {
        return;
    };

    if whole_face {
        let adjacency = Adjacency::of(&object.mesh);
        object.supports.paint_face(
            &object.mesh,
            &adjacency,
            hit.face,
            angle_deg,
            blocking,
            marked,
        );
    } else {
        let placed = Placed::new(&object.mesh, &object.bvh, object.transform);
        object
            .supports
            .paint(&placed, hit.point, radius_mm, blocking, marked);
    }
}

/// Keeps the wall solid where the model was clicked, and selects what it was put on so
/// that the inspector is talking about the same object.
fn place_blocker_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    window.doc.scene.select(Some(id));
    let radius_mm = window.tools.hollow.blocker_mm;
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.add_blocker(hit.point, radius_mm, transform);
    }
}

/// Takes away the blocker the click landed in.
fn remove_blocker_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.remove_blocker(hit.point, transform);
    }
}

/// Drills a drain hole where the model was clicked, straight into the surface under the
/// cursor, and selects what it was drilled into.
fn place_drain_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    window.doc.scene.select(Some(id));
    let size = window.tools.drain.size();
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.add_drain(
            &object.mesh,
            &object.bvh,
            hit.point,
            hit.normal,
            size,
            transform,
        );
    }
}

/// Adds a point to the channel being laid out on the model under the cursor.
fn add_channel_point_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    window.doc.scene.select(Some(id));
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object
            .hollow
            .add_channel_point(hit.point, hit.normal, transform);
    }
}

/// Takes away the drain hole the click landed in.
fn remove_drain_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
    let Some((id, hit)) = pick_surface(&window.doc.scene, &window.view.camera, viewport, cursor)
    else {
        return;
    };
    if let Some(object) = window.doc.scene.get_mut(id) {
        let transform = object.transform;
        object.hollow.remove_drain(hit.point, transform);
    }
}

/// Takes away the support under the cursor. Objects are searched in the order they were
/// imported, which is the order the viewport draws them in.
fn remove_support_under_cursor(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2) {
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

/// Which part of which support the cursor is over, with the object it belongs to.
/// Objects are searched in the order they were imported.
fn grab_under_cursor(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
) -> Option<(crate::scene::ObjectId, Grab)> {
    let ray = ray_through(&window.view.camera, viewport, cursor)?;
    let table = window.tools.supports.table();
    let profiles = Profiles::new(&table)?;

    window
        .doc
        .scene
        .here()
        .find_map(|object| grab(object.supports.trees(), &ray, profiles).map(|it| (object.id, it)))
}

/// Draws what the Edit mode is holding, over the models rather than into them: a stick as
/// a line and a joint as a dot, both in the picked colour.
fn draw_picked(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let on_screen = projector(&window.view.camera, viewport);
    let painter = ui.painter_at(viewport);

    for picked in &window.tools.supports.picked {
        let Some(object) = window.doc.scene.get(picked.id) else {
            continue;
        };
        let matrix = object.transform.to_matrix();
        let Some(tree) = object.supports.frozen().get(picked.frozen) else {
            continue;
        };
        let at = |point: Vec3| on_screen(matrix.transform_point3(point));

        let ends = match picked.part {
            Part::Node(node) => tree.nodes().get(node).map(|node| (node.position, None)),
            Part::Strut(child) => tree.nodes().get(child).and_then(|node| {
                let parent = node.parent?;
                Some((node.position, Some(tree.nodes()[parent].position)))
            }),
            Part::Trunk => Some((tree.root().position, Some(tree.landing().base))),
            Part::Foot => Some((tree.landing().base, None)),
        };
        let Some((from, to)) = ends else {
            continue;
        };

        match to.and_then(at).zip(at(from)) {
            Some((to, from)) => {
                painter.line_segment(
                    [from, to],
                    egui::Stroke::new(PICKED_WIDTH, theme::colors().picked),
                );
            }
            None => {
                if let Some(from) = at(from) {
                    painter.circle_filled(from, PICKED_DOT_R, theme::colors().picked);
                }
            }
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
}
