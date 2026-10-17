//! The General page: how the window behaves, set once and kept in `preferences.json`.

use egui::Ui;

use crate::i18n::{self, Language, tr};
use crate::panels::settings::form_card;
use crate::state::Machine;
use crate::ui::{hint, switch};

pub fn form(ui: &mut Ui, machine: &mut Machine) {
    let general = &mut machine.general;
    form_card(ui, &tr!("general-language"), |ui| {
        language(ui, &mut general.language)
    });
    form_card(ui, &tr!("general-start"), |ui| {
        switch(ui, &mut general.start_page, &tr!("general-start-page"));
        hint(ui, &tr!("general-start-page-hint"));
    });
    form_card(ui, &tr!("general-viewport"), |ui| {
        switch(ui, &mut general.invert_zoom, &tr!("general-invert-zoom"));
    });
    #[cfg(not(target_arch = "wasm32"))]
    desk(ui, machine);
}

/// The language the window is written in. Each is named in itself, so a user who cannot
/// read the current one still finds theirs.
fn language(ui: &mut Ui, choice: &mut Option<Language>) {
    let system = tr!(
        "general-language-system",
        name = i18n::system().native_name()
    );
    let shown = choice.map_or_else(
        || system.clone(),
        |language| language.native_name().to_owned(),
    );
    egui::ComboBox::from_id_salt("general-language")
        .width(ui.available_width())
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(choice, None, system);
            for language in Language::ALL {
                ui.selectable_value(choice, Some(language), language.native_name());
            }
        });
    hint(ui, &tr!("general-language-hint"));
}

/// What only a desk has: folders.
#[cfg(not(target_arch = "wasm32"))]
fn desk(ui: &mut Ui, machine: &mut Machine) {
    use anyhow::Context as _;

    use crate::ui::{icon, inline_button, quiet_button};

    let general = &mut machine.general;
    form_card(ui, &tr!("general-sliced-files"), |ui| {
        let shown = general.sliced_dir.as_ref().map_or_else(
            || tr!("general-sliced-dir-system"),
            |dir| tr!("general-sliced-dir", dir = dir.display().to_string()),
        );
        hint(ui, &shown);
        ui.horizontal(|ui| {
            if inline_button(ui, icon::OPEN, &tr!("general-choose-folder"), false, true).clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                general.sliced_dir = Some(dir);
            }
            let set = general.sliced_dir.is_some();
            if quiet_button(ui, icon::RESET, &tr!("general-forget-folder"), false, set).clicked() {
                general.sliced_dir = None;
            }
        });
    });
    form_card(ui, &tr!("general-profiles"), |ui| {
        let Some(dir) = printer_profiles::user_dir() else {
            hint(ui, &tr!("general-profiles-nowhere"));
            return;
        };
        let shown = dir.display().to_string();
        hint(ui, &tr!("general-profiles-dir", dir = shown.clone()));
        if inline_button(ui, icon::OPEN, &tr!("general-open-folder"), false, true).clicked() {
            let opened =
                reveal(&dir).with_context(|| tr!("error-cannot-open", path = shown.clone()));
            machine
                .status
                .report(&tr!("status-opened", path = shown), opened);
        }
    });
}

/// Shows `dir` in the system's file manager, making it first: most users have never
/// saved a profile, so it may not be there yet.
#[cfg(not(target_arch = "wasm32"))]
fn reveal(dir: &std::path::Path) -> std::io::Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::fs::create_dir_all(dir)?;
    std::process::Command::new(opener)
        .arg(dir)
        .spawn()
        .map(drop)
}
