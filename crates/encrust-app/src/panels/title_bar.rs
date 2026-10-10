use egui::{Align, Align2, Layout, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

use crate::panels::{Window, frame_view};
use crate::profiles;
use crate::project;
use crate::scene::ObjectId;
use crate::settings::Section;
use crate::shortcuts::{self, Action};
use crate::sliced;
use crate::state::{Doc, Machine};
use crate::ui::{icon, icon_button, theme};
use crate::updates::Stage;
use crate::workspace::Tool;

/// The room either side of what a bar control says, and the room kept between the two
/// ends of the bar.
const PAD: f32 = 10.0;
const ENDS_GAP: f32 = 12.0;

/// The bar is the window's own title bar rather than a band under one; see
/// `docs/decisions/0104` and `0217`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let bar = ui.max_rect();

    // Interacted with first so that every control drawn after it takes the press instead.
    let drag = ui.interact(
        bar,
        ui.id().with("title-drag"),
        egui::Sense::click_and_drag(),
    );
    if drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        toggle_maximised(ui.ctx());
    }

    // The right end goes first, so the tabs know how far they may run.
    let right = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(bar)
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                window_buttons(ui);
                window_controls(ui, window);
                find_button(ui);
                if !window.machine.settings.open {
                    print_chip(ui, window);
                }
                update_badge(ui, window.machine);
                alpha_badge(ui, window.machine);
            },
        )
        .response
        .rect;

    let left = bar.with_max_x((right.left() - ENDS_GAP).max(bar.left()));
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(left)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            #[cfg(target_os = "macos")]
            ui.add_space(crate::traffic_lights::ROOM);
            menu(ui, window);
            project_name(ui, &window.doc.project);
        },
    );
}

/// What belongs to the whole window rather than to the plate: the sheet of keys and the
/// Settings screen.
fn window_controls(ui: &mut egui::Ui, window: &mut Window) {
    if icon_button(ui, icon::SETTINGS, &shortcuts::tooltip(Action::Settings)).clicked() {
        toggle_settings(window.machine);
    }
    if icon_button(ui, icon::KEYBOARD, &shortcuts::tooltip(Action::Sheet)).clicked() {
        window.view.options.sheet = true;
    }
}

const MENUS: [&str; 3] = ["File", "Edit", "View"];

/// egui draws its own menu bar: there is no native one on the platforms this runs on.
fn menu(ui: &mut egui::Ui, window: &mut Window) {
    ui.scope(|ui| {
        let visuals = ui.visuals_mut();
        visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.inactive.fg_stroke.color = theme::colors().text_mid;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.active.bg_stroke = egui::Stroke::NONE;

        // A menu bar takes every point it is offered, so it is offered what its titles need.
        let font = egui::TextStyle::Button.resolve(ui.style());
        let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;
        let width: f32 = MENUS
            .iter()
            .map(|title| {
                let galley = ui.painter().layout_no_wrap(
                    (*title).to_owned(),
                    font.clone(),
                    egui::Color32::WHITE,
                );
                galley.size().x + pad
            })
            .sum();
        let size = vec2(width, ui.available_height());
        ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(MENUS[0], |ui| file_menu(ui, window));
                ui.menu_button(MENUS[1], |ui| edit_menu(ui, window));
                ui.menu_button(MENUS[2], |ui| view_menu(ui, window));
            });
        });
    });
    ui.add_space(PAD);
}

