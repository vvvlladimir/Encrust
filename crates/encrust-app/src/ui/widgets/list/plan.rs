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
    let lines =
        1.0 + f32::from(u8::from(row.reading.is_some())) + f32::from(u8::from(row.note.is_some()));
    let size = vec2(ui.available_width(), PLAN_LINE_H * lines);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| {
        WidgetInfo::labeled(
            WidgetType::Label,
            true,
            format!("{}, {}", row.name, row.verdict),
        )
    });
    let colors = theme::colors();
    let (glyph, tint) = row.mark.look();
    let painter = ui.painter();
    let first = rect.top() + PLAN_LINE_H / 2.0;
    let turn = match row.mark {
        PlanMark::Running => motion::spin(ui.ctx()).unwrap_or(0.0) * std::f32::consts::TAU,
        _ => 0.0,
    };
    let glyph_galley =
        painter.layout_no_wrap(glyph.to_owned(), theme::icon(theme::glyph::MARK), tint);
    let corner = pos2(rect.left() + PLAN_GLYPH_W / 2.0, first) - glyph_galley.size() / 2.0;
    painter.add(
        egui::epaint::TextShape::new(corner, glyph_galley, tint)
            .with_angle_and_anchor(turn, Align2::CENTER_CENTER),
    );
    let verdict = painter.text(
        pos2(rect.right(), first),
        Align2::RIGHT_CENTER,
        row.verdict,
        theme::small(),
        tint,
    );
    let left = rect.left() + PLAN_GLYPH_W;
    let ink = if row.mark.quiet() {
        colors.text_low
    } else {
        colors.text_high
    };
    let name_room = verdict.left() - PLAN_GAP - left;
    cut_line(
        painter,
        pos2(left, first),
        row.name,
        theme::label(),
        ink,
        name_room,
    );
    let mut at = first + PLAN_LINE_H;
    let room = rect.right() - left;
    if let Some(reading) = row.reading {
        cut_line(
            painter,
            pos2(left, at),
            reading,
            theme::small(),
            colors.text_low,
            room,
        );
        at += PLAN_LINE_H;
    }
    if let Some(note) = row.note {
        cut_line(
            painter,
            pos2(left, at),
            note,
            theme::small(),
            colors.warn,
            room,
        );
    }
    let whole: Vec<&str> = [row.reading, row.note].into_iter().flatten().collect();
    if whole.is_empty() {
        response
    } else {
        response.on_hover_text(whole.join("\n"))
    }
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
