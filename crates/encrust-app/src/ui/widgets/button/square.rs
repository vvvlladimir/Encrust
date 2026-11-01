use egui::{Align2, Color32, Response, Sense, Ui, WidgetInfo, WidgetType, vec2};

use super::focus_ring;
use crate::ui::theme;

/// A borderless square with one glyph in it: the top bar and the overlay cards.
pub fn icon_button(ui: &mut Ui, glyph: &str, tooltip: &str) -> Response {
    square(ui, glyph, tooltip, Square::icon())
}

/// An icon square that stays lit while `on`: a lock, a link, a mode held down.
pub fn icon_toggle(ui: &mut Ui, glyph: &str, tooltip: &str, on: bool) -> Response {
    square(
        ui,
        glyph,
        tooltip,
        Square {
            active: on,
            ..Square::icon()
        },
    )
}

/// A rail button filling the room it is given: a glyph over the tool's name, or the glyph
/// alone, raised in the accent while it is the tool in use, and a dot in `badge` at its
/// corner while the tool needs attention.
pub fn rail_button(
    ui: &mut Ui,
    glyph: &str,
    name: &str,
    labelled: bool,
    tooltip: &str,
    active: bool,
    badge: Option<Color32>,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, active, name));
    let colors = theme::colors();
    let (fill, foreground) = match (active, response.hovered()) {
        (true, _) => (colors.raised, colors.accent_soft),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (Color32::TRANSPARENT, colors.text_mid),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let glyph_at = if labelled {
        rect.center_top() + vec2(0.0, 16.0)
    } else {
        rect.center()
    };
    painter.text(
        glyph_at,
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(17.0),
        foreground,
    );
    if labelled {
        painter.text(
            rect.center_bottom() - vec2(0.0, 9.0),
            Align2::CENTER_CENTER,
            name,
            theme::rail_name(),
            foreground,
        );
    }
    if let Some(badge) = badge {
        let at = glyph_at + vec2(RAIL_BADGE_OFFSET.x, RAIL_BADGE_OFFSET.y - 16.0);
        painter.circle_filled(at, RAIL_BADGE_R + 1.5, colors.base);
        painter.circle_filled(at, RAIL_BADGE_R, badge);
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_text(tooltip)
}

/// Where a rail button's dot sits from the top of its glyph, and its radius, points.
const RAIL_BADGE_OFFSET: egui::Vec2 = vec2(10.0, 8.0);
const RAIL_BADGE_R: f32 = 3.0;

/// Every square icon control is the same drawing with different numbers.
struct Square {
    size: f32,
    glyph_size: f32,
    active: bool,
}

impl Square {
    fn icon() -> Self {
        Self {
            size: theme::ICON_SIZE,
            glyph_size: 16.0,
            active: false,
        }
    }
}

fn square(ui: &mut Ui, glyph: &str, tooltip: &str, config: Square) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(config.size, config.size), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), config.active, tooltip)
    });
    let colors = theme::colors();

    let (fill, foreground) = match (config.active, response.hovered()) {
        (true, _) => (colors.accent_wash, colors.accent_soft),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (Color32::TRANSPARENT, colors.text_mid),
    };

    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(config.glyph_size),
        foreground,
    );
    focus_ring(ui, &response, theme::R_CONTROL);

    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}
