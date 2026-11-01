use egui::{Align2, Color32, Response, Sense, StrokeKind, Ui, WidgetInfo, WidgetType, vec2};

use super::{DISABLED_GLYPH, focus_ring, hover_mix, pressed_rect, sense_for};
use crate::ui::theme;

/// A borderless square with one glyph in it: the top bar and the overlay cards.
pub fn icon_button(ui: &mut Ui, glyph: &str, tooltip: &str) -> Response {
    square(ui, glyph, tooltip, Square::icon())
}

/// The same square at 26 points, for the rows of a card: its plates, its actions.
pub fn small_icon_button(ui: &mut Ui, glyph: &str, tooltip: &str) -> Response {
    square(ui, glyph, tooltip, Square::small())
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

/// An icon square standing beside a bigger control: raised, bordered and `side` points
/// square, as tall as its neighbour — 40 beside a primary button, 32 beside a picker.
pub fn companion_square(
    ui: &mut Ui,
    glyph: &str,
    tooltip: &str,
    side: f32,
    enabled: bool,
) -> Response {
    glyph_square(ui, (glyph, tooltip), side, SquareLook::Framed, enabled)
}

/// A borderless square `side` points across beside a picker, its glyph in the mid tone:
/// the preset row's Save, lighter than the picker it serves.
pub fn quiet_square(ui: &mut Ui, glyph: &str, tooltip: &str, side: f32, enabled: bool) -> Response {
    glyph_square(ui, (glyph, tooltip), side, SquareLook::Quiet, enabled)
}

fn glyph_square(
    ui: &mut Ui,
    (glyph, tooltip): (&str, &str),
    side: f32,
    look: SquareLook,
    enabled: bool,
) -> Response {
    let (_, response) = ui.allocate_exact_size(vec2(side, side), sense_for(enabled));
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, tooltip));
    paint_square(ui, &response, (look, glyph), None, enabled);
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_text(tooltip)
}

/// How an icon square stands: raised and ringed beside a bigger control, or a bare glyph
/// in a row or beside a field.
#[derive(Clone, Copy)]
pub enum SquareLook {
    Framed,
    Quiet,
}

/// Paints an icon square over `response`: its fill, ring and glyph by `look` and its height,
/// or on the danger's wash in `armed`, the ring of an armed control.
pub(in crate::ui::widgets) fn paint_square(
    ui: &Ui,
    response: &Response,
    (look, glyph): (SquareLook, &str),
    armed: Option<Color32>,
    enabled: bool,
) {
    let colors = theme::colors();
    let rect = pressed_rect(ui, response);
    let eased = |rest: Color32, hovered: Color32| match enabled {
        true => hover_mix(ui, response, rest, hovered),
        false => rest,
    };
    let (fill, ring) = match (armed, look) {
        (Some(ring), _) => (colors.danger_wash, Some(ring)),
        (None, SquareLook::Framed) => (eased(colors.raised, colors.hover), Some(colors.hairline)),
        (None, SquareLook::Quiet) => (eased(Color32::TRANSPARENT, colors.raised), None),
    };
    let stroke = ring.map_or(egui::Stroke::NONE, |ring| egui::Stroke::new(1.0, ring));
    let painter = ui.painter();
    painter.rect(rect, theme::R_CONTROL, fill, stroke, StrokeKind::Inside);
    let tall = match look {
        SquareLook::Framed => rect.height() >= theme::PRIMARY_H - 2.0,
        SquareLook::Quiet => rect.height() >= theme::BUTTON_H - 2.0,
    };
    let rest = match (look, tall) {
        (SquareLook::Framed, _) => colors.text_high,
        (SquareLook::Quiet, true) => colors.text_mid,
        (SquareLook::Quiet, false) => colors.text_low,
    };
    let ink = match (enabled, armed.is_some(), response.hovered()) {
        (false, ..) => rest.gamma_multiply(DISABLED_GLYPH),
        (true, true, _) => colors.danger,
        (true, false, true) => colors.text_high,
        (true, false, false) => rest,
    };
    let size_pt = match (look, tall) {
        (SquareLook::Framed, true) => 18.0,
        (SquareLook::Framed, false) | (SquareLook::Quiet, true) => 16.0,
        (SquareLook::Quiet, false) => 13.0,
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(size_pt),
        ink,
    );
}

/// One tool of the tool row, filling the room it is given: a glyph over the tool's name,
/// or the glyph alone. Washed in the accent while it is the tool in use; a dot in `badge`
/// at its top right while the tool needs attention; `key`, the digit that picks it, at its
/// top left while the pointer is on the row.
pub struct RailButton<'a> {
    pub glyph: &'a str,
    pub name: &'a str,
    pub tooltip: &'a str,
    pub labelled: bool,
    pub active: bool,
    pub badge: Option<Color32>,
    pub key: Option<&'a str>,
}

