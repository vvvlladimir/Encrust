use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, StrokeKind, Ui, WidgetInfo,
    WidgetType, vec2,
};

use crate::ui::theme;
use crate::ui::theme::motion;

mod square;
mod tile;

pub use square::{RailButton, companion_square, icon_button, icon_toggle, rail_button};
pub use tile::{picker, summary_button, swatch, zone_button};

/// The ring a control wears while it holds the keyboard's focus: the accent inside its own
/// edge, and a halo of the accent's wash outside it.
pub fn focus_ring(ui: &Ui, response: &Response, radius: impl Into<CornerRadius>) {
    if response.has_focus() {
        let (radius, colors) = (radius.into(), theme::colors());
        let halo = egui::Stroke::new(theme::FOCUS_HALO_W, colors.accent_wash);
        let ring = egui::Stroke::new(theme::FOCUS_W, colors.accent);
        let painter = ui.painter();
        painter.rect_stroke(response.rect, radius, halo, StrokeKind::Outside);
        painter.rect_stroke(response.rect, radius, ring, StrokeKind::Inside);
    }
}

/// Where a control is painted: its own rect, shrunk towards its centre while it is pressed.
pub(super) fn pressed_rect(ui: &Ui, response: &Response) -> Rect {
    let id = response.id.with("pressed");
    let down = response.is_pointer_button_down_on();
    let pressed = motion::toggle(ui.ctx(), id, down, motion::FAST);
    let scale = 1.0 - (1.0 - PRESSED_SCALE) * pressed;
    Rect::from_center_size(response.rect.center(), response.rect.size() * scale)
}

/// How far a pressed control shrinks.
const PRESSED_SCALE: f32 = 0.96;

/// How much of itself a glyph keeps on a control that cannot be used.
pub(super) const DISABLED_GLYPH: f32 = 0.4;

/// The ordinary full-width button: raised, bordered, an optional leading glyph.
pub fn secondary_button(ui: &mut Ui, glyph: &str, text: &str) -> Response {
    let width = ui.available_width();
    filled_button(
        ui,
        glyph,
        text,
        vec2(width, theme::BUTTON_H),
        &Skin {
            fill: theme::colors().raised,
            foreground: theme::colors().text_high,
            border: Some(theme::colors().line),
            hovered: theme::colors().hover,
            pressed: theme::colors().active,
        },
        true,
    )
}

/// A full-width button that stays pressed: washed in the accent while `on`, so a click
/// takes up a tool and the next one puts it down.
pub fn toggle_button(ui: &mut Ui, glyph: &str, text: &str, on: bool) -> Response {
    let colors = theme::colors();
    let skin = match on {
        true => Skin {
            fill: colors.accent_wash,
            foreground: colors.accent_soft,
            border: Some(colors.accent_deep),
            hovered: colors.accent_wash,
            pressed: colors.accent_wash,
        },
        false => Skin {
            fill: colors.raised,
            foreground: colors.text_high,
            border: Some(colors.line),
            hovered: colors.hover,
            pressed: colors.active,
        },
    };
    let size = vec2(ui.available_width(), theme::BUTTON_H);
    let response = filled_button(ui, glyph, text, size, &skin, true);
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, text));
    response
}

/// A second action standing beside the primary one: raised rather than accented, and as
/// tall as it, so the row reads as one control.
pub fn companion_button(ui: &mut Ui, glyph: &str, text: &str, enabled: bool) -> Response {
    let colors = theme::colors();
    let foreground = if enabled {
        colors.text_high
    } else {
        colors.text_low
    };
    filled_button(
        ui,
        glyph,
        text,
        vec2(ui.available_width(), theme::PRIMARY_H),
        &Skin {
            fill: colors.raised,
            foreground,
            border: Some(colors.line),
            hovered: colors.hover,
            pressed: colors.active,
        },
        enabled,
    )
}

