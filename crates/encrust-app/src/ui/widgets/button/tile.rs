use egui::{Align2, Rect, Response, Sense, StrokeKind, Ui, WidgetInfo, WidgetType, vec2};

use super::focus_ring;
use crate::ui::theme;

/// A picture to pick from a grid, `side` points square, ringed in the accent while it is
/// the one picked.
pub fn swatch(
    ui: &mut Ui,
    texture: &egui::TextureHandle,
    side: f32,
    picked: bool,
    name: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, picked, name));
    let colors = theme::colors();
    egui::Image::new(texture)
        .corner_radius(theme::R_CONTROL)
        .paint_at(ui, rect);
    let stroke = match (picked, response.hovered()) {
        (true, _) => egui::Stroke::new(theme::FOCUS_W, colors.accent),
        (false, true) => egui::Stroke::new(1.0, colors.text_low),
        (false, false) => egui::Stroke::new(1.0, colors.line),
    };
    ui.painter()
        .rect_stroke(rect, theme::R_CONTROL, stroke, StrokeKind::Inside);
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_text(name)
}

/// A row that opens a chooser: a leading glyph, what is chosen, and a caret.
pub fn picker(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    preset_picker(ui, glyph, label, false)
}

/// A `picker` of named settings: what is chosen, and `· edited` in `warn` after it once the
/// values differ from the ones it holds.
pub fn preset_picker(ui: &mut Ui, glyph: &str, label: &str, edited: bool) -> Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, ui.is_enabled(), label));

    let border = if response.hovered() {
        colors.line
    } else {
        colors.hairline
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        theme::R_CONTROL,
        colors.sunken,
        egui::Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    painter.text(
        egui::pos2(rect.left() + 9.0 + 7.0, rect.center().y),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(14.0),
        colors.text_low,
    );
    painter.text(
        egui::pos2(rect.right() - 9.0 - 7.0, rect.center().y),
        Align2::CENTER_CENTER,
        crate::ui::icon::CARET_DOWN,
        theme::icon(14.0),
        colors.text_low,
    );
    let text_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 26.0, rect.top()),
        egui::pos2(rect.right() - 26.0, rect.bottom()),
    );
    let mut job = egui::text::LayoutJob::default();
    let format = |color| egui::TextFormat::simple(theme::label(), color);
    job.append(label, 0.0, format(colors.text_high));
    if edited {
        job.append(
            &crate::i18n::tr!("presets-edited"),
            4.0,
            format(colors.warn),
        );
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width(text_rect.width());
    let galley = painter.layout_job(job);
    painter.galley(
        egui::pos2(
            text_rect.left(),
            text_rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        colors.text_high,
    );
    focus_ring(ui, &response, theme::R_CONTROL);

    response
}

/// A card that opens a view of its own: a glyph in the colour of what it found, a title,
/// a line under it, and a chevron saying it leads somewhere.
pub fn summary_button(
    ui: &mut Ui,
    glyph: &str,
    tint: egui::Color32,
    title: &str,
    subtitle: &str,
) -> Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), SUMMARY_H), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, title));
    let fill = if response.hovered() {
        colors.hover
    } else {
        colors.raised
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        theme::R_SURFACE,
        fill,
        egui::Stroke::new(1.0, colors.line),
        egui::StrokeKind::Inside,
    );

    let glyph_at = rect.left_center() + vec2(20.0, 0.0);
    painter.text(
        glyph_at,
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(20.0),
        tint,
    );
    let text_x = rect.left() + 42.0;
    painter.text(
        egui::pos2(text_x, rect.center().y - 1.0),
        Align2::LEFT_BOTTOM,
        title,
        theme::label(),
        colors.text_high,
    );
    painter.text(
        egui::pos2(text_x, rect.center().y + 2.0),
        Align2::LEFT_TOP,
        subtitle,
        theme::small(),
        colors.text_low,
    );
    painter.text(
        rect.right_center() - vec2(14.0, 0.0),
        Align2::CENTER_CENTER,
        crate::ui::icon::CARET_RIGHT,
        theme::icon(14.0),
        colors.text_low,
    );
    focus_ring(ui, &response, theme::R_SURFACE);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Height of a summary card: two lines of text and room around them.
const SUMMARY_H: f32 = 52.0;

/// A large target filling `rect`: a glyph over a title and a line under it, for the few
/// things a view with nothing in it yet offers.
pub fn zone_button(ui: &mut Ui, rect: Rect, glyph: &str, title: &str, detail: &str) -> Response {
    let colors = theme::colors();
    let response = ui.allocate_rect(rect, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, title));
    let (fill, line) = if response.hovered() {
        (colors.hover, colors.accent_soft)
    } else {
        (colors.raised, colors.line)
    };
    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        theme::R_SURFACE,
        fill,
        egui::Stroke::new(1.0, line),
        StrokeKind::Inside,
    );
    let middle = rect.center();
    let wrap = (rect.width() - 2.0 * ZONE_PAD).max(0.0);
    painter.text(
        middle - vec2(0.0, 22.0),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(30.0),
        colors.text_mid,
    );
    painter.text(
        middle + vec2(0.0, 12.0),
        Align2::CENTER_CENTER,
        title,
        theme::page_title(16.0),
        colors.text_high,
    );
    let mut job =
        egui::text::LayoutJob::simple(detail.to_owned(), theme::small(), colors.text_low, wrap);
    job.halign = egui::Align::Center;
    let words = painter.layout_job(job);
    painter.galley(middle + vec2(0.0, 26.0), words, colors.text_low);
    focus_ring(ui, &response, theme::R_SURFACE);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Room a zone's line keeps from its edges.
const ZONE_PAD: f32 = 24.0;
