//! The drawing beside the tolerance fields: a body with a hole through it, and the side
//! `a` moves the hole's wall to and `b` the outer wall, positive when a field is zero.

use std::f32::consts::TAU;

use egui::{Align2, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};

use crate::ui::theme;

/// The drawing's side, points.
pub const SIZE: f32 = 120.0;
/// Half the body's side, the hole's radius and how far a moved wall is drawn, points.
const BODY: f32 = 34.0;
const HOLE: f32 = 18.0;
const SHIFT: f32 = 10.0;

const LINE_W: f32 = 1.2;
const ARROW: f32 = 3.5;
const DASH: f32 = 3.0;
const CIRCLE_STEPS: usize = 48;

/// Draws the body and the two walls as `hole_mm` and `outer_mm` move them.
pub fn tolerance(ui: &mut Ui, hole_mm: f32, outer_mm: f32) {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(vec2(SIZE, SIZE), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme::R_SURFACE, colors.sunken);
    let centre = rect.center();
    let ink = Stroke::new(LINE_W, colors.text_mid);
    let moved = Stroke::new(LINE_W, colors.accent);

    let body = Rect::from_center_size(centre, vec2(BODY * 2.0, BODY * 2.0));
    painter.rect(body, 0.0, colors.raised, ink, egui::StrokeKind::Middle);
    painter.circle(centre, HOLE, colors.sunken, ink);

    // A positive `a` closes the hole and a positive `b` grows the body.
    let a = -SHIFT * sign(hole_mm);
    let b = SHIFT * sign(outer_mm);
    let hole = circle(centre, HOLE + a);
    painter.extend(Shape::dashed_line(&hole, moved, DASH, DASH));
    let grown = body.expand(b);
    let corners = [
        grown.left_top(),
        grown.right_top(),
        grown.right_bottom(),
        grown.left_bottom(),
        grown.left_top(),
    ];
    painter.extend(Shape::dashed_line(&corners, moved, DASH, DASH));

    let hole_edge = pos2(centre.x - HOLE, centre.y);
    dimension(&painter, hole_edge, hole_edge - vec2(a, 0.0), "a");
    let outer_edge = pos2(body.right(), centre.y);
    dimension(&painter, outer_edge, outer_edge + vec2(b, 0.0), "b");
}

fn sign(value_mm: f32) -> f32 {
    if value_mm < 0.0 { -1.0 } else { 1.0 }
}

fn circle(centre: Pos2, radius: f32) -> Vec<Pos2> {
    (0..=CIRCLE_STEPS)
        .map(|step| {
            let angle = TAU * step as f32 / CIRCLE_STEPS as f32;
            centre + radius * vec2(angle.cos(), angle.sin())
        })
        .collect()
}

/// A level arrow between two walls with its letter over it.
fn dimension(painter: &Painter, from: Pos2, to: Pos2, label: &str) {
    let colors = theme::colors();
    painter.line_segment([from, to], Stroke::new(LINE_W, colors.accent));
    arrowhead(painter, from, to);
    arrowhead(painter, to, from);
    painter.text(
        from.lerp(to, 0.5) - vec2(0.0, 4.0),
        Align2::CENTER_BOTTOM,
        label,
        theme::figures(11.0),
        colors.accent_soft,
    );
}

fn arrowhead(painter: &Painter, tip: Pos2, tail: Pos2) {
    let back = (tail - tip).normalized() * ARROW;
    let side = back.rot90() * 0.6;
    painter.add(Shape::convex_polygon(
        vec![tip, tip + back + side, tip + back - side],
        theme::colors().accent,
        Stroke::NONE,
    ));
}
