//! The start page: what the window shows under the top bar until the first model, project
//! or sliced file arrives. A place to drop a model, the files opened lately, and the test
//! prints bundled with the window; see `docs/decisions/0220` and `0232`.

use egui::{Align, Align2, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use crate::files::{self, Handed, Wanted};
use crate::panels::Window;
use crate::samples::{SAMPLES, Sample};
use crate::ui::{ago, describe, focus_ring, icon, inline_button, theme, tone};
use crate::workspace::Tool;

/// The page's column, the share of it the drop zone takes, and the gap between the cards.
const COLUMN_W: f32 = 980.0;
const DROP_SHARE: f32 = 0.575;
const GAP: f32 = 32.0;
/// The drop zone's height, the room inside a card's border, and the dashes of the zone's.
const DROP_H: f32 = 300.0;
const CARD_PAD: i8 = 20;
const DASH: f32 = 4.0;
/// How many recent files the page lists.
const RECENT_SHOWN: usize = 5;
/// A test print's card, the gap between two, and its picture's height inside it.
const SAMPLE_H: f32 = 186.0;
const SAMPLE_GAP: f32 = 16.0;
const PREVIEW_H: f32 = 92.0;

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
            let top = ui.max_rect().top() + 40.0;
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
            page.add_space(28.0);
            samples(&mut page, window, width);
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
        RichText::new("Open a model, or begin with a test print.")
            .font(theme::lede())
            .color(colors.text_mid),
    );
}

/// The drop zone on the left, the recent files beside it at the zone's height.
fn cards(ui: &mut Ui, window: &mut Window, width: f32) {
    let drop_w = ((width - GAP) * DROP_SHARE).floor();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        drop_zone(ui, window, vec2(drop_w, DROP_H));
        ui.vertical(|ui| {
            ui.set_width(width - drop_w - GAP);
            card(ui, DROP_H, |ui| recent(ui, window));
        });
    });
}

/// A bordered card the width of its column and `height` tall, border included.
fn card(ui: &mut Ui, height: f32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, theme::colors().line))
        .corner_radius(theme::R_PANEL)
        .inner_margin(egui::Margin::same(CARD_PAD))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(height - 2.0 * (f32::from(CARD_PAD) + 1.0));
            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
            add(ui);
        });
}

/// Where a file dropped on the window goes, said where the user is looking. A click on it
/// asks for a model or a project.
fn drop_zone(ui: &mut Ui, window: &mut Window, size: egui::Vec2) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Open a model or a project")
    });
    paint_zone(ui.painter(), rect, response.hovered());
    focus_ring(ui, &response, theme::R_SURFACE);
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
    if let Some(path) = picked {
        open_recent(window, path);
    }
}

/// Opens a file from the recent list again, or drops it from the list and says so if it has
/// gone since.
pub fn open_recent(window: &mut Window, path: std::path::PathBuf) {
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
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    focus_ring(ui, &response, theme::R_CONTROL);
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

/// The bundled test prints in a row, each put on the plate by a click.
fn samples(ui: &mut Ui, window: &mut Window, width: f32) {
    let colors = theme::colors();
    ui.label(
        RichText::new("Test prints")
            .font(theme::block_title())
            .color(colors.text_high),
    );
    ui.add_space(4.0);
    describe(
        ui,
        "Resin tests drawn for Encrust, to try out a machine or a resin on.",
    );
    ui.add_space(12.0);
    let count = SAMPLES.len() as f32;
    let card_w = ((width - SAMPLE_GAP * (count - 1.0)) / count).floor();
    let mut picked = None;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = SAMPLE_GAP;
        for (index, sample) in SAMPLES.iter().enumerate() {
            if sample_card(ui, index, sample, vec2(card_w, SAMPLE_H)).clicked() {
                picked = Some(sample);
            }
        }
    });
    if let Some(sample) = picked {
        crate::app::open_by_what_it_is(window, sample.handed());
    }
}

/// One test print: its picture over its name and what printing it tells.
fn sample_card(ui: &mut Ui, index: usize, sample: &Sample, size: egui::Vec2) -> egui::Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, sample.name));
    let (fill, edge) = match response.hovered() {
        true => (colors.raised, colors.accent_deep),
        false => (colors.base, colors.line),
    };
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        theme::R_PANEL,
        fill,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(14.0);
    let stage = Rect::from_min_size(inner.min, vec2(inner.width(), PREVIEW_H));
    let texture = preview(ui.ctx(), index, sample, stage.size());
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let picture = egui::epaint::RectShape::filled(stage, theme::R_CONTROL, egui::Color32::WHITE)
        .with_texture(texture.id(), uv);
    painter.add(picture);
    let name_at = pos2(inner.left(), stage.bottom() + 14.0);
    painter.text(
        name_at,
        Align2::LEFT_TOP,
        sample.name,
        theme::label(),
        colors.text_high,
    );
    let mut job = egui::text::LayoutJob::simple(
        sample.about.to_owned(),
        theme::small(),
        colors.text_mid,
        inner.width(),
    );
    job.wrap.max_rows = 2;
    let about = painter.layout_job(job);
    painter.galley(name_at + vec2(0.0, 24.0), about, colors.text_mid);
    focus_ring(ui, &response, theme::R_PANEL);
    response
}

/// The test print's picture as a texture `size` points large, rendered once per window and
/// again only when the card changes size.
fn preview(
    ctx: &egui::Context,
    index: usize,
    sample: &Sample,
    size: egui::Vec2,
) -> egui::TextureHandle {
    let px = (size * ctx.pixels_per_point()).round();
    let px = [px.x.max(1.0) as usize, px.y.max(1.0) as usize];
    let id = egui::Id::new(("sample-preview", index, px));
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let rgb = |color: egui::Color32| [color.r(), color.g(), color.b()];
    let image = sample.picture(px, rgb(theme::colors().sunken), rgb(theme::scene().object));
    let texture = ctx.load_texture(sample.name, image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}
