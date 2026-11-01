use crate::i18n::tr;
use crate::state::Machine;
use crate::ui::{bar_pill, icon, theme};
use crate::updates::Stage;

/// A release newer than this build, offered and never applied: a click opens the update
/// sheet, or restarts into a build already installed. See `crate::updates`.
pub(super) fn update_badge(ui: &mut egui::Ui, machine: &mut Machine, compact: bool) {
    let Some(offer) = machine.updates.waiting() else {
        return;
    };
    let installed = matches!(machine.updates.stage, Stage::Installed(_));
    let downloading = machine.updates.progress().and_then(|(progress, _)| {
        let (received, total) = progress.bytes();
        total.map(|total| received * 100 / total.max(1))
    });
    let text = match (installed, downloading) {
        (true, _) => tr!("bar-restart-to-update"),
        (false, Some(percent)) => tr!("bar-update-percent", percent = percent),
        (false, None) => tr!("app-version", version = offer.version.to_string()),
    };
    let colors = theme::colors();
    let response = bar_pill(
        ui,
        icon::UPDATE,
        &text,
        compact,
        colors.accent,
        colors.accent_wash,
    );
    let tooltip = if installed {
        tr!("bar-restart-tooltip")
    } else {
        tr!("bar-update-tooltip")
    };
    if response.on_hover_text(tooltip).clicked() {
        if installed {
            machine.updates.restart();
        } else {
            machine.updates.sheet = true;
        }
    }
    ui.add_space(6.0);
}

/// What this build is, where it can be acted on: a click opens the sheet that writes a
/// bug report. See `crate::report`.
pub(super) fn alpha_badge(ui: &mut egui::Ui, machine: &mut Machine, compact: bool) {
    let colors = theme::colors();
    let response = bar_pill(
        ui,
        icon::BUG,
        &tr!("bar-alpha"),
        compact,
        colors.warn,
        colors.warn_wash,
    );
    if response.on_hover_text(tr!("bar-alpha-tooltip")).clicked() {
        machine.report.open = true;
    }
    ui.add_space(6.0);
}
