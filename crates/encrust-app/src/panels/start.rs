//! The start page: what the window shows under the top bar until the first model, project
//! or sliced file arrives. A place to drop a model, the machine it is printed on, and the
//! files opened lately; see `docs/decisions/0220`.

use egui::{Align, Align2, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use crate::files::{self, Handed, Wanted};
use crate::panels::Window;
use crate::ui::{ago, describe, icon, inline_button, theme, tone, two_lines};
use crate::workspace::Tool;

/// The page's column, the share of it the drop zone takes, and the gap between the cards.
const COLUMN_W: f32 = 980.0;
const DROP_SHARE: f32 = 0.575;
const GAP: f32 = 32.0;
/// The drop zone's height, the room inside a card's border, and the dashes of the zone's.
const DROP_H: f32 = 340.0;
const CARD_PAD: i8 = 20;
const DASH: f32 = 4.0;
/// How many recent files the page lists.
const RECENT_SHOWN: usize = 5;

/// Whether the window is still on its start page. It leaves for good the moment anything
/// is on the plate or under the layer strip, or a tool is picked.
pub fn still_starting(window: &mut Window) -> bool {
    let quiet = window.doc.scene.is_empty()
        && window.doc.project.path.is_none()
        && window.machine.preview.read_path().is_none()
        && !window.doc.imports.is_busy()
        && *window.tool == Tool::default();
    if !quiet {
        window.view.options.started = true;
    }
    !window.view.options.started
}

pub fn ui(ui: &mut Ui, window: &mut Window) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = COLUMN_W.min(ui.available_width() - 2.0 * GAP);
            let top = ui.max_rect().top() + 64.0;
            let left = ui.max_rect().center().x - width / 2.0;
            let column = Rect::from_min_max(pos2(left, top), pos2(left + width, f32::INFINITY));
            let mut page = ui.new_child(
                UiBuilder::new()
                    .max_rect(column)
                    .layout(Layout::top_down(Align::Min)),
            );
            heading(&mut page);
            // The stage's notice is not on this page, so a failure is said here instead.
            if window.machine.status.is_error() {
                page.add_space(8.0);
                tone(
                    &mut page,
                    window.machine.status.text(),
                    theme::colors().danger,
                );
            }
            page.add_space(28.0);
            cards(&mut page, window, width);
            let used = page.min_rect().bottom() - ui.max_rect().top() + 48.0;
            ui.allocate_space(vec2(ui.available_width(), used));
        });
}

fn heading(ui: &mut Ui) {
    let colors = theme::colors();
    ui.label(
        RichText::new("Start a plate")
            .font(theme::page_title(26.0))
            .color(colors.text_high),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new("Choose your machine, then open a model.")
            .font(theme::lede())
            .color(colors.text_mid),
    );
}

/// The drop zone on the left, the machine and the recent files stacked on the right.
fn cards(ui: &mut Ui, window: &mut Window, width: f32) {
    let drop_w = ((width - GAP) * DROP_SHARE).floor();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        drop_zone(ui, window, vec2(drop_w, DROP_H));
        ui.vertical(|ui| {
            ui.set_width(width - drop_w - GAP);
            ui.spacing_mut().item_spacing = vec2(theme::ITEM_GAP, 16.0);
            card(ui, |ui| machine(ui, window));
            card(ui, |ui| recent(ui, window));
        });
    });
}

/// A bordered card the width of its column.
fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, theme::colors().line))
        .corner_radius(theme::R_PANEL)
        .inner_margin(egui::Margin::same(CARD_PAD))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
            add(ui);
        });
}

/// Where a file dropped on the window goes, said where the user is looking. A click on it
/// asks for a model or a project.
fn drop_zone(ui: &mut Ui, window: &mut Window, size: egui::Vec2) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    paint_zone(ui.painter(), rect, response.hovered());
    let buttons = Rect::from_center_size(
        rect.center() + vec2(0.0, 52.0),
        vec2(size.x, theme::BUTTON_H),
    );
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(buttons)
            .layout(Layout::left_to_right(Align::Center)),
    );
    row.spacing_mut().item_spacing.x = theme::ITEM_GAP;
    let (model, project) = buttons_row(&mut row);
    let wanted = match (model, project, response.clicked()) {
        (true, _, _) => Some(Wanted::Model),
        (_, true, _) => Some(Wanted::Project),
        (_, _, true) => Some(Wanted::ModelOrProject),
        _ => None,
    };
    if let Some(file) = wanted.and_then(files::pick) {
        crate::app::open_by_what_it_is(window, file);
    }
}

/// The zone's dashed edge and what it says, lit in the accent under the pointer.
fn paint_zone(painter: &egui::Painter, rect: Rect, hovered: bool) {
    let colors = theme::colors();
    let (fill, edge, glyph) = match hovered {
        true => (colors.accent_wash, colors.accent_deep, colors.accent_soft),
        false => (colors.base, colors.line, colors.text_low),
    };
    painter.rect_filled(rect, theme::R_PANEL, fill);
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    let edge = Stroke::new(1.0, edge);
    painter.extend(egui::Shape::dashed_line(&corners, edge, DASH, DASH));
    let lines = [
        (-72.0, icon::DROP_HERE, theme::icon(28.0), glyph),
        (
            -28.0,
            "Drop a model here",
            theme::page_title(18.0),
            colors.text_high,
        ),
        (
            2.0,
            "STL, OBJ, 3MF, or an .encrust project",
            theme::small(),
            colors.text_low,
        ),
    ];
    for (dy, text, font, tint) in lines {
        let at = rect.center() + vec2(0.0, dy);
        painter.text(at, Align2::CENTER_CENTER, text, font, tint);
    }
}

