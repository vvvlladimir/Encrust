//! A printer as one row on `raised`, and the pictures of its prints; see
//! `docs/design/ui-design-system.md`, Device row.

use egui::{Align2, Color32, Rect, Response, Sense, StrokeKind, Ui, pos2, vec2};

use super::focus_ring;
use crate::ui::{icon, theme};

/// What a device row says: its name, its state word in its colour, whether that state is
/// work under way, and whether the row keeps room at its right for a sheet's square.
pub struct DeviceRow<'a> {
    pub name: &'a str,
    pub state: &'a str,
    pub tint: Color32,
    pub busy: bool,
    pub opens: bool,
}

/// Points from the row's right edge to the middle of the sheet's square inside it.
pub const DEVICE_OPEN_INSET: f32 = 20.0;

/// How dim a busy device's dot breathes down to.
const BREATH_LOW: f32 = 0.35;

/// Draws `row`: the printer glyph, its name cut short before the state, the state word
/// and its dot, breathing while busy, and the caret. Answers the row and where its state
/// stands, for a hover of its own.
pub fn device_row(ui: &mut Ui, row: &DeviceRow<'_>) -> (Response, Rect) {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), theme::PRINTER_ROW_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.hovered() {
        colors.hover
    } else {
        colors.raised
    };
    let painter = ui.painter();
    let stroke = egui::Stroke::new(1.0, colors.hairline);
    painter.rect(rect, theme::R_CONTROL, fill, stroke, StrokeKind::Inside);
    let y = rect.center().y;
    let low = colors.text_low;
    painter.text(
        pos2(rect.left() + 16.0, y),
        Align2::CENTER_CENTER,
        icon::PRINTER,
        theme::icon(14.0),
        low,
    );
    let right = match row.opens {
        true => rect.right() - 2.0 * DEVICE_OPEN_INSET,
        false => rect.right(),
    };
    painter.text(
        pos2(right - 16.0, y),
        Align2::CENTER_CENTER,
        icon::CARET_DOWN,
        theme::icon(14.0),
        low,
    );
    let status = paint_status(painter, row, rect, right - 30.0);
    paint_name(painter, row.name, rect, status.left());
    focus_ring(ui, &response, theme::R_CONTROL);
    let name = row.name;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, name));
    (response, status)
}

/// The name at the row's left, cut short where the status word begins.
fn paint_name(painter: &egui::Painter, name: &str, rect: Rect, status_left: f32) {
    let ink = theme::colors().text_high;
    let room = (status_left - 8.0 - rect.left() - 30.0).max(0.0);
    let mut job = egui::text::LayoutJob::simple_singleline(name.to_owned(), theme::label(), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let galley = painter.layout_job(job);
    let at = pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(at, galley, ink);
}

/// The status word and its dot, right-aligned on `right`; answers where they stand, the
/// full height of the row.
fn paint_status(painter: &egui::Painter, row: &DeviceRow<'_>, rect: Rect, right: f32) -> Rect {
    let word = painter.layout_no_wrap(row.state.to_owned(), theme::small(), row.tint);
    let at = pos2(right - word.size().x, rect.center().y - word.size().y / 2.0);
    painter.galley(at, word, row.tint);
    let dot = pos2(at.x - 8.0, rect.center().y);
    let dot_colour = match row.busy {
        true => {
            let breath = theme::motion::breathe(painter.ctx());
            row.tint
                .gamma_multiply(BREATH_LOW + (1.0 - BREATH_LOW) * breath)
        }
        false => row.tint,
    };
    painter.circle_filled(dot, 3.5, dot_colour);
    Rect::from_x_y_ranges(dot.x - 4.0..=right, rect.y_range())
}

/// A print's picture fitted into `size`, rounded like the card, or the model glyph on a
/// sunken square while there is none.
pub fn print_picture(ui: &mut Ui, picture: Option<&egui::TextureHandle>, size: egui::Vec2) {
    match picture {
        Some(texture) => {
            let image = egui::Image::new(texture)
                .fit_to_exact_size(size)
                .corner_radius(theme::R_CONTROL);
            ui.add(image);
        }
        None => {
            let colors = theme::colors();
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.painter()
                .rect_filled(rect, theme::R_CONTROL, colors.sunken);
            let at = rect.center();
            ui.painter().text(
                at,
                Align2::CENTER_CENTER,
                icon::MODEL,
                theme::icon(18.0),
                colors.text_low,
            );
        }
    }
}