/// A button as wide as what it says, to stand in a row of several. `primary` paints it
/// as the one action of the screen.
pub fn inline_button(
    ui: &mut Ui,
    glyph: &str,
    text: &str,
    primary: bool,
    enabled: bool,
) -> Response {
    let colors = theme::colors();
    let text_w = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::label(), colors.text_high)
        .size()
        .x;
    let glyph_w = if glyph.is_empty() { 0.0 } else { 22.0 };
    let size = vec2(text_w + glyph_w + 32.0, theme::BUTTON_H);
    let skin = match (primary, enabled) {
        (true, true) => Skin {
            fill: colors.accent,
            foreground: colors.on_accent,
            border: None,
            hovered: colors.accent_soft,
            pressed: colors.accent_deep,
        },
        (_, false) => Skin {
            fill: colors.raised,
            foreground: colors.text_low,
            border: Some(colors.line),
            hovered: colors.raised,
            pressed: colors.raised,
        },
        (false, true) => Skin {
            fill: colors.raised,
            foreground: colors.text_high,
            border: Some(colors.line),
            hovered: colors.hover,
            pressed: colors.active,
        },
    };
    filled_button(ui, glyph, text, size, &skin, enabled)
}

/// A button with no surface until it is hovered, for the lesser actions of a row: as wide
/// as what it says, and `danger` when what it does cannot be taken back.
pub fn quiet_button(ui: &mut Ui, glyph: &str, text: &str, danger: bool, enabled: bool) -> Response {
    let colors = theme::colors();
    let text_w = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::label(), colors.text_high)
        .size()
        .x;
    let glyph_w = if glyph.is_empty() { 0.0 } else { 22.0 };
    let size = vec2(text_w + glyph_w + 24.0, theme::BUTTON_H);
    let (foreground, hovered) = match (danger, enabled) {
        (_, false) => (colors.text_low, Color32::TRANSPARENT),
        (true, true) => (colors.danger, colors.danger_wash),
        (false, true) => (colors.text_mid, colors.hover),
    };
    let skin = Skin {
        fill: Color32::TRANSPARENT,
        foreground,
        border: None,
        hovered,
        pressed: if enabled {
            colors.active
        } else {
            Color32::TRANSPARENT
        },
    };
    filled_button(ui, glyph, text, size, &skin, enabled)
}

/// One of a row of filters: bordered, and washed in the accent while it is the one on.
pub fn filter_chip(ui: &mut Ui, text: &str, on: bool) -> Response {
    let colors = theme::colors();
    let text_w = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::label(), colors.text_high)
        .size()
        .x;
    let size = vec2(text_w + 20.0, theme::FIELD_H);
    let skin = match on {
        true => Skin {
            fill: colors.accent_wash,
            foreground: colors.accent_soft,
            border: Some(colors.accent_deep),
            hovered: colors.accent_wash,
            pressed: colors.accent_wash,
        },
        false => Skin {
            fill: Color32::TRANSPARENT,
            foreground: colors.text_mid,
            border: Some(colors.line),
            hovered: colors.hover,
            pressed: colors.active,
        },
    };
    filled_button(ui, "", text, size, &skin, true)
}

/// A button as tall as a field and `width` wide, to stand in a row of fields.
pub fn compact_button(ui: &mut Ui, text: &str, width: f32) -> Response {
    filled_button(
        ui,
        "",
        text,
        vec2(width, theme::FIELD_H),
        &Skin {
            fill: theme::colors().raised,
            foreground: theme::colors().text_high,
            border: Some(theme::colors().line),
            hovered: theme::colors().hover,
            pressed: theme::colors().active,
        },
        true,
    )
}

/// The one action a screen is for, in words alone: the tool's glyph already heads the
/// card. There is never a second one of these in view.
pub fn primary_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let colors = theme::colors();
    let (fill, foreground) = if enabled {
        (colors.accent, colors.on_accent)
    } else {
        (colors.raised, colors.text_low)
    };
    let skin = Skin {
        fill,
        foreground,
        border: None,
        hovered: colors.accent_soft,
        pressed: colors.accent_deep,
    };
    let size = vec2(ui.available_width(), theme::PRIMARY_H);
    let face = Face {
        glyph: "",
        text,
        font: theme::primary(),
    };
    painted_button(ui, &face, size, &skin, enabled)
}

