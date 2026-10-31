//! The General page: how the window behaves and where its files go, kept in
//! `preferences.json`.

use egui::Ui;

use crate::i18n::tr;
use crate::panels::settings::form_card;
use crate::scene::Scene;
use crate::state::Machine;
use crate::ui::{describe, hint, icon, icon_button, switch};

pub fn form(ui: &mut Ui, machine: &mut Machine, scene: &Scene) {
    let general = &mut machine.general;
    form_card(ui, &tr!("general-start"), |ui| {
        switch(ui, &mut general.start_page, &tr!("general-start-page"));
        hint(ui, &tr!("general-start-page-hint"));
    });
    form_card(ui, &tr!("general-viewport"), |ui| {
        switch(ui, &mut general.invert_zoom, &tr!("general-invert-zoom"));
    });
    #[cfg(not(target_arch = "wasm32"))]
    sliced_folder(ui, machine);
    file_name(ui, machine, scene);
    #[cfg(not(target_arch = "wasm32"))]
    profiles_folder(ui, machine);
}

/// The template a sliced file is named by, and the name it makes of what is in hand now.
fn file_name(ui: &mut Ui, machine: &mut Machine, scene: &Scene) {
    let template = &mut machine.general.file_name;
    form_card(ui, &tr!("general-file-name"), |ui| {
        ui.horizontal(|ui| {
            let reset = template != core_engine::DEFAULT_TEMPLATE;
            let buttons = if reset { RESET_W } else { 0.0 };
            ui.add(
                crate::ui::text_box(egui::TextEdit::singleline(template))
                    .font(crate::ui::theme::typed_path())
                    .desired_width((ui.available_width() - buttons).max(0.0)),
            );
            if reset && icon_button(ui, icon::RESET, &tr!("general-file-name-reset")).clicked() {
                *template = core_engine::DEFAULT_TEMPLATE.to_owned();
            }
        });
        let name = crate::panels::slice::file_name(scene, &machine.slicing, template);
        hint(ui, &tr!("general-file-name-example", name = name));
        let unknown = core_engine::unknown_tags(template);
        if !unknown.is_empty() {
            let tags = unknown
                .iter()
                .map(|tag| format!("{{{tag}}}"))
                .collect::<Vec<_>>();
            hint(
                ui,
                &tr!("general-file-name-unknown", tags = tags.join(", ")),
            );
        }
        describe(ui, &tr!("general-file-name-help"));
    });
}

/// Where the save dialog opens.
#[cfg(not(target_arch = "wasm32"))]
fn sliced_folder(ui: &mut Ui, machine: &mut Machine) {
    use crate::ui::inline_button;

    let general = &mut machine.general;
    form_card(ui, &tr!("general-sliced-files"), |ui| {
        let shown = general
            .sliced_dir
            .as_deref()
            .map_or_else(|| tr!("general-sliced-dir-none"), tilde);
        ui.horizontal(|ui| {
            path_box(ui, &shown, PATH_BUTTONS_W);
            if inline_button(ui, icon::OPEN, &tr!("general-choose-folder"), false, true).clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                general.sliced_dir = Some(dir);
            }
            let forget = tr!("general-forget-folder");
            if general.sliced_dir.is_some() && icon_button(ui, icon::RESET, &forget).clicked() {
                general.sliced_dir = None;
            }
        });
        hint(ui, &tr!("general-sliced-dir-system"));
    });
}

/// Where the profiles are kept, opened in the file manager.
#[cfg(not(target_arch = "wasm32"))]
fn profiles_folder(ui: &mut Ui, machine: &mut Machine) {
    use anyhow::Context as _;

    use crate::ui::inline_button;

    form_card(ui, &tr!("general-profiles"), |ui| {
        let Some(dir) = printer_profiles::user_dir() else {
            hint(ui, &tr!("general-profiles-nowhere"));
            return;
        };
        let shown = dir.display().to_string();
        ui.horizontal(|ui| {
            path_box(ui, &tilde(&dir), PATH_BUTTONS_W);
            if inline_button(ui, icon::OPEN, &tr!("general-open-folder"), false, true).clicked() {
                let opened =
                    reveal(&dir).with_context(|| tr!("error-cannot-open", path = shown.clone()));
                machine
                    .status
                    .report(&tr!("status-opened", path = shown), opened);
            }
        });
        hint(ui, &tr!("general-profiles-dir"));
    });
}

/// A folder as the user knows it, read but not typed into, leaving `buttons` points beside it.
#[cfg(not(target_arch = "wasm32"))]
fn path_box(ui: &mut Ui, path: &str, buttons: f32) {
    let mut text = path.to_owned();
    ui.add(
        crate::ui::text_box(egui::TextEdit::singleline(&mut text))
            .font(crate::ui::theme::typed_path())
            .interactive(false)
            .desired_width((ui.available_width() - buttons).max(0.0)),
    );
}

/// `path` with the home folder written as `~`, as a file manager shows it.
#[cfg(not(target_arch = "wasm32"))]
fn tilde(path: &std::path::Path) -> String {
    let home = std::env::home_dir();
    match home
        .as_deref()
        .and_then(|home| path.strip_prefix(home).ok())
    {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// What a path box leaves for the buttons beside it, points.
#[cfg(not(target_arch = "wasm32"))]
const PATH_BUTTONS_W: f32 = 170.0;

/// What the template's box leaves for Reset beside it, points.
const RESET_W: f32 = 40.0;

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
