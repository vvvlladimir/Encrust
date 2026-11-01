//! The layer strip's track: a rule with the cured area as a silhouette behind it, a scale,
//! the fill between the floor and the cut, a bar at every risk, and the two thumbs; see
//! `docs/design/ui-design-system.md`, Layer track.

use egui::{Rect, Stroke, pos2, vec2};

use crate::ui::theme;

/// The track: how thick its rule and the thumbs on it.
const RULE_W: f32 = 3.0;
pub const LAYER_HANDLE: egui::Vec2 = vec2(26.0, 12.0);
/// The scale: how many ticks, how long each, and how far it stands in from the edge.
const TICKS: usize = 18;
const TICK_W: f32 = 8.0;
const TICK_INSET: f32 = 4.0;
/// The cured-area silhouette: a row every this many points up the track, its widest
/// standing so far either side of the rule.
pub const PROFILE_PITCH: f32 = 2.0;
const PROFILE_HALF_W: f32 = 22.0;
/// A risk's bar, the one under the pointer, and how far above and below it a press still
/// takes it, points.
const RISK: egui::Vec2 = vec2(28.0, 3.0);
pub const LAYER_RISK_HOT: egui::Vec2 = vec2(32.0, 5.0);
pub const LAYER_RISK_REACH: f32 = 3.0;

/// Where along `span` the pointer at `y` stands, from zero at its bottom to one at its top.
pub fn span_fraction(span: egui::Rangef, y: f32) -> f32 {
    if span.span() <= 0.0 {
        return 1.0;
    }
    ((span.max - y) / span.span()).clamp(0.0, 1.0)
}

/// How far down `span` a share `at` of the way up stands.
pub fn up(span: egui::Rangef, at: f32) -> f32 {
    egui::lerp(span.max..=span.min, at)
}

/// The rule, the cured-area profile behind it, the scale at the right, the fill between
/// the two thumbs, the risks across it and the thumbs themselves, in that order.
pub fn paint_layer_track(
    ui: &egui::Ui,
    rect: Rect,
    span: egui::Rangef,
    (at, floor): (Option<f32>, Option<(f32, usize)>),
    marks: Option<&(LayerMarks, egui::Rangef)>,
    hot: Option<usize>,
) {
    let colors = theme::colors();
    let painter = ui.painter();
    let rule = Rect::from_x_y_ranges(
        egui::Rangef::new(
            rect.center().x - RULE_W / 2.0,
            rect.center().x + RULE_W / 2.0,
        ),
        span.expand(LAYER_HANDLE.y / 2.0),
    );
    let round = egui::CornerRadius::same(u8::MAX);

    if let Some((marks, along)) = marks {
        marks.paint_profile(painter, *along, rule);
    }
    painter.rect_filled(rule, round, colors.active);
    // With nothing to cut through the rule stands alone: a scale would measure nothing.
    let Some(at) = at else {
        if let Some((marks, along)) = marks {
            marks.paint_risks(painter, *along, rule, hot);
        }
        return;
    };
    let ticks_x = egui::Rangef::new(
        rect.right() - TICK_INSET - TICK_W,
        rect.right() - TICK_INSET,
    );
    for tick in 0..TICKS {
        let y = egui::lerp(span, tick as f32 / (TICKS - 1) as f32);
        painter.hline(ticks_x, y, Stroke::new(1.0, colors.line));
    }
    let y = up(span, at);
    let floor_y = floor.map_or(up(span, 0.0), |(floor_at, _)| up(span, floor_at));
    let filled = rule.with_min_y(y).with_max_y(floor_y.max(y));
    painter.rect_filled(filled, round, colors.accent);
    if let Some((marks, along)) = marks {
        marks.paint_risks(painter, *along, rule, hot);
    }
    for y in [floor_y, y] {
        let handle = Rect::from_center_size(pos2(rect.center().x, y), LAYER_HANDLE);
        painter.rect(
            handle,
            egui::CornerRadius::same(4),
            colors.knob,
            Stroke::new(2.0, colors.accent),
            egui::StrokeKind::Outside,
        );
    }
}

/// What the strip shows of the stack along its track: the cured area up the print, as a
/// silhouette either side of the rule, and a bar at every layer named as a risk.
pub struct LayerMarks {
    /// The widest layer in each row of the track, from the plate up, over the widest of
    /// all.
    pub widths: Vec<f32>,
    /// The risks, lowest first.
    pub risks: Vec<LayerMark>,
}

