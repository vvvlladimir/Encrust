use egui::{Rect, Ui};

use crate::i18n::tr;
use crate::ui::theme;
use crate::ui::theme::motion;
use crate::ui::widgets::{secondary_button, tone};

/// The bar of a running job: filled to `fraction` where it is known, and running on its
/// own where it is not, which is a job that cannot honestly count its work.
pub fn progress_bar(ui: &mut Ui, fraction: Option<f32>) {
    progress_bar_of(ui, fraction, theme::BAR_H);
}

/// The same bar `height` points tall, as the printer sheet's.
pub fn progress_bar_of(ui: &mut Ui, fraction: Option<f32>, height: f32) {
    let colors = theme::colors();
    let size = egui::vec2(ui.available_width(), height);
    let (track, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::new(egui::WidgetType::ProgressIndicator);
        info.value = fraction.map(f64::from);
        info
    });
    let painter = ui.painter_at(track);
    let round = egui::CornerRadius::same((height / 2.0) as u8);
    painter.rect_filled(track, round, colors.active);
    let phase = motion::sheen(ui.ctx());
    let fill = match fraction {
        Some(fraction) => track.with_max_x(track.left() + track.width() * fraction.clamp(0.0, 1.0)),
        // A job that cannot count runs a stretch of the bar along it; still, a stretch.
        None => {
            let width = track.width() * RUNNING_SHARE;
            let left = phase.map_or(track.left(), |phase| {
                track.left() + phase * (track.width() + width) - width
            });
            Rect::from_x_y_ranges(left..=left + width, track.y_range())
        }
    };
    painter.rect_filled(fill, round, colors.accent);
    if let (Some(phase), Some(_)) = (phase, fraction) {
        let width = fill.width() * RUNNING_SHARE;
        let left = fill.left() + phase * (fill.width() + width) - width;
        let sheen = Rect::from_x_y_ranges(left..=left + width, fill.y_range()).intersect(fill);
        painter.rect_filled(sheen, round, colors.text_high.gamma_multiply(SHEEN_ALPHA));
    }
}

/// How much of the bar the sheen and a running stretch cover, and how much of the light ink
/// the sheen keeps.
const RUNNING_SHARE: f32 = 0.3;
const SHEEN_ALPHA: f32 = 0.25;

/// What a tool is busy with and the bar of its run, and where the run can be stopped a way
/// to stop it. `step` is the step in words and the share done, for a run that counts; the
/// bar runs without a share for one that does not. Answers whether it was asked to stop.
pub fn busy(ui: &mut Ui, label: &str, step: Option<(&str, f32)>, stoppable: bool) -> bool {
    let share = step.map(|(_, share)| share);
    run(ui, label, share, step.map(|(words, _)| words), stoppable)
}

/// A run that tells its step: `what` above the bar, in words, then the bar filled to `share`
/// or running on its own for a run that does not count, and where the run can be stopped a
/// way to stop it. Answers whether it was asked to stop.
pub fn step_bar(ui: &mut Ui, what: &str, share: Option<f32>, stoppable: bool) -> bool {
    run(ui, what, share, None, stoppable)
}

fn run(ui: &mut Ui, what: &str, share: Option<f32>, step: Option<&str>, stoppable: bool) -> bool {
    tone(ui, what, theme::colors().text_mid);
    progress_bar(ui, share);
    if let Some(step) = step {
        tone(ui, step, theme::colors().text_low);
    }
    stoppable && secondary_button(ui, crate::ui::icon::CANCEL, &tr!("common-cancel")).clicked()
}
