//! The marks a model's row carries for what its import found, and their tooltips.

use super::POPUP_W;
use crate::i18n::tr;
use crate::scene::{ImportSummary, SceneObject};
use crate::ui::{hint, icon, tag, theme};
use crate::ui::{inline_button, row_mark};
use egui::Rect;

/// What the end of a row says about a model: a model that will not slice as it stands is
/// named broken in words, a model repair only had to tidy gets the quiet sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    Repaired,
    Broken,
}

/// The mark at the end of the row, with what import found behind a click, and the repair on a
/// broken model. Returns where it starts, and whether repair was asked for.
pub(super) fn import_mark(
    ui: &mut egui::Ui,
    rect: Rect,
    object: &SceneObject,
    right: f32,
    mark: Mark,
    lines: &[String],
) -> (f32, bool) {
    let colors = theme::colors();
    let id = ui.id().with((object.id, "import"));
    let response = match mark {
        Mark::Repaired => quiet_mark(ui, rect, id, right),
        Mark::Broken => tag(
            ui,
            egui::pos2(right, rect.center().y),
            id,
            &tr!("models-broken"),
            (colors.danger, colors.danger_wash),
        ),
    };
    let mut repair = false;
    egui::Popup::menu(&response).show(|ui| {
        ui.set_max_width(POPUP_W);
        for line in lines {
            hint(ui, line);
        }
        if mark == Mark::Broken {
            ui.add_space(theme::NOTE_GAP);
            repair = inline_button(ui, icon::REPAIR, &tr!("notice-repair"), false, true).clicked();
        }
    });
    (response.rect.left(), repair)
}

/// The quiet sign of a model repair only had to tidy, `right` its right end.
pub(super) fn quiet_mark(
    ui: &mut egui::Ui,
    rect: Rect,
    id: egui::Id,
    right: f32,
) -> egui::Response {
    let at = egui::pos2(right, rect.center().y);
    row_mark(ui, at, id, icon::WARNING, &tr!("models-import-found"))
}

/// What is wrong with a model, what is odd about it and what repair did, and how loudly.
/// `None` when the mesh came in clean.
pub(super) fn trouble(summary: &ImportSummary) -> Option<(Mark, Vec<String>)> {
    let defects = summary.defects();
    let mark = if defects.is_empty() {
        Mark::Repaired
    } else {
        Mark::Broken
    };
    let lines = [defects, summary.notes(), summary.repairs()].concat();
    (!lines.is_empty()).then_some((mark, lines))
}
