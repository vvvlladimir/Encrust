use super::{PAD, open_machines, toggle_settings};
use crate::i18n::tr;
use crate::panels::{Window, frame_view};
use crate::profiles;
use crate::project;
use crate::scene::ObjectId;
use crate::shortcuts::{self, Action};
use crate::state::Doc;
use crate::ui::{MENU_W, MenuRow, icon, menu_row, menu_rule, menu_style, menu_sub, theme};
use egui::containers::menu::MenuConfig;
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
        visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.open.weak_bg_fill = theme::colors().active;
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
            let config = MenuConfig::new().style(menu_style);
            egui::MenuBar::new().config(config).ui(ui, |ui| {
                let [file, edit, view] = menus;
                ui.menu_button(file, |ui| wide(ui, |ui| file_menu(ui, window)));
                ui.menu_button(edit, |ui| wide(ui, |ui| edit_menu(ui, window)));
                ui.menu_button(view, |ui| wide(ui, |ui| view_menu(ui, window)));
            });
        });
    });
    ui.add_space(PAD);
}

fn wide(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.set_width(MENU_W);
    add(ui);
}

/// A row that runs `action`, its keys beside it: the menu is where a shortcut is learned.
fn action_row<'a>(glyph: Option<&'a str>, label: &'a str, action: Action) -> MenuRow<'a> {
    MenuRow::new(glyph, label).keys(shortcuts::text(action))
}

fn clicked(ui: &mut egui::Ui, row: &MenuRow<'_>) -> bool {
    let pressed = menu_row(ui, row).clicked();
    if pressed {
        ui.close();
    }
    pressed
}