/// The two buttons of the zone, centred in `row`. Answers which was pressed.
fn buttons_row(row: &mut Ui) -> (bool, bool) {
    let colors = theme::colors();
    let width = |text: &str| {
        row.painter()
            .layout_no_wrap(text.to_owned(), theme::label(), colors.text_high)
            .size()
            .x
            + 32.0
    };
    let both = width("Open a model") + width("Open a project") + row.spacing().item_spacing.x;
    row.add_space((row.available_width() - both) / 2.0);
    let model = inline_button(row, "", "Open a model", true, true).clicked();
    let project = inline_button(row, "", "Open a project", false, true).clicked();
    (model, project)
}

/// The machine the plate is printed on, or the warning that there is none yet. Either
/// opens the Machine and resin window.
fn machine(ui: &mut Ui, window: &mut Window) {
    let colors = theme::colors();
    ui.label(
        RichText::new("Your machine")
            .font(theme::block_title())
            .color(colors.text_high),
    );
    ui.add_space(10.0);
    let slicing = &window.machine.slicing;
    let (title, detail, tint) = match slicing.printer.as_ref() {
        Some(printer) => {
            let format = core_pipeline::SlicedFormat::from(printer.output).extension();
            let detail = format!("{}, .{format}", printer.manufacturer);
            (printer.name.clone(), detail, colors.text_mid)
        }
        None => {
            let (machines, makers) = crate::settings::library_size(&slicing.catalogue);
            let detail = format!("{machines} profiles from {makers} makers");
            ("Pick your machine".to_owned(), detail, colors.warn)
        }
    };
    if machine_button(ui, &title, &detail, tint).clicked() {
        crate::panels::open_machines(window.machine);
    }
    ui.add_space(10.0);
    describe(
        ui,
        "The plate and every figure use this machine's build volume and pixel size.",
    );
}

/// The machine as one wide button: its glyph, its name over its detail, and a caret.
fn machine_button(ui: &mut Ui, title: &str, detail: &str, tint: egui::Color32) -> egui::Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::TWO_LINE_ROW_H + 8.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.hovered() {
        colors.hover
    } else {
        colors.raised
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        theme::R_CONTROL,
        fill,
        Stroke::new(1.0, tint.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    let left = rect.left_center() + vec2(14.0, 0.0);
    painter.text(
        left,
        Align2::LEFT_CENTER,
        icon::PRINTER,
        theme::icon(20.0),
        tint,
    );
    two_lines(painter, left + vec2(34.0, 0.0), title, detail);
    let caret = rect.right_center() - vec2(14.0, 0.0);
    painter.text(
        caret,
        Align2::RIGHT_CENTER,
        icon::CARET_DOWN,
        theme::icon(13.0),
        colors.text_low,
    );
    response
}

/// The files opened lately, each opened again by a click. One that has gone is dropped
/// from the list and said so.
fn recent(ui: &mut Ui, window: &mut Window) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(icon::RECENT)
                .font(theme::icon(15.0))
                .color(colors.text_low),
        );
        ui.label(
            RichText::new("Recent")
                .font(theme::block_title())
                .color(colors.text_high),
        );
    });
    ui.add_space(6.0);
    let files = window.machine.recent.files();
    if files.is_empty() {
        describe(
            ui,
            "Nothing opened yet. A model or a project you open is listed here.",
        );
        return;
    }
    let now = crate::updates::now_s();
    let mut picked = None;
    for file in files.iter().take(RECENT_SHOWN) {
        if recent_row(ui, &file.name(), &ago(Some(file.opened_at_s), now))
            .on_hover_text(file.path.display().to_string())
            .clicked()
        {
            picked = Some(file.path.clone());
        }
    }
    let Some(path) = picked else {
        return;
    };
    if path.exists() {
        crate::app::open_by_what_it_is(window, Handed::Path(path));
    } else {
        window.machine.recent.forget(&path);
        window.machine.status =
            crate::status::Status::Error(format!("{} is no longer there", path.display()));
    }
}

fn recent_row(ui: &mut Ui, name: &str, when: &str) -> egui::Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, theme::R_CONTROL, colors.raised);
    }
    let inner = rect.shrink2(vec2(10.0, 0.0));
    let painter = ui.painter_at(rect);
    painter.text(
        inner.right_center(),
        Align2::RIGHT_CENTER,
        when,
        theme::small(),
        colors.text_low,
    );
    let mut job =
        egui::text::LayoutJob::simple_singleline(name.to_owned(), theme::body(), colors.text_high);
    job.wrap = egui::text::TextWrapping::truncate_at_width(inner.width() - 96.0);
    let galley = painter.layout_job(job);
    painter.galley(
        pos2(inner.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        colors.text_high,
    );
    response
}
