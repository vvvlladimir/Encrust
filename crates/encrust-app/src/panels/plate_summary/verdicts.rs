//! What the plate card says over its figures about models that may not print as they stand.

use crate::i18n::tr;
use crate::panels::Window;
use crate::plate::{Standing, standing};
use crate::scene::ObjectId;
use crate::ui::theme::layout::plate_summary::FIGURES_GAP;
use crate::ui::{Inline, icon, inline_button, notice, theme};

/// A model left broken, past the build volume, in the inset or meeting another is a verdict
/// here rather than a blocker: this is the last place to turn back before slicing.
pub(super) fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let scene = &window.doc.scene;
    let broken = crate::repair::broken_on_the_plate(scene);
    let plate = &window.doc.plate;
    let Standing { off, in_inset } = match plate.name {
        Some(_) => standing(ui.ctx(), scene, plate),
        None => Standing::default(),
    };
    let said = [
        (
            broken,
            tr!("summary-broken", count = broken),
            tr!("summary-broken-why", count = broken),
        ),
        (
            off,
            tr!("summary-off-plate", count = off),
            tr!("summary-off-plate-why"),
        ),
        (
            in_inset,
            tr!("summary-in-inset", count = in_inset),
            tr!("summary-in-inset-why"),
        ),
    ];
    for (_, text, why) in said.iter().filter(|(count, _, _)| *count > 0) {
        warning(ui, text, why);
    }
    meeting(ui, window);
}

/// The models standing into another, and a press that picks them all.
fn meeting(ui: &mut egui::Ui, window: &mut Window) {
    let scene = &window.doc.scene;
    let mut meeting: Vec<ObjectId> = window.doc.meeting.met(scene).concat();
    meeting.sort_unstable();
    meeting.dedup();
    if meeting.is_empty() {
        return;
    }
    let count = meeting.len();
    let text = tr!("summary-meeting", count = count);
    warning(ui, &text, &tr!("summary-meeting-why", count = count));
    let pick = tr!("summary-meeting-pick");
    if inline_button(ui, Inline::new(&pick).glyph(icon::SELECT)).clicked() {
        window.doc.scene.select_many(&meeting);
    }
    ui.add_space(FIGURES_GAP);
}

fn warning(ui: &mut egui::Ui, text: &str, why: &str) {
    notice(ui, icon::WARNING, theme::colors().warn, text, why);
    ui.add_space(FIGURES_GAP);
}
