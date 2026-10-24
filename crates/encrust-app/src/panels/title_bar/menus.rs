use super::{PAD, open_machines, toggle_settings};
use crate::i18n::tr;
use crate::panels::{Window, frame_view};
use crate::profiles;
use crate::project;
use crate::scene::ObjectId;
use crate::shortcuts::{self, Action};
use crate::state::Doc;
use crate::ui::theme;
use egui::{Align, Layout, vec2};

/// The menu titles, File, Edit and View, in the window's language.
pub(super) fn menus() -> [String; 3] {
    [tr!("menu-file"), tr!("menu-edit"), tr!("menu-view")]
}

/// Points inside a menu title either side of its name, and between two titles.
pub(super) const MENU_PAD: f32 = 8.0;

pub(super) const MENU_GAP: f32 = 4.0;

/// egui draws its own menu bar: there is no native one on the platforms this runs on.
pub(super) fn menu(ui: &mut egui::Ui, window: &mut Window) {
    ui.scope(|ui| {
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.inactive.fg_stroke.color = theme::colors().text_mid;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
        // Room inside and between the titles, so each reads as a button of its own.
        ui.spacing_mut().button_padding.x = MENU_PAD;
        ui.spacing_mut().item_spacing.x = MENU_GAP;

        // A menu bar takes every point it is offered, so it is offered what its titles need.
        let font = egui::TextStyle::Button.resolve(ui.style());
        let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;
        let menus = menus();
        let width: f32 = menus
            .iter()
            .map(|title| {
                let galley =
                    ui.painter()
                        .layout_no_wrap(title.clone(), font.clone(), egui::Color32::WHITE);
                galley.size().x + pad
            })
            .sum();
        let size = vec2(width, ui.available_height());
        ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let [file, edit, view] = menus;
                ui.menu_button(file, |ui| file_menu(ui, window));
                ui.menu_button(edit, |ui| edit_menu(ui, window));
                ui.menu_button(view, |ui| view_menu(ui, window));
            });
        });
    });
    ui.add_space(PAD);
}

/// A menu row that names its key beside itself: the menu is where a shortcut is learned.
pub(super) fn item(ui: &mut egui::Ui, label: &str, action: Action) -> egui::Response {
    let keys = shortcuts::text(action);
    ui.add(egui::Button::new(label).shortcut_text(keys))
}

pub(super) fn item_enabled(
    ui: &mut egui::Ui,
    label: &str,
    action: Action,
    enabled: bool,
) -> egui::Response {
    let keys = shortcuts::text(action);
    ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(keys))
}

