//! A round status beside the print chip: a track, an arc for the share done or spinning while
//! there is no share, a glyph or the percent in the middle, and a count in a disc at its
//! corner. The printers' ring and the agents' (ADR 0264, 0462).

use egui::{Align2, Color32, Pos2, Rect, Shape, Stroke, pos2, vec2};

use crate::ui::theme;
use crate::ui::theme::layout::printer_card_ring::{SMALL, TRACK_W};

/// Share of a turn the arc spans while it spins.
const SPINNER_SHARE: f32 = 0.25;

/// What a ring shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingLook<'a> {
    pub glyph: &'a str,
    pub glyph_tint: Color32,
    pub track: Color32,
    pub arc: Color32,
    /// Share done, drawn as an arc from the top and as a percent where the ring has room.
    pub share: Option<f32>,
    /// An arc going round, where there is no share to draw.
    pub spinning: bool,
}

/// Paints a ring of `radius` round `center`, washed while `hovered`.
pub fn paint_ring(ui: &egui::Ui, center: Pos2, radius: f32, look: &RingLook<'_>, hovered: bool) {
    let colors = theme::colors();
    let painter = ui.painter();
    if hovered {
        painter.circle_filled(center, radius + 2.0, colors.hover);
    }
    painter.circle_stroke(
        center,
        radius - TRACK_W / 2.0,
        Stroke::new(TRACK_W, look.track),
    );
    let arc = if look.spinning {
        let turn = theme::motion::spin(ui.ctx()).unwrap_or(0.0);
        Some((turn, SPINNER_SHARE))
    } else {
        look.share.map(|share| (0.0, share))
    };
    if let Some((from, share)) = arc {
        let points = arc_points(center, radius - TRACK_W / 2.0, from, share);
        painter.add(Shape::line(points, Stroke::new(TRACK_W, look.arc)));
    }
    match look.share {
        Some(share) if radius > SMALL / 2.0 => {
            painter.text(
                center,
                Align2::CENTER_CENTER,
                format!("{:.0}", share * 100.0),
                theme::figures(10.0),
                colors.text_high,
            );
        }
        _ => {
            painter.text(
                center,
                Align2::CENTER_CENTER,
                look.glyph,
                theme::icon(radius * 0.85),
                look.glyph_tint,
            );
        }
    }
}

/// How many a ring stands for, in a small disc at the corner of its `rect`.
pub fn ring_badge(ui: &egui::Ui, rect: Rect, count: usize) {
    let colors = theme::colors();
    let at = rect.right_top() + vec2(-4.0, 5.0);
    ui.painter().circle_filled(at, 6.5, colors.raised);
    ui.painter()
        .circle_stroke(at, 6.5, Stroke::new(1.0, colors.line));
    ui.painter().text(
        at,
        Align2::CENTER_CENTER,
        count.to_string(),
        theme::figures(9.0),
        colors.text_high,
    );
}

/// Points along a circle from `from` of a turn, clockwise from the top, for `share` of
/// a turn; enough of them that the arc reads as round at the ring's size.
fn arc_points(center: Pos2, radius: f32, from: f32, share: f32) -> Vec<Pos2> {
    let share = share.clamp(0.0, 1.0);
    let steps = ((share * 64.0).ceil() as usize).max(1);
    (0..=steps)
        .map(|step| {
            let turn = from + share * step as f32 / steps as f32;
            let angle = turn * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arc_starts_at_the_top_and_runs_clockwise_for_its_share() {
        let center = pos2(0.0, 0.0);
        let quarter = arc_points(center, 10.0, 0.0, 0.25);
        let first = quarter.first().expect("an arc has points");
        let last = quarter.last().expect("an arc has points");
        assert!(
            (first.x).abs() < 1e-4 && (first.y + 10.0).abs() < 1e-4,
            "top: {first:?}"
        );
        assert!(
            (last.x - 10.0).abs() < 1e-4 && last.y.abs() < 1e-4,
            "right: {last:?}"
        );
    }

    #[test]
    fn a_share_past_the_whole_is_drawn_as_the_whole() {
        let center = pos2(0.0, 0.0);
        let whole = arc_points(center, 10.0, 0.0, 1.5);
        let (first, last) = (whole[0], whole[whole.len() - 1]);
        assert!(
            (first - last).length() < 1e-3,
            "a full turn closes on itself"
        );
    }
}
