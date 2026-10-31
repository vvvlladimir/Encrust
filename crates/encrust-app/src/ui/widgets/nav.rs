//! One entry of a window's list of pages, as Settings and the key sheet both list them.

use egui::{Align2, Color32, Response, Sense, Ui, WidgetInfo, WidgetType, vec2};

use super::button::{focus_ring, hover_mix};
use crate::ui::{icon, theme};

/// A page row's height, points.
pub const NAV_ROW_H: f32 = theme::BUTTON_H + 4.0;
/// The badge on a page a release waits on, points across.
const BADGE_D: f32 = 16.0;

/// What stands at the right of a page's row.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NavAside {
    None,
    /// How many profiles the page lists.
    Count(usize),
    /// A release waits to be installed.
    Waiting,
    /// The row opens something other than a page of this window.
    Elsewhere,
}

/// One entry of the list of pages: a glyph and a name, washed while it is the page open,
/// and at its right what [`NavAside`] says.
pub fn nav_row(ui: &mut Ui, glyph: &str, label: &str, active: bool, aside: NavAside) -> Response {
    let colors = theme::colors();
    let size = vec2(ui.available_width(), NAV_ROW_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, active, label));
    let (fill, text) = match active {
        true => (colors.accent_wash, colors.accent_soft),
        false => (
            hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover),
            hover_mix(ui, &response, colors.text_mid, colors.text_high),
        ),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let left = Align2::LEFT_CENTER;
    let glyph_at = rect.left_center() + vec2(12.0, 0.0);
    painter.text(glyph_at, left, glyph, theme::icon(15.0), text);
    let label_at = rect.left_center() + vec2(36.0, 0.0);
    painter.text(label_at, left, label, theme::body(), text);
    aside_of(ui, rect.right_center() - vec2(12.0, 0.0), aside, active);
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// Paints `aside` ending at `right`.
fn aside_of(ui: &Ui, right: egui::Pos2, aside: NavAside, active: bool) {
    let (colors, painter) = (theme::colors(), ui.painter());
    let at = Align2::RIGHT_CENTER;
    match aside {
        NavAside::None => {}
        NavAside::Count(count) => {
            let ink = if active {
                colors.accent_soft
            } else {
                colors.text_low
            };
            painter.text(right, at, count.to_string(), theme::figures(12.0), ink);
        }
        NavAside::Waiting => {
            let centre = right - vec2(BADGE_D / 2.0, 0.0);
            painter.circle_filled(centre, BADGE_D / 2.0, colors.accent);
            let middle = Align2::CENTER_CENTER;
            painter.text(centre, middle, "1", theme::figures(11.0), colors.on_accent);
        }
        NavAside::Elsewhere => {
            let glyph = icon::ELSEWHERE;
            painter.text(right, at, glyph, theme::icon(13.0), colors.text_low);
        }
    }
}
