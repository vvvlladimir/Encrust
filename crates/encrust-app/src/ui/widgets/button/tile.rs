use egui::{Align2, Rect, Response, Sense, StrokeKind, Ui, WidgetInfo, WidgetType, vec2};

use super::{focus_ring, hover_mix};
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
    let fill = hover_mix(ui, &response, colors.raised, colors.hover);
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
    let fill = hover_mix(ui, &response, colors.raised, colors.hover);
    let line = hover_mix(ui, &response, colors.line, colors.accent_soft);
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

/// A rounded tag in the bar ringed in its tint, the one shape both badges are drawn as;
/// its glyph alone when `compact`, its words alone otherwise.
pub fn bar_pill(
    ui: &mut egui::Ui,
    glyph: &str,
    text: &str,
    compact: bool,
    tint: egui::Color32,
    wash: egui::Color32,
) -> egui::Response {
    let (shown, font) = if compact {
        (glyph, theme::icon(13.0))
    } else {
        (text, theme::small())
    };
    let label = ui.painter().layout_no_wrap(shown.to_owned(), font, tint);
    let size = vec2(label.size().x + PILL_PAD * 2.0, PILL_H);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let fill = if response.hovered() {
        wash
    } else {
        egui::Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        egui::CornerRadius::same(255),
        fill,
        egui::Stroke::new(1.0, tint.gamma_multiply(PILL_RING)),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.center() - label.size() / 2.0, label, tint);
    focus_ring(ui, &response, egui::CornerRadius::same(255));
    response
}

/// A badge's padding either side of its words, its height, and how strongly its ring
/// carries the tint.
const PILL_PAD: f32 = 8.0;
const PILL_H: f32 = 22.0;
const PILL_RING: f32 = 0.35;
