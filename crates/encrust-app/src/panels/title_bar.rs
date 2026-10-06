use egui::{Align, Layout, Rect, RichText, UiBuilder, vec2};

use crate::panels::{Window, frame_view};
use crate::profiles;
use crate::project;
use crate::scene::ObjectId;
use crate::settings::Section;
use crate::shortcuts::{self, Action};
use crate::sliced;
use crate::state::{Doc, Machine, View};
use crate::ui::{Segment, Segmented, icon, icon_button, theme};
use crate::updates::Stage;
use crate::workspace::Mode;

/// Room kept clear on macOS for the window buttons the system still draws over the strip.
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS_W: f32 = 68.0;

/// How wide the mode switch is drawn, so it reads the same whatever its labels measure,
/// and the room it leaves above and below itself in the strip.
const MODE_SWITCH_W: f32 = 200.0;
const MODE_SWITCH_MARGIN: f32 = 3.0;

/// The strip is the window's own title bar rather than a band under one; see
/// `docs/decisions/0104`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let strip = ui.max_rect();

    // Interacted with first so that every control drawn after it takes the press instead.
    let drag = ui.interact(
        strip,
        ui.id().with("title-drag"),
        egui::Sense::click_and_drag(),
    );
    if drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        toggle_maximised(ui.ctx());
    }

    // Both ends are laid out in the strip itself rather than in the row each would
    // allocate, so the menus, the gear and the traffic lights share one centre line.
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(strip)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            #[cfg(target_os = "macos")]
            ui.add_space(TRAFFIC_LIGHTS_W);
            menu(ui, window);
            project_name(ui, &window.doc.project);
        },
    );

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(strip)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            window_buttons(ui);
            settings_button(ui, window);
            sheet_button(ui, window.view);
            update_badge(ui, window.machine);
        },
    );

    // Centred on the window rather than on what is left between the two ends, so it stays
    // put while the menus and the project name change.
    let height = strip.height() - 2.0 * MODE_SWITCH_MARGIN;
    ui.scope_builder(
        UiBuilder::new().max_rect(Rect::from_center_size(
            strip.center(),
            vec2(MODE_SWITCH_W, height),
        )),
        |ui| mode_switch(ui, window.mode, height),
    );
}

/// Prepare is the editing half of the application, Preview the reading half.
fn mode_switch(ui: &mut egui::Ui, mode: &mut Mode, height: f32) {
    let segments: Vec<Segment<'_, Mode>> = Mode::ALL
        .iter()
        .map(|mode| Segment::new(*mode, mode.label()))
        .collect();
    Segmented::new(&segments)
        .filled()
        .width(MODE_SWITCH_W)
        .height(height)
        .show(ui, mode);
}

fn toggle_maximised(ctx: &egui::Context) {
    let maximised = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximised));
}

/// macOS keeps drawing its own; every other platform has none, because the strip replaced
/// the system title bar.
#[cfg(target_os = "macos")]
fn window_buttons(_ui: &mut egui::Ui) {}

/// A browser tab is closed by the browser. In its place the page offers the source it runs,
/// at the tag it was built from, as the AGPL asks of a program served over a network.
#[cfg(target_arch = "wasm32")]
fn window_buttons(ui: &mut egui::Ui) {
    let source = concat!(
        env!("CARGO_PKG_REPOSITORY"),
        "/tree/v",
        env!("CARGO_PKG_VERSION")
    );
    ui.add_space(8.0);
    let label = egui::RichText::new("Source")
        .font(theme::small())
        .color(theme::colors().text_low);
    ui.hyperlink_to(label, source)
        .on_hover_text("Encrust is free software under the AGPL-3.0; this is the code it runs");
}

