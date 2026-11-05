use egui::{Align2, Color32, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::ui::theme::motion;
use crate::ui::{icon, theme};

/// Where a step of a recipe's plan stands: foreseen before a run, or reached in one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanMark {
    Runs,
    Skipped,
    WhenReached,
    Disabled,
    Stops,
    Pauses,
    Done,
    Running,
    ToCome,
}

impl PlanMark {
    /// Its glyph and the tint of glyph and verdict.
    fn look(self) -> (&'static str, Color32) {
        let colors = theme::colors();
        match self {
            Self::Runs => (icon::PLAY, colors.text_mid),
            Self::Skipped => (icon::SKIPPED, colors.text_low),
            Self::WhenReached => (icon::WHEN_REACHED, colors.text_low),
            Self::Disabled => (icon::STEP_OFF, colors.text_low),
            Self::Stops => (icon::STOP, colors.danger),
            Self::Pauses => (icon::PAUSE, colors.accent_soft),
            Self::Done => (icon::IN_HAND, colors.ok),
            Self::Running => (icon::JOB, colors.accent_soft),
            Self::ToCome => (icon::STEP_LATER, colors.text_low),
        }
    }

    /// Whether the step does nothing, so its name stands in the low tone.
    fn quiet(self) -> bool {
        matches!(self, Self::Skipped | Self::Disabled | Self::ToCome)
    }
}

/// One step of a plan: its glyph, its name, its verdict at the right; under them, quieter,
/// the readings that decided it, and a note in `warn` where it undoes an earlier step.
pub struct PlanRow<'a> {
    pub name: &'a str,
    pub verdict: &'a str,
    pub mark: PlanMark,
    pub reading: Option<&'a str>,
    pub note: Option<&'a str>,
}

/// Draws a [`PlanRow`]; the readings and the note are cut to one line each, read whole on
/// hover.
pub fn plan_row(ui: &mut Ui, row: &PlanRow<'_>) -> Response {
    let under: Vec<(&str, Color32)> = [
        row.reading
            .map(|reading| (reading, theme::colors().text_low)),
        row.note.map(|note| (note, theme::colors().warn)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let size = vec2(ui.available_width(), PLAN_LINE_H * (1 + under.len()) as f32);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| {
        let said = format!("{}, {}", row.name, row.verdict);
        WidgetInfo::labeled(WidgetType::Label, true, said)
    });
    head_line(ui, rect, row);
    let left = rect.left() + PLAN_GLYPH_W;
    for (line, (text, ink)) in under.iter().enumerate() {
        let at = pos2(left, rect.top() + PLAN_LINE_H * (line as f32 + 1.5));
        cut_line(
            ui.painter(),
            at,
            text,
            theme::small(),
            *ink,
            rect.right() - left,
        );
    }
    match under.is_empty() {
        true => response,
        false => {
            let whole: Vec<&str> = under.iter().map(|(text, _)| *text).collect();
            response.on_hover_text(whole.join("\n"))
        }
    }
}

/// A plan row's first line: the glyph, turning while the step runs, the name and the verdict.
fn head_line(ui: &Ui, rect: egui::Rect, row: &PlanRow<'_>) {
    let colors = theme::colors();
    let (glyph, tint) = row.mark.look();
    let painter = ui.painter();
    let middle = rect.top() + PLAN_LINE_H / 2.0;
    let turn = match row.mark {
        PlanMark::Running => motion::spin(ui.ctx()).unwrap_or(0.0) * std::f32::consts::TAU,
        _ => 0.0,
    };
    let font = theme::icon(theme::glyph::MARK);
    let glyph = painter.layout_no_wrap(glyph.to_owned(), font, tint);
    let corner = pos2(rect.left() + PLAN_GLYPH_W / 2.0, middle) - glyph.size() / 2.0;
    let shape = egui::epaint::TextShape::new(corner, glyph, tint);
    painter.add(shape.with_angle_and_anchor(turn, Align2::CENTER_CENTER));
    let right = pos2(rect.right(), middle);
    let verdict = painter.text(
        right,
        Align2::RIGHT_CENTER,
        row.verdict,
        theme::small(),
        tint,
    );
    let left = rect.left() + PLAN_GLYPH_W;
    let ink = match row.mark.quiet() {
        true => colors.text_low,
        false => colors.text_high,
    };
    let room = verdict.left() - PLAN_GAP - left;
    cut_line(
        painter,
        pos2(left, middle),
        row.name,
        theme::label(),
        ink,
        room,
    );
}

/// `text` from `left_center`, cut with an ellipsis at `room` points.
fn cut_line(
    painter: &egui::Painter,
    left_center: egui::Pos2,
    text: &str,
    font: egui::FontId,
    ink: Color32,
    room: f32,
) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room.max(0.0));
    let galley = painter.layout_job(job);
    let top = left_center.y - galley.size().y / 2.0;
    painter.galley(pos2(left_center.x, top), galley, ink);
}

/// A line of a plan row, the column its glyph stands in and the room before its verdict.
const PLAN_LINE_H: f32 = 20.0;
const PLAN_GLYPH_W: f32 = 20.0;
const PLAN_GAP: f32 = 8.0;
