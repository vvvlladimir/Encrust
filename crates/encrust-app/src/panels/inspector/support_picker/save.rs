use super::rows::InHand;
use crate::i18n::tr;
use crate::panels::Window;
use crate::settings::keep_support;
use crate::ui::{icon, icon_button};
use anyhow::Context as _;

/// Save: over the automatic preset shown, at once, since none is ever added; for any
/// other, over the one it came from or as a new one.
pub(super) fn save_button(ui: &mut egui::Ui, window: &mut Window, hand: &InHand) {
    let tip = tr!("shape-save-tooltip");
    let button = ui
        .add_enabled_ui(hand.edited, |ui| icon_button(ui, icon::SAVE, &tip))
        .inner;
    if let Some(id) = &hand.auto_source {
        if button.clicked() {
            save_auto(window, id);
        }
        return;
    }
    let (mut over, mut as_new) = (false, false);
    egui::Popup::menu(&button).show(|ui| {
        let save = egui::Button::new(tr!("shape-save"));
        over = ui.add_enabled(hand.source.is_some(), save).clicked();
        as_new = ui.button(tr!("shape-save-as-new")).clicked();
    });
    if over && let Some(id) = &hand.source {
        save_over(window, id);
    }
    if as_new {
        save_new(window);
    }
}

pub(super) fn save_auto(window: &mut Window, id: &str) {
    let Some(profile) = window.tools.supports.auto.clone() else {
        return;
    };
    let kept = window
        .machine
        .slicing
        .catalogue
        .save_auto(id, &profile)
        .context(tr!("support-cannot-save"));
    window.machine.status.report(&tr!("shape-saved"), kept);
}

pub(super) fn save_over(window: &mut Window, id: &str) {
    let profile = window.tools.supports.profile.clone();
    let kept = window
        .machine
        .slicing
        .catalogue
        .save_support(id, &profile)
        .context(tr!("support-cannot-save"));
    if window
        .machine
        .status
        .report(&tr!("shape-saved"), kept)
        .is_some()
    {
        window.tools.supports.saved = Some(profile);
    }
}

/// Saves the preset in hand under a name of its own, and opens it in the settings.
pub(super) fn save_new(window: &mut Window) {
    let profile = window.tools.supports.profile.clone();
    let kept = keep_support(&mut window.machine.slicing.catalogue, &profile)
        .context(tr!("support-cannot-save"));
    if window
        .machine
        .status
        .report(&tr!("shape-saved"), kept)
        .is_some()
    {
        window
            .machine
            .settings
            .open_supports(&window.machine.slicing.catalogue, &profile);
    }
}