#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
fn window_buttons(ui: &mut egui::Ui) {
    if icon_button(ui, icon::CANCEL, "Close").clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let maximised = ui.input(|input| input.viewport().maximized.unwrap_or(false));
    let tooltip = if maximised { "Restore" } else { "Maximise" };
    if icon_button(ui, icon::MAXIMISE, tooltip).clicked() {
        toggle_maximised(ui.ctx());
    }
    if icon_button(ui, icon::MINIMISE, "Minimise").clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

/// The way into the sheet of keys, beside the gear: an overlay is what an application of
/// this many shortcuts owes the user.
fn sheet_button(ui: &mut egui::Ui, view: &mut View) {
    let tooltip = shortcuts::tooltip(Action::Sheet);
    if icon_button(ui, icon::KEYBOARD, &tooltip).clicked() {
        view.options.sheet = true;
    }
}

/// The way into the Settings screen, at the right end of the strip.
fn settings_button(ui: &mut egui::Ui, window: &mut Window) {
    let open = window.machine.settings.open;
    let tooltip = if open {
        "Back to the plate".to_owned()
    } else {
        shortcuts::tooltip(Action::Settings)
    };
    if icon_button(
        ui,
        if open { icon::CANCEL } else { icon::SETTINGS },
        &tooltip,
    )
    .clicked()
    {
        toggle_settings(window.machine);
    }
}

/// A release newer than this build, offered and never applied: a click opens the Updates
/// page, or restarts into a build already installed. See `crate::updates`.
fn update_badge(ui: &mut egui::Ui, machine: &mut Machine) {
    let Some(offer) = machine.updates.waiting() else {
        return;
    };
    let installed = matches!(machine.updates.stage, Stage::Installed(_));
    let text = if installed {
        "Restart to update".to_owned()
    } else {
        format!("Encrust {}", offer.version)
    };
    let colors = theme::colors();
    let label = ui
        .painter()
        .layout_no_wrap(text, theme::small(), colors.accent);
    let size = vec2(label.size().x + 34.0, theme::TITLE_H - 8.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if response.hovered() {
        colors.accent_wash.gamma_multiply(1.6)
    } else {
        colors.accent_wash
    };
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(255), fill);
    let glyph_at = rect.left_center() + vec2(10.0, 0.0);
    painter.text(
        glyph_at,
        egui::Align2::LEFT_CENTER,
        icon::UPDATE,
        theme::icon(13.0),
        colors.accent,
    );
    let text_at = rect.left_center() + vec2(26.0, -label.size().y / 2.0);
    painter.galley(text_at, label, colors.accent);
    let tooltip = if installed {
        "Close the window and start the new build"
    } else {
        "A new release is out: see what changed"
    };
    if response.on_hover_text(tooltip).clicked() {
        if installed {
            machine.updates.restart();
        } else {
            if !machine.settings.open {
                toggle_settings(machine);
            }
            machine.settings.section = Section::Updates;
        }
    }
    ui.add_space(6.0);
}

/// Opens the Settings screen on the profiles the plate is using, or closes it.
pub fn toggle_settings(machine: &mut Machine) {
    if machine.settings.open {
        machine.settings.open = false;
        return;
    }
    let printer = machine.slicing.printer_id.clone();
    let resin = machine.slicing.resin_id.clone();
    machine.settings.open(
        &machine.slicing.catalogue,
        printer.as_deref(),
        resin.as_deref(),
    );
}

/// What project is open, if one is. The window has no other place to say so.
fn project_name(ui: &mut egui::Ui, opened: &project::Opened) {
    let Some(name) = opened.name() else {
        return;
    };
    ui.add_space(8.0);
    ui.label(
        RichText::new(name)
            .font(theme::label())
            .color(theme::colors().text_low),
    );
}

/// egui draws its own menu bar: there is no native one on the platforms this runs on.
fn menu(ui: &mut egui::Ui, window: &mut Window) {
    ui.scope(|ui| {
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.inactive.fg_stroke.color = theme::colors().text_mid;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.active.bg_stroke = egui::Stroke::NONE;

        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| file_menu(ui, window));
            ui.menu_button("Edit", |ui| edit_menu(ui, window));
            ui.menu_button("View", |ui| view_menu(ui, window));
        });
    });
}

/// A menu row that names its key beside itself: the menu is where a shortcut is learned.
fn item(ui: &mut egui::Ui, label: &str, action: Action) -> egui::Response {
    let keys = shortcuts::text(action);
    ui.add(egui::Button::new(label).shortcut_text(keys))
}

fn item_enabled(ui: &mut egui::Ui, label: &str, action: Action, enabled: bool) -> egui::Response {
    let keys = shortcuts::text(action);
    ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(keys))
}