/// Create, open, save, slice, set up, leave: one group each. See `docs/design/ui-screens.md`.
pub(super) fn file_menu(ui: &mut egui::Ui, window: &mut Window) {
    if clicked(
        ui,
        &action_row(Some(icon::ADD), &tr!("menu-new-plate"), Action::NewPlate),
    ) {
        project::new_plate(window);
    }
    let new_project = tr!("menu-new-project");
    if clicked(
        ui,
        &action_row(Some(icon::NEW_PROJECT), &new_project, Action::NewProject),
    ) {
        project::new_project(window);
    }
    menu_rule(ui);
    let open = tr!("menu-open-project");
    if clicked(
        ui,
        &action_row(Some(icon::OPEN), &open, Action::OpenProject),
    ) {
        project::open_dialog(window);
    }
    if clicked(
        ui,
        &action_row(
            Some(icon::MODEL),
            &tr!("menu-open-model"),
            Action::OpenModel,
        ),
    ) {
        crate::import::open_dialog(window);
    }
    // A browser hands over bytes and keeps no paths to reopen.
    if cfg!(not(target_arch = "wasm32")) {
        let recent = tr!("menu-open-recent");
        menu_sub(ui, &MenuRow::new(Some(icon::RECENT), &recent), |ui| {
            recent_menu(ui, window);
        });
    }
    let tests = tr!("menu-open-test-print");
    menu_sub(ui, &MenuRow::new(Some(icon::TEST_PRINT), &tests), |ui| {
        test_print_menu(ui, window);
    });
    menu_rule(ui);
    let save = tr!("menu-save-project");
    if clicked(
        ui,
        &action_row(Some(icon::SAVE), &save, Action::SaveProject),
    ) {
        project::save_open(window);
    }
    let save_as = tr!("menu-save-project-as");
    if clicked(ui, &action_row(None, &save_as, Action::SaveProjectAs)) {
        project::save_dialog(window);
    }
    menu_rule(ui);
    slice_items(ui, window);
    menu_rule(ui);
    setup_items(ui, window);
    // A page is left by closing its tab.
    if cfg!(not(target_arch = "wasm32")) {
        menu_rule(ui);
        // macOS binds Quit to Command-Q itself; elsewhere the menu is the way out.
        let keys = if cfg!(target_os = "macos") {
            format!("{}Q", icon::KEY_COMMAND)
        } else {
            String::new()
        };
        if clicked(ui, &MenuRow::new(None, &tr!("menu-quit")).keys(keys)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// Slicing into a file whatever the plate's press is set to, and every plate at once. Off,
/// the row says what stands in the way.
fn slice_items(ui: &mut egui::Ui, window: &mut Window) {
    let slicing = &window.machine.slicing;
    let blocker = slicing
        .blocker(&window.doc.scene)
        .or_else(|| slicing.job.is_some().then(|| tr!("menu-slicing-already")));
    let extension = slicing.format.extension();
    let label = tr!("slice-to", extension = extension);
    let row = action_row(Some(icon::SLICE), &label, Action::Slice).unless(blocker.as_deref());
    if clicked(ui, &row) {
        crate::panels::slice_this_plate(window);
    }
    let plates = window.doc.scene.plates().len();
    if plates > 1 {
        let every = tr!("slice-all", count = plates, extension = extension);
        if clicked(ui, &MenuRow::new(None, &every).enabled(blocker.is_none())) {
            crate::panels::slice_every_plate(window);
        }
    }
}

/// Profiles brought in, the window that keeps them, and Settings.
fn setup_items(ui: &mut egui::Ui, window: &mut Window) {
    let import = tr!("menu-import");
    menu_sub(ui, &MenuRow::new(Some(icon::IMPORT), &import), |ui| {
        if clicked(
            ui,
            &MenuRow::new(Some(icon::PRINTER), &tr!("menu-import-machine")),
        ) {
            profiles::open_printer(window);
        }
        if clicked(
            ui,
            &MenuRow::new(Some(icon::RESIN), &tr!("menu-import-resin")),
        ) {
            profiles::open_material(window.machine);
        }
    });
    let machines = tr!("menu-machine-and-resin");
    if clicked(ui, &MenuRow::new(Some(icon::PRINTER), &machines)) {
        open_machines(window.machine);
    }
    let settings = tr!("menu-settings");
    if clicked(
        ui,
        &action_row(Some(icon::SETTINGS), &settings, Action::Settings),
    ) {
        toggle_settings(window.machine);
    }
}

/// The files opened lately, newest first, each with how long ago, and a way to forget them.
pub(super) fn recent_menu(ui: &mut egui::Ui, window: &mut Window) {
    ui.set_width(RECENT_W);
    let now = crate::updates::now_s();
    let mut picked = None;
    for file in window.machine.recent.files() {
        let when = crate::ui::ago(Some(file.opened_at_s), now);
        let name = file.name();
        let row = MenuRow::new(Some(icon::MODEL), &name).keys(when);
        if menu_row(ui, &row)
            .on_hover_text(file.path.display().to_string())
            .clicked()
        {
            picked = Some(file.path.clone());
        }
    }
    if window.machine.recent.files().is_empty() {
        menu_row(
            ui,
            &MenuRow::new(None, &tr!("menu-nothing-recent")).enabled(false),
        );
        return;
    }
    menu_rule(ui);
    if clicked(ui, &MenuRow::new(None, &tr!("menu-clear-recent"))) {
        window.machine.recent.clear();
    }
    if let Some(path) = picked {
        ui.close();
        crate::panels::open_recent(window, path);
    }
}

/// The test prints drawn for the window, the ones the start page offers.
pub(super) fn test_print_menu(ui: &mut egui::Ui, window: &mut Window) {
    for sample in &crate::samples::SAMPLES {
        let name = sample.name();
        if menu_row(ui, &MenuRow::new(None, &name))
            .on_hover_text(sample.about())
            .clicked()
        {
            ui.close();
            crate::app::open_by_what_it_is(window, sample.handed());
        }
    }
}

/// History, the selection, the plate, and Find.
pub(super) fn edit_menu(ui: &mut egui::Ui, window: &mut Window) {
    let history = &window.doc.history;
    let (can_undo, can_redo) = (history.can_undo(), history.can_redo());
    let (undo, redo) = (tr!("action-undo"), tr!("action-redo"));
    let undo = action_row(Some(icon::UNDO), &undo, Action::Undo).enabled(can_undo);
    if clicked(ui, &undo) {
        window.doc.history.undo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }
    let redo = action_row(Some(icon::REDO), &redo, Action::Redo).enabled(can_redo);
    if clicked(ui, &redo) {
        window.doc.history.redo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }
    menu_rule(ui);
    selection_items(ui, window);
    menu_rule(ui);
    plate_items(ui, window);
    menu_rule(ui);
    if clicked(
        ui,
        &action_row(Some(icon::FIND), &tr!("action-find"), Action::Find),
    ) {
        window.view.options.find = true;
    }
}

/// Every model, none, a copy, gone, and another plate. The first that is off says why.
fn selection_items(ui: &mut egui::Ui, window: &mut Window) {
    let anything = window
        .doc
        .scene
        .has_printable(window.doc.scene.active_plate());
    let all = tr!("action-select-all");
    let all = action_row(Some(icon::SELECT_ALL), &all, Action::SelectAll).enabled(anything);
    if clicked(ui, &all) {
        window.doc.scene.select_here();
    }
    let picked = !window.doc.scene.selection().is_empty();
    let none = tr!("action-deselect");
    let none = action_row(None, &none, Action::Deselect).enabled(picked);
    if clicked(ui, &none) {
        shortcuts::act(window, Action::Deselect);
    }
    let pick_first = tr!("menu-pick-first");
    let why = (!picked).then_some(pick_first.as_str());
    let duplicate = tr!("action-duplicate");
    let duplicate = action_row(Some(icon::DUPLICATE), &duplicate, Action::Duplicate).unless(why);
    if clicked(ui, &duplicate) {
        crate::panels::duplicate_selection(window);
    }
    let remove = tr!("action-remove");
    let remove = action_row(Some(icon::REMOVE), &remove, Action::Remove).enabled(picked);
    if clicked(ui, &remove) {
        shortcuts::act(window, Action::Remove);
    }
    move_to_plate(ui, window.doc);
}

/// Laying the plate out, and turning every model on it.
fn plate_items(ui: &mut egui::Ui, window: &mut Window) {
    let anything = window
        .doc
        .scene
        .has_printable(window.doc.scene.active_plate());
    let empty = tr!("menu-plate-empty");
    let why = (!anything).then_some(empty.as_str());
    let arrange = tr!("menu-arrange-plate");
    if clicked(ui, &MenuRow::new(Some(icon::ARRANGE), &arrange).unless(why)) {
        let gap_mm = window.tools.array.gap_mm;
        window.machine.status =
            crate::arrange::arrange_plate(&mut window.doc.scene, &window.doc.plate, gap_mm);
    }
    let can_orient = window.tools.orient.blocker(&window.doc.scene).is_none()
        && !window.tools.orient.is_running();
    let orient = tr!("menu-auto-orient-plate");
    let orient = MenuRow::new(Some(icon::AUTO_ORIENT), &orient).enabled(can_orient);
    if clicked(ui, &orient) {
        crate::panels::inspector::auto_orient(window, None);
    }
}

/// Sends everything selected to another plate, the one it stands on ticked. Off with
/// nothing selected, and with only one plate to send it to.
pub(super) fn move_to_plate(ui: &mut egui::Ui, doc: &mut Doc) {
    let selected: Vec<ObjectId> = doc.scene.selection().to_vec();
    let plates: Vec<String> = doc.scene.plates().to_vec();
    let active = doc.scene.active_plate();
    let enabled = !selected.is_empty() && plates.len() > 1;

    let mut moved = None;
    let label = tr!("menu-move-to-plate");
    let row = MenuRow::new(Some(icon::MOVE_TO_PLATE), &label).enabled(enabled);
    menu_sub(ui, &row, |ui| {
        for (index, name) in plates.iter().enumerate() {
            let plate = index as u32;
            let here = plate == active;
            let row = MenuRow::new(None, name).ticked(here).enabled(!here);
            if menu_row(ui, &row).clicked() {
                moved = Some(plate);
                ui.close();
            }
        }
    });
    if let Some(plate) = moved {
        for id in selected {
            doc.scene.move_to_plate(id, plate);
        }
    }
}

/// Framing, the camera as a choice of two, what is drawn over the plate, and the sheet.
pub(super) fn view_menu(ui: &mut egui::Ui, window: &mut Window) {
    let frame = tr!("menu-frame-view");
    if clicked(
        ui,
        &action_row(Some(icon::FRAME), &frame, Action::FrameView),
    ) {
        frame_view(
            &window.doc.scene,
            &window.doc.plate,
            &mut window.view.camera,
        );
    }
    menu_rule(ui);
    crate::ui::chooser_group(ui, &tr!("menu-camera"));
    let flat = window.view.camera.orthographic;
    for (orthographic, label) in [
        (false, tr!("menu-camera-perspective")),
        (true, tr!("menu-camera-orthographic")),
    ] {
        if clicked(ui, &MenuRow::new(None, &label).ticked(flat == orthographic)) {
            crate::panels::set_orthographic(window.view, orthographic);
        }
    }
    menu_rule(ui);
    crate::ui::chooser_group(ui, &tr!("menu-show"));
    let options = &mut window.view.options;
    if clicked(
        ui,
        &MenuRow::new(None, &tr!("menu-plate-grid")).ticked(options.grid),
    ) {
        options.grid = !options.grid;
    }
    if clicked(
        ui,
        &MenuRow::new(None, &tr!("menu-xray")).ticked(options.xray),
    ) {
        options.xray = !options.xray;
    }
    menu_rule(ui);
    let sheet = tr!("shortcuts-title");
    if clicked(ui, &action_row(Some(icon::KEYBOARD), &sheet, Action::Sheet)) {
        window.view.options.sheet = true;
    }
}

/// How wide the list of recent files stands at least, points: room for a name and its age.
const RECENT_W: f32 = 250.0;