/// Why a button cannot be pressed, shown only while the pointer is on it; nothing while it can.
pub fn blocked_hover(response: Response, blocker: Option<String>) -> Response {
    match blocker {
        Some(reason) => response.on_hover_text(reason),
        None => response,
    }
}

/// Space kept clear either side of a button's label.
const LABEL_PAD: f32 = 10.0;

/// What a text button is painted in, in each of the states it can be in.
pub(in crate::ui::widgets) struct Skin {
    pub fill: Color32,
    pub foreground: Color32,
    pub border: Option<Color32>,
    pub hovered: Color32,
    pub pressed: Color32,
}

/// What a text button says: its glyph, if any, and its label in `font`.
pub(in crate::ui::widgets) struct Face<'a> {
    pub glyph: &'a str,
    pub text: &'a str,
    pub font: FontId,
}

/// A text button in the label face.
pub(in crate::ui::widgets) fn filled_button(
    ui: &mut Ui,
    glyph: &str,
    text: &str,
    size: egui::Vec2,
    skin: &Skin,
    enabled: bool,
) -> Response {
    let face = Face {
        glyph,
        text,
        font: theme::label(),
    };
    painted_button(ui, &face, size, skin, enabled)
}

/// One drawing behind every text button: a filled rounded rect, a glyph and a label.
fn painted_button(
    ui: &mut Ui,
    face: &Face<'_>,
    size: egui::Vec2,
    skin: &Skin,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (_, response) = ui.allocate_exact_size(size, sense);
    let text = face.text;
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, text));
    paint_button(ui, &response, face, skin, enabled);
    response
}

/// Paints a text button over a response already laid out.
pub(in crate::ui::widgets) fn paint_button(
    ui: &Ui,
    response: &Response,
    face: &Face<'_>,
    skin: &Skin,
    enabled: bool,
) {
    let Face { glyph, text, .. } = *face;
    let fill = match (
        enabled,
        response.is_pointer_button_down_on(),
        response.hovered(),
    ) {
        (false, _, _) => skin.fill,
        (true, true, _) => skin.pressed,
        (true, false, true) => skin.hovered,
        (true, false, false) => skin.fill,
    };
    let foreground = skin.foreground;

    let rect = pressed_rect(ui, response);
    let painter = ui.painter();
    let stroke = skin
        .border
        .map_or(egui::Stroke::NONE, |border| egui::Stroke::new(1.0, border));
    painter.rect(rect, theme::R_CONTROL, fill, stroke, StrokeKind::Inside);

    let glyph_width = if glyph.is_empty() { 0.0 } else { GLYPH_W };
    // A label longer than its button is cut rather than drawn past the edge: the text
    // comes from a file name or a printer, and neither has a length we chose.
    let mut job =
        egui::text::LayoutJob::simple_singleline(text.to_owned(), face.font.clone(), foreground);
    job.wrap = egui::text::TextWrapping::truncate_at_width(
        (response.rect.width() - glyph_width - 2.0 * LABEL_PAD).max(0.0),
    );
    let galley = painter.layout_job(job);
    let content_left = rect.center().x - (glyph_width + galley.size().x) / 2.0;
    if !glyph.is_empty() {
        painter.text(
            egui::pos2(content_left + 8.0, rect.center().y),
            Align2::CENTER_CENTER,
            glyph,
            theme::icon(16.0),
            if enabled {
                foreground
            } else {
                foreground.gamma_multiply(DISABLED_GLYPH)
            },
        );
    }
    painter.galley(
        egui::pos2(
            content_left + glyph_width,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        foreground,
    );
    focus_ring(ui, response, theme::R_CONTROL);
}

/// Room a leading glyph takes in a text button, points.
pub(in crate::ui::widgets) const GLYPH_W: f32 = 22.0;
