//! The General page: how the window behaves, set once and kept in `preferences.json`.

use egui::Ui;

use crate::panels::settings::form_card;
use crate::state::Machine;
use crate::ui::{hint, switch};

pub fn form(ui: &mut Ui, machine: &mut Machine) {
    let general = &mut machine.general;
    form_card(ui, "Start", |ui| {
        switch(ui, &mut general.start_page, "Open on the start page");
        hint(
            ui,
            "The drop zone, the files opened lately and the resin test prints, until a \
             model arrives. Off, a run opens straight on the plate.",
        );
    });
    form_card(ui, "Viewport", |ui| {
        switch(ui, &mut general.invert_zoom, "Scroll down to zoom in");
    });
    #[cfg(not(target_arch = "wasm32"))]
    desk(ui, machine);
}

/// What only a desk has: folders.
#[cfg(not(target_arch = "wasm32"))]
fn desk(ui: &mut Ui, machine: &mut Machine) {
    use anyhow::Context as _;

    use crate::ui::{icon, inline_button, quiet_button};

    let general = &mut machine.general;
    form_card(ui, "Sliced files", |ui| {
        let shown = general.sliced_dir.as_ref().map_or_else(
            || "The save dialog opens wherever the system last left it.".to_owned(),
            |dir| format!("The save dialog opens in {}", dir.display()),
        );
        hint(ui, &shown);
        ui.horizontal(|ui| {
            if inline_button(ui, icon::OPEN, "Choose a folder", false, true).clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                general.sliced_dir = Some(dir);
            }
            let set = general.sliced_dir.is_some();
            if quiet_button(ui, icon::RESET, "Forget it", false, set).clicked() {
                general.sliced_dir = None;
            }
        });
    });
    form_card(ui, "Profiles", |ui| {
        let Some(dir) = printer_profiles::user_dir() else {
            hint(ui, "This system has nowhere to keep your profiles.");
            return;
        };
        hint(
            ui,
            &format!(
                "Your machines, resins and profiles are kept in {}",
                dir.display()
            ),
        );
        if inline_button(ui, icon::OPEN, "Open the folder", false, true).clicked() {
            let opened = reveal(&dir).with_context(|| format!("cannot open {}", dir.display()));
            machine
                .status
                .report(&format!("Opened {}", dir.display()), opened);
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