/// One risk's bar: where it sits up the stack from zero to one, its colour, and its layer.
pub struct LayerMark {
    pub at: f32,
    pub tint: egui::Color32,
    pub layer: usize,
}

impl LayerMarks {
    /// One filled outline, centred on the rule, running smoothly over the rows' peaks.
    pub fn paint_profile(&self, painter: &egui::Painter, span: egui::Rangef, rule: Rect) {
        let fill = theme::colors().text_low.gamma_multiply(PROFILE_ALPHA);
        let rows = self.widths.len() as f32;
        let x = rule.center().x;
        let mut mesh = egui::Mesh::default();
        let reach = (PROFILE_SMOOTH / PROFILE_PITCH).round() as usize;
        for (row, width) in outline(&self.widths, reach).iter().enumerate() {
            let y = up(span, (row as f32 + 0.5) / rows);
            let half = if *width > 0.0 {
                RULE_W / 2.0 + width * (PROFILE_HALF_W - RULE_W / 2.0)
            } else {
                0.0
            };
            let next = mesh.vertices.len() as u32;
            mesh.colored_vertex(pos2(x - half, y), fill);
            mesh.colored_vertex(pos2(x + half, y), fill);
            if row > 0 {
                mesh.add_triangle(next - 2, next - 1, next);
                mesh.add_triangle(next - 1, next + 1, next);
            }
        }
        painter.add(mesh);
    }

    /// Over the rule and its fill, so a risk under the cut reads as well as one above it;
    /// the bar under the pointer stands larger.
    pub fn paint_risks(
        &self,
        painter: &egui::Painter,
        span: egui::Rangef,
        rule: Rect,
        hot: Option<usize>,
    ) {
        for (index, mark) in self.risks.iter().enumerate() {
            let size = if hot == Some(index) {
                LAYER_RISK_HOT
            } else {
                RISK
            };
            let bar = Rect::from_center_size(pos2(rule.center().x, up(span, mark.at)), size);
            painter.rect_filled(bar, egui::CornerRadius::same(1), mark.tint);
        }
    }

    /// The risk whose bar `pointer` is on, the nearest where two touch: bisected, since a
    /// stack may hold thousands.
    pub fn under(&self, span: egui::Rangef, x: f32, pointer: egui::Pos2) -> Option<usize> {
        if (pointer.x - x).abs() > LAYER_RISK_HOT.x / 2.0 || span.span() <= 0.0 {
            return None;
        }
        let at = span_fraction(span, pointer.y);
        let split = self.risks.partition_point(|mark| mark.at < at);
        let near = split.saturating_sub(1)..(split + 1).min(self.risks.len());
        near.map(|index| (index, (up(span, self.risks[index].at) - pointer.y).abs()))
            .filter(|(_, off)| *off <= RISK.y / 2.0 + LAYER_RISK_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }
}

/// `widths` drawn through their peaks: each the widest within `reach` rows, then averaged
/// over as many, so the outline never dips under a row's own width.
pub fn outline(widths: &[f32], reach: usize) -> Vec<f32> {
    let around = |row: usize| row.saturating_sub(reach)..(row + reach + 1).min(widths.len());
    let peaks: Vec<f32> = (0..widths.len())
        .map(|row| widths[around(row)].iter().copied().fold(0.0, f32::max))
        .collect();
    (0..peaks.len())
        .map(|row| {
            let near = &peaks[around(row)];
            near.iter().sum::<f32>() / near.len() as f32
        })
        .collect()
}

/// How far the outline reaches either side to join peaks, points.
const PROFILE_SMOOTH: f32 = 8.0;
/// How much of `text_low` the cured-area profile keeps.
const PROFILE_ALPHA: f32 = 0.35;

#[cfg(test)]
mod tests {
    use super::outline;

    #[test]
    fn the_outline_joins_close_peaks_and_never_dips_under_a_row() {
        let comb = [1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        let joined = outline(&comb, 1);
        assert!(
            joined.iter().all(|width| (width - 1.0).abs() < 1e-6),
            "{joined:?}"
        );
        let lone = [0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.25];
        let drawn = outline(&lone, 2);
        for (row, (raw, drawn)) in lone.iter().zip(&drawn).enumerate() {
            assert!(drawn >= raw, "row {row}: {drawn} under {raw}");
        }
    }
}
