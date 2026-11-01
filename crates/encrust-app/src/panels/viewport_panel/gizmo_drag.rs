//! What a drag on the gizmo has done, drawn while it lasts: where the pivot started, the
//! line an arrow keeps to, and the amount beside the cursor.

use core_geometry::Vec3;

use crate::gizmo::GizmoDrag;
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::theme;

use super::bounds::figure_plate;
use super::overlays::projector;

/// How far the arrow's line reaches each way from where the drag began, mm, and how many
/// times it is halved when an end falls behind the eye.
const AXIS_LINE_MM: f32 = 1000.0;
const AXIS_LINE_HALVINGS: u32 = 6;

/// Where the amount stands from the cursor, points: below and to the right of the pointer.
const READOUT_OFFSET: egui::Vec2 = egui::vec2(36.0, 22.0);

const START_DOT_R: f32 = 3.0;
const TRAIL_DASH: f32 = 4.0;

/// The trail and the amount of the drag under way, if any.
pub(super) fn draw_gizmo_drag(ui: &egui::Ui, window: &Window, viewport: egui::Rect) {
    let Some(drag) = window.view.gizmo.drag() else {
        return;
    };
    let painter = ui.painter_at(viewport);
    if let GizmoDrag::Move { from, by, axis, .. } = drag {
        let on_screen = projector(&window.view.camera, viewport);
        draw_trail(&painter, &on_screen, from, by, axis);
    }
    if let Some(cursor) = ui.input(|input| input.pointer.latest_pos()) {
        let (text, colour) = readout(drag);
        figure_plate(&painter, cursor + READOUT_OFFSET, text, colour);
    }
}

/// A dot where the pivot started, a dashed line to where it is, and for an arrow its line
/// through the start, faint, in the arrow's colour.
fn draw_trail(
    painter: &egui::Painter,
    on_screen: &impl Fn(Vec3) -> Option<egui::Pos2>,
    from: Vec3,
    by: Vec3,
    axis: Option<usize>,
) {
    let Some(start) = on_screen(from) else {
        return;
    };
    if let (Some(index), Some(direction)) = (axis, by.try_normalize()) {
        let colour = theme::scene().gizmo[index].gamma_multiply(0.45);
        if let Some(ends) = axis_line(on_screen, from, direction) {
            painter.line_segment(ends, egui::Stroke::new(1.0, colour));
        }
    }
    let quiet = theme::scene().bounds.gamma_multiply(0.7);
    if let Some(now) = on_screen(from + by) {
        let stroke = egui::Stroke::new(1.0, quiet);
        painter.extend(egui::Shape::dashed_line(
            &[start, now],
            stroke,
            TRAIL_DASH,
            TRAIL_DASH,
        ));
    }
    painter.circle_filled(start, START_DOT_R, quiet);
}

/// The ends of the line through `from` along `direction`, as long as both stay in front of
/// the eye.
fn axis_line(
    on_screen: &impl Fn(Vec3) -> Option<egui::Pos2>,
    from: Vec3,
    direction: Vec3,
) -> Option<[egui::Pos2; 2]> {
    (0..AXIS_LINE_HALVINGS)
        .map(|halving| AXIS_LINE_MM / 2f32.powi(halving as i32))
        .find_map(|reach| {
            Some([
                on_screen(from - direction * reach)?,
                on_screen(from + direction * reach)?,
            ])
        })
}

/// The amount a drag has moved or turned, and its colour: the axis's, or the text's for
/// the circle.
fn readout(drag: GizmoDrag) -> (String, egui::Color32) {
    let colour = |axis: Option<usize>| {
        axis.map_or(theme::colors().text_high, |index| {
            theme::colors().axis[index]
        })
    };
    match drag {
        GizmoDrag::Move {
            axis: Some(index),
            reading_mm,
            ..
        } => (
            tr!("value-mm", value = format!("{reading_mm:+.2}")),
            colour(Some(index)),
        ),
        GizmoDrag::Move { reading_mm, .. } => (
            tr!("value-mm", value = format!("{reading_mm:.2}")),
            colour(None),
        ),
        GizmoDrag::Turn { axis, by_deg } => (
            tr!("value-deg", value = format!("{by_deg:+.1}")),
            colour(axis),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_along_an_arrow_reads_signed_millimetres() {
        let drag = GizmoDrag::Move {
            from: Vec3::ZERO,
            by: Vec3::new(-12.5, 0.0, 0.0),
            axis: Some(0),
            reading_mm: -12.5,
        };
        let (text, colour) = readout(drag);
        assert_eq!(text, tr!("value-mm", value = "-12.50"));
        assert_eq!(colour, theme::colors().axis[0]);
    }

    #[test]
    fn a_turn_reads_signed_degrees_to_one_decimal() {
        let (text, _) = readout(GizmoDrag::Turn {
            axis: Some(2),
            by_deg: 15.0,
        });
        assert_eq!(text, tr!("value-deg", value = "+15.0"));
    }
}
