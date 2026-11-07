mod edit;

use super::{PAD, open_machines, toggle_settings};
use crate::i18n::tr;
use crate::panels::start::remote;
use crate::panels::{Window, frame_view};
use crate::profiles;
use crate::project;
use crate::shortcuts::{self, Action};
use crate::ui::theme::layout::title_bar_menus::RECENT_W;
pub(super) use crate::ui::theme::layout::title_bar_menus::{MENU_GAP, MENU_PAD};
use crate::ui::{MENU_W, MenuRow, icon, menu_row, menu_rule, menu_style, menu_sub, theme};
use edit::edit_menu;
use egui::containers::menu::MenuConfig;
use egui::{Align, Layout, vec2};

/// The menu titles, File, Edit and View, in the window's language.
pub(super) fn menus() -> [String; 3] {
    [tr!("menu-file"), tr!("menu-edit"), tr!("menu-view")]
}

/// egui draws its own menu bar: there is no native one on the platforms this runs on.
pub(super) fn menu(ui: &mut egui::Ui, window: &mut Window) {
    ui.scope(|ui| {
        let colors = theme::colors();
        let visuals = ui.visuals_mut();
        // The titles take the bar's icon squares' inks, which the theme's override would hide.
        visuals.override_text_color = None;
        visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.inactive.fg_stroke.color = colors.text_mid;
        for state in [
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            state.weak_bg_fill = colors.hover;
            state.bg_stroke = egui::Stroke::NONE;
            state.fg_stroke.color = colors.text_high;
        }
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
                crate::ui::menu_button(ui, file, |ui| wide(ui, |ui| file_menu(ui, window)));
                crate::ui::menu_button(ui, edit, |ui| wide(ui, |ui| edit_menu(ui, window)));
                crate::ui::menu_button(ui, view, |ui| wide(ui, |ui| view_menu(ui, window)));
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
    create_items(ui, window);
    menu_rule(ui);
    open_items(ui, window);
    open_subs(ui, window);
    menu_rule(ui);
    save_items(ui, window);
    menu_rule(ui);
    slice_items(ui, window);
    menu_rule(ui);
    setup_items(ui, window);
    // A page is left by closing its tab.
    if cfg!(not(target_arch = "wasm32")) {
        menu_rule(ui);
        quit_row(ui);
    }
}

fn create_items(ui: &mut egui::Ui, window: &mut Window) {
    let (new_plate, new_project) = (tr!("menu-new-plate"), tr!("menu-new-project"));
    run_rows(
        ui,
        window,
        [
            (
                Some(icon::ADD),
                new_plate,
                Action::NewPlate,
                project::new_plate,
            ),
            (
                Some(icon::NEW_PROJECT),
                new_project,
                Action::NewProject,
                project::new_project,
            ),
        ],
    );
}

fn open_items(ui: &mut egui::Ui, window: &mut Window) {
    let (open_project, open_model) = (tr!("menu-open-project"), tr!("menu-open-model"));
    run_rows(
        ui,
        window,
        [
            (
                Some(icon::OPEN),
                open_project,
                Action::OpenProject,
                project::open_dialog,
            ),
            (
                Some(icon::MODEL),
                open_model,
                Action::OpenModel,
                crate::import::open_dialog,
            ),
        ],
    );
}

fn save_items(ui: &mut egui::Ui, window: &mut Window) {
    let save = tr!("menu-save-project");
    let save_as = tr!("menu-save-project-as");
    run_rows(
        ui,
        window,
        [
            (Some(icon::SAVE), save, Action::SaveProject, |window| {
                project::save_open(window);
            }),
            (None, save_as, Action::SaveProjectAs, |window| {
                project::save_dialog(window);
            }),
        ],
    );
}

/// A menu row's glyph, name and action, and what it runs.
type RunRow<'a> = (Option<&'a str>, String, Action, fn(&mut Window));

/// Rows that each run an action with its keys beside it.
fn run_rows<const N: usize>(ui: &mut egui::Ui, window: &mut Window, rows: [RunRow<'_>; N]) {
    for (glyph, label, action, run) in rows {
        if clicked(ui, &action_row(glyph, &label, action)) {
            run(window);
        }
    }
}

/// The recent files and the test prints, each a submenu.
fn open_subs(ui: &mut egui::Ui, window: &mut Window) {
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
}

fn quit_row(ui: &mut egui::Ui) {
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

/// The test prints the start page offers: those kept in the repository, each saying whether
/// it is fetched, then those drawn for the window.
pub(super) fn test_print_menu(ui: &mut egui::Ui, window: &mut Window) {
    for (index, sample) in crate::samples::remote::REMOTES.iter().enumerate() {
        let name = sample.name();
        let state = remote::state(&window.machine.fetches, index);
        if menu_row(ui, &MenuRow::new(None, &name).keys(state))
            .on_hover_text(sample.about())
            .clicked()
        {
            ui.close();
            remote::picked(ui, window, index);
        }
    }
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
    if clicked(
        ui,
        &MenuRow::new(None, &tr!("menu-render")).ticked(options.render),
    ) {
        options.render = !options.render;
    }
    if clicked(
        ui,
        &MenuRow::new(None, &tr!("menu-depth-of-field")).ticked(options.depth_of_field),
    ) {
        options.depth_of_field = !options.depth_of_field;
    }
    menu_rule(ui);
    let sheet = tr!("shortcuts-title");
    if clicked(ui, &action_row(Some(icon::KEYBOARD), &sheet, Action::Sheet)) {
        window.view.options.sheet = true;
    }
}
