//! What is drawn over the plate: the measurement, the cut plane, a model's bounds, the
//! washed facet and what the Edit mode is holding.

use core_geometry::Vec3;
use core_supports::Part;

use crate::camera::OrbitCamera;
use crate::panels::Window;
use crate::pick::occluded;
use crate::render::{AXIS_LENGTH_MM, CutLine};
use crate::ui::theme;
use crate::workspace::Tool;

/// How wide the dot at each end of a measurement is drawn, in points.
const MEASURE_END_R: f32 = 4.0;

/// How far along each edge a bounds bracket reaches, as a fraction of that edge.
const BRACKET_ARM: f32 = 0.15;

/// How many pieces each arm is tested for being hidden in.
const BRACKET_PIECES: u32 = 6;

/// A face bigger than this is washed only in part; the lay-down still takes all of it.
const MAX_FACET_TRIANGLES: usize = 200_000;

/// How far the cut plane reaches past the model on each side: a share of its widest side,
/// and a floor so a small part still shows a plane around it, in millimetres.
const CUT_PLANE_MARGIN: f32 = 0.15;

const CUT_PLANE_MARGIN_MIN_MM: f32 = 3.0;

/// What the Edit mode is holding is drawn in green: the one colour the palette does not
/// already spend on a model, a support, a selection or a warning.
const PICKED_WIDTH: f32 = 3.0;

const PICKED_DOT_R: f32 = 5.0;

/// How far past the tip of each axis arrow its letter stands, millimetres.
const AXIS_LETTER_GAP_MM: f32 = 5.0;

/// X, Y and Z past the tips of the arrows in the plate's origin corner. Drawn over the 3D pass
/// rather than in it, so a letter always faces the camera.
pub(super) fn draw_axis_letters(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let project = projector(&window.view.camera, viewport);
    for ((axis, letter), colour) in [(Vec3::X, "X"), (Vec3::Y, "Y"), (Vec3::Z, "Z")]
        .into_iter()
        .zip(theme::scene().plate_axis)
    {
        let tip = axis * (AXIS_LENGTH_MM + AXIS_LETTER_GAP_MM);
        if let Some(at) = project(tip).filter(|at| viewport.contains(*at)) {
            ui.painter().text(
                at,
                egui::Align2::CENTER_CENTER,
                letter,
                theme::axis_letter(),
                colour,
            );
        }
    }
}

/// Where a point of the plate lands in the panel, or `None` when it is behind the camera.
pub(super) fn projector(
    camera: &OrbitCamera,
    viewport: egui::Rect,
) -> impl Fn(Vec3) -> Option<egui::Pos2> {
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

/// Where the Cut tool's plane crosses the selected model, for the 3D pass to trace on its
/// surface.
pub(super) fn cut_line(window: &Window) -> Option<CutLine> {
    if *window.tool != Tool::Cut {
        return None;
    }
    let object = window.doc.scene.get(window.doc.scene.selected()?)?;
    let plane = window.tools.cut.plane(object)?;
    Some(CutLine {
        normal: plane.normal,
        offset_mm: plane.normal.dot(plane.point),
        bounds: object.world_bounds()?,
    })
}

/// Draws the span over the models rather than into them: a measurement is chrome, and
/// putting it in the 3D pass would hide it inside the very feature being measured.
pub(super) fn draw_measure(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
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

/// The outline of the Cut tool's plane, a little wider than the selected model, so the
/// plane can be found where it misses the model and no line is traced on it.
pub(super) fn draw_cut_plane(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let Some(object) = window
        .doc
        .scene
        .selected()
        .and_then(|id| window.doc.scene.get(id))
    else {
        return;
    };
    let Some(bounds) = object.world_bounds() else {
        return;
    };
    let tool = &window.tools.cut;
    let (axis, across, along) = match tool.state.axis {
        crate::scene::Axis::X => (0, 1, 2),
        crate::scene::Axis::Y => (1, 0, 2),
        crate::scene::Axis::Z => (2, 0, 1),
    };
    let size = bounds.maxs - bounds.mins;
    let margin = (size.max_element() * CUT_PLANE_MARGIN).max(CUT_PLANE_MARGIN_MIN_MM);
    let (lo, hi) = (bounds.mins - margin, bounds.maxs + margin);

    let on_screen = projector(&window.view.camera, viewport);
    let corners: Option<Vec<egui::Pos2>> = [(lo, lo), (hi, lo), (hi, hi), (lo, hi)]
        .into_iter()
        .map(|(u, v)| {
            let mut corner = Vec3::ZERO;
            corner[axis] = tool.position_mm(object);
            corner[across] = u[across];
            corner[along] = v[along];
            on_screen(corner)
        })
        .collect();
    let Some(corners) = corners else {
        return;
    };
    ui.painter_at(viewport).add(egui::Shape::closed_line(
        corners,
        egui::Stroke::new(1.0, theme::colors().accent.gamma_multiply(0.6)),
    ));
}

/// Brackets on the corners of each picked model's bounds, and the size of each side, the
/// way a slicer shows what a model measures without hiding it inside a box.
pub(super) fn draw_bounds(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let on_screen = projector(&window.view.camera, viewport);
    let camera = &window.view.camera;
    let seen = |point: Vec3| {
        on_screen(point).filter(|_| !occluded(&window.doc.scene, camera.sight_to(point), point))
    };
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
        let galley = painter.layout_no_wrap(text, theme::figures(11.0), theme::colors().axis[axis]);
        let plate = egui::Rect::from_center_size(at, galley.size() + egui::vec2(8.0, 4.0));
        painter.rect_filled(plate, theme::R_CONTROL, theme::colors().panel);
        painter.galley(
            plate.center() - galley.size() / 2.0,
            galley,
            theme::colors().axis[axis],
        );
    }
}

/// Washes the face a click would lay down, over the model, so the user sees which one.
pub(super) fn draw_facet(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
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

/// Draws what the Edit mode is holding, over the models rather than into them: a stick as
/// a line and a joint as a dot, both in the picked colour.
pub(super) fn draw_picked(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
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
