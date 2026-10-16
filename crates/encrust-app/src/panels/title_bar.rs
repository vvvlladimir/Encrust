use egui::{Align, Align2, Layout, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

use crate::panels::{Window, frame_view, printer_card};
use crate::profiles;
use crate::project;
use crate::scene::ObjectId;
use crate::settings::Section;
use crate::shortcuts::{self, Action};
use crate::sliced;
use crate::state::{Doc, Machine};
use crate::ui::{focus_ring, icon, icon_button, theme};
use crate::updates::Stage;
use crate::workspace::Tool;

/// The room either side of what a bar control says, and the room kept between the two
/// ends of the bar.
const PAD: f32 = 10.0;
const ENDS_GAP: f32 = 12.0;
/// A bar narrower than this shows the right end's controls as their glyphs alone.
const COMPACT_BAR_W: f32 = 720.0;

/// The bar is the window's own title bar rather than a band under one; see
/// `docs/decisions/0104` and `0217`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let bar = ui.max_rect();

    // Interacted with first so that every control drawn after it takes the press instead.
    // Not focusable: there is nothing to do with the bar from the keyboard.
    let sense = egui::Sense::CLICK | egui::Sense::DRAG;
    let drag = ui.interact(bar, ui.id().with("title-drag"), sense);
    if drag.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        toggle_maximised(ui.ctx());
    }

    // The right end goes first, so the tabs know how far they may run.
    let compact = bar.width() < COMPACT_BAR_W;
    let right = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(bar)
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                window_buttons(ui);
                find_button(ui, window, compact);
                window_controls(ui, window);
                update_badge(ui, window.machine, compact);
                alpha_badge(ui, window.machine, compact);
            },
        )
        .response
        .rect;

    // The menus next, which never give way, then the chip in what is left.
    let menus = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(bar.with_max_x(right.left()))
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                #[cfg(target_os = "macos")]
                ui.add_space(crate::traffic_lights::ROOM);
                menu(ui, window);
            },
        )
        .response
        .rect;
    let room = (menus.right() + ENDS_GAP)..=(right.left() - ENDS_GAP);
    centred_chip(ui, window, bar, room);
}

/// The print chip in the middle of the window, or as near it as `room` lets it stand,
/// its names cut short when even all of `room` is too little.
///
/// It is placed by last frame's width, and a frame whose width changed is drawn again.
fn centred_chip(
    ui: &mut egui::Ui,
    window: &mut Window,
    bar: Rect,
    room: std::ops::RangeInclusive<f32>,
) {
    let id = ui.id().with("print-chip-w");
    let width = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or(0.0);
    let (least, most) = (*room.start(), *room.end());
    let max_w = (most - least).max(0.0);
    let x = (bar.center().x - width / 2.0).min(most - width).max(least);
    let rect = Rect::from_x_y_ranges(x..=x + max_w, bar.y_range());
    let drawn = ui
        .scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                let ring_w = printer_card::ring::ui(ui, window);
                print_chip(ui, window, max_w - ring_w);
            },
        )
        .response
        .rect
        .width();
    if (drawn - width).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, drawn));
        ui.ctx().request_discard("the print chip changed width");
    }
}

/// What belongs to the whole window rather than to the plate: the sheet of keys and the
/// Settings window.
fn window_controls(ui: &mut egui::Ui, window: &mut Window) {
    if icon_button(ui, icon::SETTINGS, &shortcuts::tooltip(Action::Settings)).clicked() {
        toggle_settings(window.machine);
    }
    if icon_button(ui, icon::KEYBOARD, &shortcuts::tooltip(Action::Sheet)).clicked() {
        window.view.options.sheet = true;
    }
}

const MENUS: [&str; 3] = ["File", "Edit", "View"];
/// Points inside a menu title either side of its name, and between two titles.
const MENU_PAD: f32 = 8.0;
const MENU_GAP: f32 = 4.0;