/// The machine and the resin the plate is printed on, and the layers and exposure it is
/// cut with: they belong to the plate rather than to a tool, so they stand over it.
fn print_chip(ui: &mut egui::Ui, window: &mut Window) {
    let colors = theme::colors();
    let slicing = &window.machine.slicing;
    let untuned = slicing.printer_id.is_some() && slicing.has_resin() && !slicing.resin_is_tuned();
    let resin_tint = if untuned {
        colors.warn
    } else {
        colors.text_mid
    };
    let layers = layers_and_exposure(window.machine);
    let parts = [
        Part::new(
            ui,
            icon::PRINTER,
            window.doc.plate.display_name(),
            colors.text_high,
            "",
        ),
        Part::new(
            ui,
            icon::RESIN,
            slicing.resin_name(),
            resin_tint,
            icon::CARET_DOWN,
        ),
        Part::new(ui, icon::PARAMETERS, &layers, colors.text_mid, ""),
    ];
    let slash_w = 10.0;
    let width: f32 = parts.iter().map(|part| part.width).sum::<f32>() + slash_w;
    let (rect, _) = ui.allocate_exact_size(vec2(width, theme::CHIP_H), Sense::hover());

    let mut x = rect.left();
    let mut zones = [Rect::NOTHING; 3];
    for (index, part) in parts.iter().enumerate() {
        zones[index] = Rect::from_x_y_ranges(x..=x + part.width, rect.y_range());
        x += part.width + if index == 0 { slash_w } else { 0.0 };
    }
    let open = *window.tool == Tool::PrintSettings;
    let responses: Vec<egui::Response> = zones
        .iter()
        .enumerate()
        .map(|(index, zone)| {
            ui.interact(*zone, ui.id().with(("print-chip", index)), Sense::click())
        })
        .collect();

    let painter = ui.painter();
    for (index, (part, zone)) in parts.into_iter().zip(zones).enumerate() {
        let lit = index == 2 && open;
        part.paint(painter, zone, lit, responses[index].hovered());
    }
    painter.text(
        pos2(zones[0].right() + slash_w / 2.0, rect.center().y),
        Align2::CENTER_CENTER,
        "/",
        theme::label(),
        colors.text_low,
    );
    painter.vline(
        zones[2].left(),
        rect.y_range(),
        Stroke::new(1.0, colors.hairline),
    );
    painter.rect_stroke(
        rect,
        theme::R_CONTROL,
        Stroke::new(1.0, colors.hairline),
        egui::StrokeKind::Inside,
    );

    let [printer, resin, layers] = <[egui::Response; 3]>::try_from(responses)
        .unwrap_or_else(|_| unreachable!("three zones, three responses"));
    let printer = printer.on_hover_text("The machine this plate is printed on");
    egui::Popup::menu(&printer).show(|ui| profiles::printer_menu(ui, window));
    let resin = if untuned {
        resin.on_hover_text("The resin, not measured on this printer")
    } else {
        resin.on_hover_text("The resin")
    };
    egui::Popup::menu(&resin).show(|ui| profiles::resin_menu(ui, window.machine));
    if layers.on_hover_text(Tool::PrintSettings.label()).clicked() {
        shortcuts::pick(window, Tool::PrintSettings);
    }
}

/// The layer height and the exposure a plate is cut with, as the chip and the print
/// settings' heading both read them.
pub fn layers_and_exposure(machine: &Machine) -> String {
    format!(
        "{:.0} \u{b5}m, {:.2} s",
        machine.slicing.layer_height_mm() * 1000.0,
        machine.slicing.material.exposure_s
    )
}

/// One stretch of the print chip: a glyph, what is chosen, and a caret where it opens a
/// list.
struct Part {
    glyph: &'static str,
    text: std::sync::Arc<egui::Galley>,
    caret: &'static str,
    width: f32,
}

impl Part {
    fn new(
        ui: &egui::Ui,
        glyph: &'static str,
        text: &str,
        tint: egui::Color32,
        caret: &'static str,
    ) -> Self {
        let text = ui
            .painter()
            .layout_no_wrap(text.to_owned(), theme::label(), tint);
        let caret_w = if caret.is_empty() { 0.0 } else { 16.0 };
        let width = PAD + 20.0 + text.size().x + caret_w + PAD;
        Self {
            glyph,
            text,
            caret,
            width,
        }
    }

    fn paint(self, painter: &egui::Painter, zone: Rect, lit: bool, hovered: bool) {
        let colors = theme::colors();
        if lit {
            painter.rect_filled(zone, theme::R_CONTROL, colors.raised);
        } else if hovered {
            painter.rect_filled(zone, theme::R_CONTROL, colors.hover);
        }
        let glyph_tint = if lit {
            colors.accent_soft
        } else {
            colors.text_low
        };
        painter.text(
            pos2(zone.left() + PAD + 7.0, zone.center().y),
            Align2::CENTER_CENTER,
            self.glyph,
            theme::icon(14.0),
            glyph_tint,
        );
        let at = pos2(
            zone.left() + PAD + 20.0,
            zone.center().y - self.text.size().y / 2.0,
        );
        let text_w = self.text.size().x;
        let fallback = if lit {
            colors.accent_soft
        } else {
            colors.text_high
        };
        painter.galley(at, self.text, fallback);
        if !self.caret.is_empty() {
            painter.text(
                pos2(at.x + text_w + 9.0, zone.center().y),
                Align2::CENTER_CENTER,
                self.caret,
                theme::icon(11.0),
                colors.text_low,
            );
        }
    }
}