pub(super) fn file_menu(ui: &mut egui::Ui, window: &mut Window) {
    if item(ui, &tr!("menu-new-plate"), Action::NewPlate).clicked() {
        ui.close();
        window.doc.scene.add_plate();
    }
    if item(ui, &tr!("menu-new-project"), Action::NewProject).clicked() {
        ui.close();
        project::new_project(window);
    }
    if item(ui, &tr!("menu-open-project"), Action::OpenProject).clicked() {
        ui.close();
        project::open_dialog(window);
    }
    // A browser hands over bytes and keeps no paths to reopen.
    if cfg!(not(target_arch = "wasm32")) {
        ui.menu_button(tr!("menu-open-recent"), |ui| recent_menu(ui, window));
    }
    if item(ui, &tr!("menu-save-project"), Action::SaveProject).clicked() {
        ui.close();
        project::save_open(window);
    }
    if item(ui, &tr!("menu-save-project-as"), Action::SaveProjectAs).clicked() {
        ui.close();
        project::save_dialog(window);
    }
    ui.separator();
    if item(ui, &tr!("menu-open-model"), Action::OpenModel).clicked() {
        ui.close();
        crate::import::open_dialog(window);
    }
    ui.menu_button(tr!("menu-open-test-print"), |ui| {
        test_print_menu(ui, window);
    });
    if ui.button(tr!("menu-import-machine")).clicked() {
        ui.close();
        profiles::open_printer(window);
    }
    if ui.button(tr!("menu-import-resin")).clicked() {
        ui.close();
        profiles::open_material(window.machine);
    }
    ui.separator();
    if ui.button(tr!("menu-machine-and-resin")).clicked() {
        ui.close();
        open_machines(window.machine);
    }
    if item(ui, &tr!("menu-settings"), Action::Settings).clicked() {
        ui.close();
        toggle_settings(window.machine);
    }
    // A page is left by closing its tab.
    if cfg!(not(target_arch = "wasm32")) {
        ui.separator();
        if ui.button(tr!("menu-quit")).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// The files opened lately, newest first, each with how long ago.
pub(super) fn recent_menu(ui: &mut egui::Ui, window: &mut Window) {
    let now = crate::updates::now_s();
    let mut picked = None;
    for file in window.machine.recent.files() {
        let when = crate::ui::ago(Some(file.opened_at_s), now);
        let row = ui.add(egui::Button::new(file.name()).shortcut_text(when));
        if row.on_hover_text(file.path.display().to_string()).clicked() {
            picked = Some(file.path.clone());
        }
    }
    if window.machine.recent.files().is_empty() {
        ui.add_enabled(false, egui::Button::new(tr!("menu-nothing-recent")));
    }
    if let Some(path) = picked {
        ui.close();
        crate::panels::open_recent(window, path);
    }
}

/// The test prints drawn for the window, the ones the start page offers.
pub(super) fn test_print_menu(ui: &mut egui::Ui, window: &mut Window) {
    for sample in &crate::samples::SAMPLES {
        if ui
            .button(sample.name())
            .on_hover_text(sample.about())
            .clicked()
        {
            ui.close();
            crate::app::open_by_what_it_is(window, sample.handed());
        }
    }
}

pub(super) fn edit_menu(ui: &mut egui::Ui, window: &mut Window) {
    let can_undo = window.doc.history.can_undo();
    if item_enabled(ui, &tr!("action-undo"), Action::Undo, can_undo).clicked() {
        ui.close();
        window.doc.history.undo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }
    let can_redo = window.doc.history.can_redo();
    if item_enabled(ui, &tr!("action-redo"), Action::Redo, can_redo).clicked() {
        ui.close();
        window.doc.history.redo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }

    ui.separator();
    if item(ui, &tr!("action-select-all"), Action::SelectAll).clicked() {
        ui.close();
        window.doc.scene.select_here();
    }
    let picked = !window.doc.scene.selection().is_empty();
    if item_enabled(ui, &tr!("action-duplicate"), Action::Duplicate, picked).clicked() {
        ui.close();
        crate::panels::duplicate_selection(window);
    }

    ui.separator();
    move_to_plate(ui, window.doc);

    ui.separator();
    let anything = window
        .doc
        .scene
        .has_printable(window.doc.scene.active_plate());
    if ui
        .add_enabled(anything, egui::Button::new(tr!("menu-arrange-plate")))
        .clicked()
    {
        ui.close();
        let gap_mm = window.tools.array.gap_mm;
        window.machine.status =
            crate::arrange::arrange_plate(&mut window.doc.scene, &window.doc.plate, gap_mm);
    }

    let can_orient = window.tools.orient.blocker(&window.doc.scene).is_none()
        && !window.tools.orient.is_running();
    if ui
        .add_enabled(can_orient, egui::Button::new(tr!("menu-auto-orient-plate")))
        .clicked()
    {
        ui.close();
        let supports = &window.tools.supports;
        let machine = window
            .machine
            .slicing
            .printer
            .as_ref()
            .map(|printer| (printer, &window.machine.slicing.material));
        let started = window.tools.orient.start(
            &mut window.doc.scene,
            &window.doc.plate,
            None,
            |object| supports.built_for(object),
            machine,
            &mut window.machine.status,
        );
        if let Err(error) = started {
            window.machine.status = crate::status::Status::failed(&error);
        }
    }
}

/// Sends everything selected to another plate. Greyed out with nothing selected, and with
/// only one plate to send it to.
pub(super) fn move_to_plate(ui: &mut egui::Ui, doc: &mut Doc) {
    let selected: Vec<ObjectId> = doc.scene.selection().to_vec();
    let plates: Vec<String> = doc.scene.plates().to_vec();
    let active = doc.scene.active_plate();
    let enabled = !selected.is_empty() && plates.len() > 1;

    let mut moved = None;
    ui.add_enabled_ui(enabled, |ui| {
        ui.menu_button(tr!("menu-move-to-plate"), |ui| {
            for (index, name) in plates.iter().enumerate() {
                let plate = index as u32;
                if ui
                    .add_enabled(plate != active, egui::Button::new(name))
                    .clicked()
                {
                    moved = Some(plate);
                    ui.close();
                }
            }
        });
    });
    if let Some(plate) = moved {
        for id in selected {
            doc.scene.move_to_plate(id, plate);
        }
    }
}

pub(super) fn view_menu(ui: &mut egui::Ui, window: &mut Window) {
    if item(ui, &tr!("menu-frame-view"), Action::FrameView).clicked() {
        ui.close();
        frame_view(
            &window.doc.scene,
            &window.doc.plate,
            &mut window.view.camera,
        );
    }
    ui.checkbox(&mut window.view.options.grid, tr!("menu-plate-grid"));
    ui.checkbox(&mut window.view.options.xray, tr!("menu-xray"));
    let mut flat = window.view.camera.orthographic;
    if ui.checkbox(&mut flat, tr!("menu-orthographic")).changed() {
        crate::panels::set_orthographic(window.view, flat);
    }
    ui.separator();
    if item(ui, &tr!("shortcuts-title"), Action::Sheet).clicked() {
        ui.close();
        window.view.options.sheet = true;
    }
}
