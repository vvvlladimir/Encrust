//! A picked model's bounds: brackets on its corners and the length of each side.

use core_geometry::Vec3;

use crate::gizmo::{self, HANDLES_REACH};
use crate::i18n::tr;
use crate::panels::Window;
use crate::pick::occluded;
use crate::ui::theme;

use super::overlays::projector;
use super::{picked_pivots, shows_gizmo};

/// How far along each edge a bounds bracket reaches, as a fraction of that edge.
const BRACKET_ARM: f32 = 0.15;

/// How many pieces each arm is tested for being hidden in.
const BRACKET_PIECES: u32 = 6;

/// Brackets on the corners of each picked model's bounds, and the size of each side.
pub(super) fn draw_bounds(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let on_screen = projector(&window.view.camera, viewport);
    let camera = &window.view.camera;
    let seen = |point: Vec3| {
        on_screen(point).filter(|_| !occluded(&window.doc.scene, camera.sight_to(point), point))
    };
    let painter = ui.painter_at(viewport);
    let handles = shows_gizmo(window)
        .then(|| on_screen(gizmo::centre(&picked_pivots(window))))
        .flatten();
    let stroke = egui::Stroke::new(1.5, theme::scene().bounds);
    let picked = window.doc.scene.selection().iter();
    let boxes = picked.filter_map(|id| window.doc.scene.get(*id)?.world_bounds());
    for bounds in boxes {
        let (lo, hi) = (bounds.mins, bounds.maxs);
        for corner in 0..8 {
            bracket(&painter, stroke, corner, (lo, hi), &seen, &on_screen);
        }
        size_labels(&painter, &seen, handles, lo, hi);
    }
}

/// The three arms of the bracket on corner `corner` of the box `lo`..`hi`, each bit of
/// `corner` picking the high side of one axis.
fn bracket(
    painter: &egui::Painter,
    stroke: egui::Stroke,
    corner: usize,
    (lo, hi): (Vec3, Vec3),
    seen: &impl Fn(Vec3) -> Option<egui::Pos2>,
    on_screen: &impl Fn(Vec3) -> Option<egui::Pos2>,
) {
    let pick = |axis: usize| {
        if corner >> axis & 1 == 0 {
            lo[axis]
        } else {
            hi[axis]
        }
    };
    let at = Vec3::new(pick(0), pick(1), pick(2));
    let inward = (lo + hi) * 0.5 - at;
    let arm = (hi - lo) * BRACKET_ARM;
    for axis in 0..3 {
        let mut end = at;
        end[axis] += arm[axis].copysign(inward[axis]);
        // Cut into pieces, so an arm half behind the model shows its other half.
        for piece in 0..BRACKET_PIECES {
            let along = |t: f32| at.lerp(end, t / BRACKET_PIECES as f32);
            let (from, to) = (along(piece as f32), along(piece as f32 + 1.0));
            if let (Some(_), Some(from), Some(to)) =
                (seen(from.lerp(to, 0.5)), on_screen(from), on_screen(to))
            {
                painter.line_segment([from, to], stroke);
            }
        }
    }
}

/// The length of each side, written beside the bottom front edge for X and Y and up the
/// front left edge for Z, in the axis colour its field uses, and moved off the `handles`.
fn size_labels(
    painter: &egui::Painter,
    on_screen: &impl Fn(Vec3) -> Option<egui::Pos2>,
    handles: Option<egui::Pos2>,
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
        let text = tr!("value-mm", value = format!("{:.2}", size[axis]));
        let colour = theme::colors().axis[axis];
        let galley = painter.layout_no_wrap(text, theme::figures(11.0), colour);
        let plate = plate_around(at, &galley);
        let plate = handles.map_or(plate, |centre| clear_of(plate, centre, HANDLES_REACH));
        figure_on(painter, plate, galley, colour);
    }
}

/// A figure laid out on a small plate, centred on `at`.
pub(super) fn figure_plate(
    painter: &egui::Painter,
    at: egui::Pos2,
    text: String,
    colour: egui::Color32,
) {
    let galley = painter.layout_no_wrap(text, theme::figures(11.0), colour);
    figure_on(painter, plate_around(at, &galley), galley, colour);
}

fn plate_around(at: egui::Pos2, galley: &egui::Galley) -> egui::Rect {
    egui::Rect::from_center_size(at, galley.size() + egui::vec2(8.0, 4.0))
}

fn figure_on(
    painter: &egui::Painter,
    plate: egui::Rect,
    galley: std::sync::Arc<egui::Galley>,
    colour: egui::Color32,
) {
    painter.rect_filled(plate, theme::R_CONTROL, theme::colors().panel);
    painter.galley(plate.center() - galley.size() / 2.0, galley, colour);
}

/// `plate` pushed straight away from `centre` until it clears the disc of `radius`, so its
/// nearest edge stands on the circle; straight down when it sits on the centre.
fn clear_of(plate: egui::Rect, centre: egui::Pos2, radius: f32) -> egui::Rect {
    let offset = plate.center() - centre;
    let distance = offset.length();
    let away = if distance > f32::EPSILON {
        offset / distance
    } else {
        egui::Vec2::DOWN
    };
    let half_across = away.x.abs() * plate.width() / 2.0 + away.y.abs() * plate.height() / 2.0;
    let needed = radius + half_across;
    if distance >= needed {
        return plate;
    }
    plate.translate(away * (needed - distance))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_on_the_handles_moves_out_to_their_edge() {
        let plate = egui::Rect::from_center_size(egui::pos2(110.0, 100.0), egui::vec2(40.0, 16.0));
        let moved = clear_of(plate, egui::pos2(100.0, 100.0), 60.0);
        assert!(
            (moved.left() - 160.0).abs() < 1e-4,
            "expected the near edge on the circle at x = 160, got {}",
            moved.left()
        );
        assert_eq!(moved.center().y, 100.0, "pushed straight out, not sideways");
    }

    #[test]
    fn a_label_clear_of_the_handles_stays_where_it_is() {
        let plate = egui::Rect::from_center_size(egui::pos2(300.0, 100.0), egui::vec2(40.0, 16.0));
        assert_eq!(clear_of(plate, egui::pos2(100.0, 100.0), 60.0), plate);
    }

    #[test]
    fn a_label_on_the_very_centre_drops_below() {
        let plate = egui::Rect::from_center_size(egui::pos2(100.0, 100.0), egui::vec2(40.0, 16.0));
        let moved = clear_of(plate, egui::pos2(100.0, 100.0), 60.0);
        assert!(
            (moved.top() - 160.0).abs() < 1e-4,
            "expected the top edge at y = 160, got {}",
            moved.top()
        );
    }
}
