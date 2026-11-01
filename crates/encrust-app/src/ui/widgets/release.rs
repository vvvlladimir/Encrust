//! What the update sheet is drawn from: the release it offers, and the steps its install
//! walks. See `docs/design/ui-design-system.md`, Release.

use egui::{Align2, Sense, Ui, pos2, vec2};

use crate::ui::{icon, theme};

/// The release a sheet offers: its name, and a line under it.
pub fn release_hero(ui: &mut Ui, title: &str, line: &str) {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HERO_H), Sense::hover());
    let painter = ui.painter();
    let x = rect.left();
    let title_at = pos2(x, rect.center().y - 1.0);
    painter.text(
        title_at,
        Align2::LEFT_BOTTOM,
        title,
        theme::dialog_title(),
        colors.text_high,
    );
    let line_at = pos2(x, rect.center().y + 3.0);
    painter.text(
        line_at,
        Align2::LEFT_TOP,
        line,
        theme::body(),
        colors.text_low,
    );
}

/// Where a step of the install stands.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Done,
    Now,
    Later,
}

/// The install's steps in a row, the one under way spinning, those done ticked.
pub fn steps(ui: &mut Ui, steps: &[(&str, StepState)]) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = STEP_GAP;
        for (name, state) in steps {
            let (glyph, ink) = match state {
                StepState::Done => (icon::IN_HAND, colors.ok),
                StepState::Now => (icon::JOB, colors.accent_soft),
                StepState::Later => (icon::STEP_LATER, colors.text_low),
            };
            let text_ink = if *state == StepState::Later {
                colors.text_low
            } else {
                colors.text_high
            };
            let word = ui
                .painter()
                .layout_no_wrap((*name).to_owned(), theme::small(), text_ink);
            let size = vec2(GLYPH_W + word.size().x, word.size().y.max(GLYPH_W));
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let painter = ui.painter();
            let glyph_at = pos2(rect.left() + GLYPH_W / 2.0 - 2.0, rect.center().y);
            painter.text(
                glyph_at,
                Align2::CENTER_CENTER,
                glyph,
                theme::icon(12.0),
                ink,
            );
            let word_at = pos2(rect.left() + GLYPH_W, rect.center().y - word.size().y / 2.0);
            painter.galley(word_at, word, text_ink);
            if *state == StepState::Now {
                ui.ctx().request_repaint();
            }
        }
    });
}

/// Points: the hero's height; a step's glyph column and the gap between steps.
const HERO_H: f32 = 40.0;
const GLYPH_W: f32 = 18.0;
const STEP_GAP: f32 = 18.0;