fn file_menu(ui: &mut egui::Ui, window: &mut Window) {
    if item(ui, "New plate", Action::NewPlate).clicked() {
        ui.close();
        window.doc.scene.add_plate();
    }
    if item(ui, "New project", Action::NewProject).clicked() {
        ui.close();
        project::new_project(window);
    }
    if item(ui, "Open project...", Action::OpenProject).clicked() {
        ui.close();
        project::open_dialog(window);
    }
    if item(ui, "Save project", Action::SaveProject).clicked() {
        ui.close();
        project::save_open(window);
    }
    if item(ui, "Save project as...", Action::SaveProjectAs).clicked() {
        ui.close();
        project::save_dialog(window);
    }
    ui.separator();
    if item(ui, "Open model...", Action::OpenModel).clicked() {
        ui.close();
        window
            .doc
            .imports
            .open_dialog(&window.doc.plate, &mut window.machine.status);
    }
    if ui.button("Open sliced file...").clicked() {
        ui.close();
        sliced::open_dialog(window);
    }
    if window.machine.preview.read_path().is_some() && ui.button("Close sliced file").clicked() {
        ui.close();
        sliced::close(&mut window.machine.preview, &mut window.machine.status);
    }
    if ui.button("Load printer profile...").clicked() {
        ui.close();
        profiles::open_printer(window);
    }
    if ui.button("Load resin profile...").clicked() {
        ui.close();
        profiles::open_material(&mut window.machine.slicing, &mut window.machine.status);
    }
    ui.separator();
    if item(ui, "Settings...", Action::Settings).clicked() {
        ui.close();
        toggle_settings(window.machine);
    }
    // A page is left by closing its tab.
    if cfg!(not(target_arch = "wasm32")) {
        ui.separator();
        if ui.button("Quit").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn edit_menu(ui: &mut egui::Ui, window: &mut Window) {
    let can_undo = window.doc.history.can_undo();
    if item_enabled(ui, "Undo", Action::Undo, can_undo).clicked() {
        ui.close();
        window.doc.history.undo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }
    let can_redo = window.doc.history.can_redo();
    if item_enabled(ui, "Redo", Action::Redo, can_redo).clicked() {
        ui.close();
        window.doc.history.redo(
            &mut window.doc.scene,
            window.tools,
            &mut window.machine.slicing,
        );
    }

    ui.separator();
    if item(ui, "Select all", Action::SelectAll).clicked() {
        ui.close();
        window.doc.scene.select_here();
    }
    let picked = !window.doc.scene.selection().is_empty();
    if item_enabled(ui, "Duplicate", Action::Duplicate, picked).clicked() {
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
        .add_enabled(anything, egui::Button::new("Arrange plate"))
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
        .add_enabled(can_orient, egui::Button::new("Auto-orient plate"))
        .clicked()
    {
        ui.close();
        if let Err(error) = window.tools.orient.start(&window.doc.scene, None) {
            window.machine.status = crate::status::Status::failed(&error);
        }
    }
}

/// Sends everything selected to another plate. Greyed out with nothing selected, and with
/// only one plate to send it to.
fn move_to_plate(ui: &mut egui::Ui, doc: &mut Doc) {
    let selected: Vec<ObjectId> = doc.scene.selection().to_vec();
    let plates: Vec<String> = doc.scene.plates().to_vec();
    let active = doc.scene.active_plate();
    let enabled = !selected.is_empty() && plates.len() > 1;

    let mut moved = None;
    ui.add_enabled_ui(enabled, |ui| {
        ui.menu_button("Move to plate", |ui| {
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

fn view_menu(ui: &mut egui::Ui, window: &mut Window) {
    if item(ui, "Frame view", Action::FrameView).clicked() {
        ui.close();
        frame_view(
            &window.doc.scene,
            &window.doc.plate,
            &mut window.view.camera,
        );
    }
    if item(ui, "Plate contents", Action::PlatePanel).clicked() {
        ui.close();
        window.view.options.plate_panel = !window.view.options.plate_panel;
    }
    ui.checkbox(&mut window.view.options.grid, "Plate grid");
    ui.checkbox(&mut window.view.options.xray, "See through models");
    ui.separator();
    if item(ui, "Keyboard shortcuts", Action::Sheet).clicked() {
        ui.close();
        window.view.options.sheet = true;
    }
}