/// Draws a [`RailButton`].
pub fn rail_button(ui: &mut Ui, button: &RailButton<'_>) -> Response {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
    let name = button.name;
    let active = button.active;
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, active, name));
    let colors = theme::colors();
    let (fill, foreground) = match (active, response.hovered()) {
        (true, _) => (colors.accent_wash, colors.accent_soft),
        (false, hovered) => (
            hover_mix(ui, &response, Color32::TRANSPARENT, colors.raised),
            if hovered {
                colors.text_high
            } else {
                colors.text_mid
            },
        ),
    };
    let shrunk = pressed_rect(ui, &response);
    let painter = ui.painter();
    painter.rect_filled(shrunk, theme::R_CONTROL, fill);
    let glyph_at = if button.labelled {
        shrunk.center() - vec2(0.0, RAIL_NAME_RISE)
    } else {
        shrunk.center()
    };
    painter.text(
        glyph_at,
        Align2::CENTER_CENTER,
        button.glyph,
        theme::icon(RAIL_GLYPH_PT),
        foreground,
    );
    if button.labelled {
        rail_name(painter, shrunk, name, foreground);
    }
    rail_corners(painter, rect, button);
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_text(button.tooltip)
}

/// The name under a rail button's glyph, cut to the button's width.
fn rail_name(painter: &egui::Painter, shrunk: egui::Rect, name: &str, ink: Color32) {
    let mut job =
        egui::text::LayoutJob::simple_singleline(name.to_owned(), theme::rail_name(), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(shrunk.width() - 2.0 * RAIL_NAME_PAD);
    let galley = painter.layout_job(job);
    let at = shrunk.center() + vec2(0.0, RAIL_NAME_DROP) - galley.size() / 2.0;
    painter.galley(at, galley, ink);
}

/// The digit of its key at the top left while named, and its badge at the top right.
fn rail_corners(painter: &egui::Painter, rect: egui::Rect, button: &RailButton<'_>) {
    let colors = theme::colors();
    if let Some(key) = button.key.filter(|_| button.labelled) {
        let ink = if button.active {
            colors.accent_soft
        } else {
            colors.text_low
        };
        let at = rect.left_top() + RAIL_KEY_OFFSET;
        painter.text(at, Align2::LEFT_TOP, key, theme::rail_key(), ink);
    }
    if let Some(badge) = button.badge {
        let at = rect.right_top() + RAIL_BADGE_OFFSET;
        painter.circle_filled(at, RAIL_BADGE_R + 2.0, colors.panel);
        painter.circle_filled(at, RAIL_BADGE_R, badge);
    }
}

/// A rail button's glyph, and how far it rises above the button's middle and its name
/// falls below it, points.
const RAIL_GLYPH_PT: f32 = 19.0;
const RAIL_NAME_RISE: f32 = 7.0;
const RAIL_NAME_DROP: f32 = 13.0;
const RAIL_NAME_PAD: f32 = 2.0;
/// Where a rail button's digit and dot sit from its top corners, and the dot's radius.
const RAIL_KEY_OFFSET: egui::Vec2 = vec2(4.0, 3.0);
const RAIL_BADGE_OFFSET: egui::Vec2 = vec2(-13.5, 9.5);
const RAIL_BADGE_R: f32 = 3.5;

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

    fn small() -> Self {
        Self {
            size: theme::SMALL_ICON_SIZE,
            glyph_size: 15.0,
            active: false,
        }
    }
}

fn square(ui: &mut Ui, glyph: &str, tooltip: &str, config: Square) -> Response {
    let (_, response) = ui.allocate_exact_size(vec2(config.size, config.size), Sense::click());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, config.active, tooltip));
    let colors = theme::colors();

    let (fill, foreground) = match (config.active, enabled && response.hovered()) {
        (true, _) => (colors.accent_wash, colors.accent_soft),
        (false, hovered) => (
            match enabled {
                true => hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover),
                false => Color32::TRANSPARENT,
            },
            if hovered {
                colors.text_high
            } else {
                colors.text_mid
            },
        ),
    };
    let foreground = if enabled {
        foreground
    } else {
        foreground.gamma_multiply(DISABLED_GLYPH)
    };

    let rect = pressed_rect(ui, &response);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::fixtures;

    #[test]
    fn a_companion_square_cannot_be_pressed_while_disabled() {
        let ctx = fixtures::context();
        let mut clicked = None;
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            let response = companion_square(ui, "x", "Slice to a file", 40.0, false);
            assert_eq!(
                response.rect.size(),
                vec2(40.0, 40.0),
                "as tall as a primary"
            );
            clicked = Some(response.sense.senses_click());
        });
        assert_eq!(clicked, Some(false), "a disabled square only takes a hover");
    }
}