/// egui draws its own menu bar: there is no native one on the platforms this runs on.
fn menu(ui: &mut egui::Ui, window: &mut Window) {
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
/// cut with: they belong to the plate rather than to a tool, so they stand over it. No
/// wider than `max_w`: the two names are cut short to fit, the longer first, and the chip
/// is left out where not even `NAME_LEAST_W` of each would.
fn print_chip(ui: &mut egui::Ui, window: &mut Window, max_w: f32) {
    let colors = theme::colors();
    let slicing = &window.machine.slicing;
    let untuned = slicing.printer_id.is_some() && slicing.has_resin() && !slicing.resin_is_tuned();
    let resin_tint = if untuned {
        colors.warn
    } else {
        colors.text_mid
    };
    let layers = layers_and_exposure(window.machine);
    let names = [
        format!("Machine, {}", window.doc.plate.display_name()),
        format!("Resin, {}", slicing.resin_name()),
        format!("{}, {layers}", Tool::PrintSettings.label()),
    ];
    let slash_w = 10.0;
    let settings = Part::new(
        ui,
        icon::PARAMETERS,
        &layers,
        colors.text_mid,
        "",
        f32::INFINITY,
    );
    let printer_name = window.doc.plate.display_name();
    let names_room =
        max_w - settings.width - slash_w - Part::chrome_w("") - Part::chrome_w(icon::CARET_DOWN);
    if names_room < 2.0 * NAME_LEAST_W {
        return;
    }
    let (printer_w, resin_w) = share_room(
        names_room,
        text_w(ui, printer_name),
        text_w(ui, slicing.resin_name()),
    );
    let parts = [
        Part::new(
            ui,
            icon::PRINTER,
            printer_name,
            colors.text_high,
            "",
            printer_w,
        ),
        Part::new(
            ui,
            icon::RESIN,
            slicing.resin_name(),
            resin_tint,
            icon::CARET_DOWN,
            resin_w,
        ),
        settings,
    ];
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
            let response = ui.interact(*zone, ui.id().with(("print-chip", index)), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &names[index])
            });
            response
        })
        .collect();

    let painter = ui.painter();
    for (index, (part, zone)) in parts.into_iter().zip(zones).enumerate() {
        let lit = index == 2 && open;
        // Square where a part meets the rule between the resin and the settings.
        let mut radius = theme::R_CONTROL;
        match index {
            1 => (radius.ne, radius.se) = (0, 0),
            2 => (radius.nw, radius.sw) = (0, 0),
            _ => {}
        }
        part.paint(painter, zone, radius, lit, responses[index].hovered());
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
    for response in &responses {
        focus_ring(ui, response, theme::R_CONTROL);
    }

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
    /// A part whose text is cut short with an ellipsis past `max_text_w` points.
    fn new(
        ui: &egui::Ui,
        glyph: &'static str,
        text: &str,
        tint: egui::Color32,
        caret: &'static str,
        max_text_w: f32,
    ) -> Self {
        let whole = ui
            .painter()
            .layout_no_wrap(text.to_owned(), theme::label(), tint);
        // Laid out again only when cut: a truncating layout sets even a whole text apart.
        let text = if whole.size().x <= max_text_w {
            whole
        } else {
            let mut job =
                egui::text::LayoutJob::simple_singleline(text.to_owned(), theme::label(), tint);
            job.wrap = egui::text::TextWrapping::truncate_at_width(max_text_w.max(0.0));
            ui.painter().layout_job(job)
        };
        let width = Self::chrome_w(caret) + text.size().x;
        Self {
            glyph,
            text,
            caret,
            width,
        }
    }

    /// The part's width less its text: the room either side, the glyph and the caret.
    fn chrome_w(caret: &str) -> f32 {
        let caret_w = if caret.is_empty() { 0.0 } else { 16.0 };
        PAD + 20.0 + caret_w + PAD
    }

    fn paint(
        self,
        painter: &egui::Painter,
        zone: Rect,
        radius: egui::CornerRadius,
        lit: bool,
        hovered: bool,
    ) {
        let colors = theme::colors();
        if lit {
            painter.rect_filled(zone, radius, colors.raised);
        } else if hovered {
            painter.rect_filled(zone, radius, colors.hover);
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

/// The least of a name the chip shows: enough for a letter and the ellipsis.
const NAME_LEAST_W: f32 = 24.0;

/// The width `text` takes in the chip, uncut.
fn text_w(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), theme::label(), egui::Color32::WHITE)
        .size()
        .x
}

/// Two names' widths cut down to share `room`: the shorter keeps all it needs when it fits
/// in half, the longer takes the rest.
fn share_room(room: f32, first_w: f32, second_w: f32) -> (f32, f32) {
    let room = room.max(0.0);
    let half = room / 2.0;
    if first_w + second_w <= room {
        (first_w, second_w)
    } else if first_w <= half {
        (first_w, room - first_w)
    } else if second_w <= half {
        (room - second_w, second_w)
    } else {
        (half, half)
    }
}

/// Where the search over every tool, field and setting will open. Drawn already, so the
/// bar keeps its shape. TODO(step-5): the palette behind it, with its own ADR.
fn find_button(ui: &mut egui::Ui, window: &mut Window, compact: bool) {
    let colors = theme::colors();
    let tint = colors.text_low;
    let label = ui
        .painter()
        .layout_no_wrap("Find".to_owned(), theme::label(), tint);
    let keys = ui
        .painter()
        .layout_no_wrap(shortcuts::text(Action::Find), theme::small(), tint);
    let width = match compact {
        true => PAD + 12.0 + PAD,
        false => PAD + 18.0 + label.size().x + 16.0 + keys.size().x + PAD,
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, theme::CHIP_H), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Find"));
    let painter = ui.painter();
    let fill = if response.hovered() {
        colors.hover
    } else {
        egui::Color32::TRANSPARENT
    };
    painter.rect(
        rect,
        theme::R_CONTROL,
        fill,
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
    if !compact {
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
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    if response
        .on_hover_text("Find a tool, an action or a model")
        .clicked()
    {
        window.view.options.find = true;
    }
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
fn update_badge(ui: &mut egui::Ui, machine: &mut Machine, compact: bool) {
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
    let response = pill(
        ui,
        icon::UPDATE,
        &text,
        compact,
        colors.accent,
        colors.accent_wash,
    );
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
            machine.settings.turn_to(Section::Updates);
        }
    }
    ui.add_space(6.0);
}

/// What this build is, where it can be acted on: a click opens the sheet that writes a
/// bug report. See `crate::report`.
fn alpha_badge(ui: &mut egui::Ui, machine: &mut Machine, compact: bool) {
    let colors = theme::colors();
    let response = pill(
        ui,
        icon::BUG,
        "Alpha",
        compact,
        colors.warn,
        colors.warn_wash,
    );
    if response
        .on_hover_text("Encrust is in alpha: say what went wrong")
        .clicked()
    {
        machine.report.open = true;
    }
    ui.add_space(6.0);
}

/// A rounded tag in the bar, the one shape both badges are drawn as, its glyph alone when
/// `compact`.
fn pill(
    ui: &mut egui::Ui,
    glyph: &str,
    text: &str,
    compact: bool,
    tint: egui::Color32,
    wash: egui::Color32,
) -> egui::Response {
    let shown = if compact { "" } else { text };
    let label = ui
        .painter()
        .layout_no_wrap(shown.to_owned(), theme::small(), tint);
    let size = vec2(label.size().x + 34.0, theme::CHIP_H - 6.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
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
    focus_ring(ui, &response, egui::CornerRadius::same(255));
    response
}

/// Opens the Settings window, on the first support profile when none is picked yet, or
/// closes it.
pub fn toggle_settings(machine: &mut Machine) {
    let settings = &mut machine.settings;
    if settings.open {
        settings.close();
        return;
    }
    settings.open = true;
    let catalogue = &machine.slicing.catalogue;
    if settings.support.is_none()
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
    // A browser hands over bytes and keeps no paths to reopen.
    if cfg!(not(target_arch = "wasm32")) {
        ui.menu_button("Open recent", |ui| recent_menu(ui, window));
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
    ui.menu_button("Open test print", |ui| test_print_menu(ui, window));
    if ui.button("Open sliced file...").clicked() {
        ui.close();
        sliced::open_dialog(window);
    }
    if window.machine.preview.read_path().is_some() && ui.button("Close sliced file").clicked() {
        ui.close();
        sliced::close(&mut window.machine.preview, &mut window.machine.status);
    }
    if ui.button("Import a machine...").clicked() {
        ui.close();
        profiles::open_printer(window);
    }
    if ui.button("Import a resin...").clicked() {
        ui.close();
        profiles::open_material(window.machine);
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

/// The files opened lately, newest first, each with how long ago.
fn recent_menu(ui: &mut egui::Ui, window: &mut Window) {
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
        ui.add_enabled(false, egui::Button::new("Nothing opened yet"));
    }
    if let Some(path) = picked {
        ui.close();
        crate::panels::open_recent(window, path);
    }
}

/// The test prints drawn for the window, the ones the start page offers.
fn test_print_menu(ui: &mut egui::Ui, window: &mut Window) {
    for sample in &crate::samples::SAMPLES {
        if ui.button(sample.name).on_hover_text(sample.about).clicked() {
            ui.close();
            crate::app::open_by_what_it_is(window, sample.handed());
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

#[cfg(test)]
mod tests {
    use super::share_room;

    #[test]
    fn names_with_room_are_not_cut() {
        assert_eq!(share_room(300.0, 120.0, 90.0), (120.0, 90.0));
    }

    #[test]
    fn the_longer_name_is_cut_first() {
        assert_eq!(share_room(200.0, 160.0, 60.0), (140.0, 60.0));
        assert_eq!(share_room(200.0, 60.0, 160.0), (60.0, 140.0));
    }

    #[test]
    fn two_long_names_share_the_room_evenly() {
        assert_eq!(share_room(200.0, 160.0, 180.0), (100.0, 100.0));
    }

    #[test]
    fn no_room_cuts_both_names_to_nothing() {
        assert_eq!(share_room(-40.0, 160.0, 60.0), (0.0, 0.0));
    }
}