/// Where the search over every tool, field and setting will open. Drawn already, so the
/// bar keeps its shape. TODO(step-5): the palette behind it, with its own ADR.
fn find_button(ui: &mut egui::Ui) {
    let colors = theme::colors();
    let tint = colors.text_low.gamma_multiply(0.6);
    let label = ui
        .painter()
        .layout_no_wrap("Find".to_owned(), theme::label(), tint);
    let keys = ui
        .painter()
        .layout_no_wrap("\u{2318}K".to_owned(), theme::small(), tint);
    let width = PAD + 18.0 + label.size().x + 16.0 + keys.size().x + PAD;
    let (rect, response) = ui.allocate_exact_size(vec2(width, theme::CHIP_H), Sense::hover());
    let painter = ui.painter();
    painter.rect_stroke(
        rect,
        theme::R_CONTROL,
        Stroke::new(1.0, colors.hairline),
        egui::StrokeKind::Inside,
    );
    painter.text(
        pos2(rect.left() + PAD + 6.0, rect.center().y),
        Align2::CENTER_CENTER,
        icon::FIND,
        theme::icon(13.0),
        tint,
    );
    let label_w = label.size().x;
    painter.galley(
        pos2(
            rect.left() + PAD + 18.0,
            rect.center().y - label.size().y / 2.0,
        ),
        label,
        tint,
    );
    painter.galley(
        pos2(
            rect.left() + PAD + 18.0 + label_w + 16.0,
            rect.center().y - keys.size().y / 2.0,
        ),
        keys,
        tint,
    );
    response.on_hover_text("Finding any tool, field or setting is still to come");
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
    if crate::ui::icon_button(ui, icon::CANCEL, "Close").clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let maximised = ui.input(|input| input.viewport().maximized.unwrap_or(false));
    let tooltip = if maximised { "Restore" } else { "Maximise" };
    if crate::ui::icon_button(ui, icon::MAXIMISE, tooltip).clicked() {
        toggle_maximised(ui.ctx());
    }
    if crate::ui::icon_button(ui, icon::MINIMISE, "Minimise").clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
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
    let response = pill(ui, icon::UPDATE, &text, colors.accent, colors.accent_wash);
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

/// What this build is, where it can be acted on: a click opens the sheet that writes a
/// bug report. See `crate::report`.
fn alpha_badge(ui: &mut egui::Ui, machine: &mut Machine) {
    let colors = theme::colors();
    let response = pill(ui, icon::BUG, "Alpha", colors.warn, colors.warn_wash);
    if response
        .on_hover_text("Encrust is in alpha: say what went wrong")
        .clicked()
    {
        machine.report.open = true;
    }
    ui.add_space(6.0);
}

/// A rounded tag in the bar, the one shape both badges are drawn as.
fn pill(
    ui: &mut egui::Ui,
    glyph: &str,
    text: &str,
    tint: egui::Color32,
    wash: egui::Color32,
) -> egui::Response {
    let label = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::small(), tint);
    let size = vec2(label.size().x + 34.0, theme::CHIP_H - 6.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let fill = if response.hovered() {
        wash.gamma_multiply(1.6)
    } else {
        wash
    };
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(255), fill);
    painter.text(
        rect.left_center() + vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        glyph,
        theme::icon(13.0),
        tint,
    );
    painter.galley(
        rect.left_center() + vec2(26.0, -label.size().y / 2.0),
        label,
        tint,
    );
    response
}

/// Opens the Settings screen, on the first support profile when none is open yet, or
/// closes it.
pub fn toggle_settings(machine: &mut Machine) {
    let settings = &mut machine.settings;
    settings.open = !settings.open;
    let catalogue = &machine.slicing.catalogue;
    if settings.open
        && settings.support.is_none()
        && let Some(first) = catalogue.supports().next()
    {
        let id = first.id.clone();
        settings.pick_support(catalogue, &id);
    }
}

/// Opens the Machine and resin window on the machine the plate is printed on.
pub fn open_machines(machine: &mut Machine) {
    let printer = machine.slicing.printer_id.clone();
    machine
        .settings
        .open_machines(&machine.slicing.catalogue, printer.as_deref(), None);
}

/// What project is open, if one is. The window has no other place to say so.
fn project_name(ui: &mut egui::Ui, opened: &project::Opened) {
    let Some(name) = opened.name() else {
        return;
    };
    ui.add_space(PAD);
    ui.label(
        egui::RichText::new(name)
            .font(theme::label())
            .color(theme::colors().text_low),
    );
    ui.add_space(PAD);
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
    if ui.button("Machine and resin...").clicked() {
        ui.close();
        open_machines(window.machine);
    }
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
    let mut flat = window.view.camera.orthographic;
    if ui.checkbox(&mut flat, "Orthographic view").changed() {
        crate::panels::set_orthographic(window.view, flat);
    }
    ui.separator();
    if item(ui, "Keyboard shortcuts", Action::Sheet).clicked() {
        ui.close();
        window.view.options.sheet = true;
    }
}
