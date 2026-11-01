use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, StrokeKind, Ui, WidgetInfo,
    WidgetType, vec2,
};

use crate::ui::theme;
use crate::ui::theme::motion;

mod square;
mod tile;

pub(in crate::ui::widgets) use square::paint_square;
pub use square::{
    RailButton, SquareLook, companion_square, icon_button, icon_toggle, numbered_square,
    quiet_square, rail_button, small_icon_button,
};
pub use tile::{bar_pill, summary_button, swatch, zone_button};

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

/// `rest` easing to `hovered` by `FAST` while the pointer is on `response`.
pub fn hover_mix(ui: &Ui, response: &Response, rest: Color32, hovered: Color32) -> Color32 {
    let id = response.id.with("hover");
    let how = motion::toggle(ui.ctx(), id, response.hovered(), motion::FAST);
    rest.lerp_to_gamma(hovered, how)
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
    let size = vec2(ui.available_width(), theme::BUTTON_H);
    filled_button(ui, glyph, text, size, &Skin::raised(), true)
}

/// A full-width button that stays pressed: washed in the accent while `on`, so a click
/// takes up a tool and the next one puts it down.
pub fn toggle_button(ui: &mut Ui, glyph: &str, text: &str, on: bool) -> Response {
    let skin = if on { Skin::on() } else { Skin::raised() };
    let size = vec2(ui.available_width(), theme::BUTTON_H);
    let response = filled_button(ui, glyph, text, size, &skin, true);
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, text));
    response
}

/// A second action standing beside the primary one: raised rather than accented, and as
/// tall as it, so the row reads as one control.
pub fn companion_button(ui: &mut Ui, glyph: &str, text: &str, enabled: bool) -> Response {
    let size = vec2(ui.available_width(), theme::PRIMARY_H);
    filled_button(
        ui,
        glyph,
        text,
        size,
        &Skin::raised_or_off(enabled),
        enabled,
    )
}

/// What a companion button adds to its words: its glyph and the room either side.
const COMPANION_CHROME_W: f32 = 50.0;

/// The primary action and its companion in one row, the companion as wide as its words:
/// `(text, enabled)` and `(glyph, text, enabled)`. Returns both responses.
pub fn primary_with_companion(
    ui: &mut Ui,
    (text, enabled): (&str, bool),
    (glyph, companion, companion_enabled): (&str, &str, bool),
) -> (Response, Response) {
    let companion_w = label_w(ui, companion) + COMPANION_CHROME_W;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::BUTTON_GAP;
        let room = ui.available_width() - companion_w - theme::BUTTON_GAP;
        let primary = ui
            .allocate_ui(vec2(room, theme::PRIMARY_H), |ui| {
                primary_button(ui, text, enabled)
            })
            .inner;
        let second = companion_button(ui, glyph, companion, companion_enabled);
        (primary, second)
    })
    .inner
}

/// How wide `text` stands in the label face.
fn label_w(ui: &Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), theme::label(), theme::colors().text_high)
        .size()
        .x
}

/// How wide a button as wide as what it says stands: its words, its glyph if any, `pad`.
fn words_w(ui: &Ui, glyph: &str, text: &str, pad: f32) -> f32 {
    let glyph_w = if glyph.is_empty() { 0.0 } else { GLYPH_W };
    label_w(ui, text) + glyph_w + pad
}

/// What an [`inline_button`] says and how it stands: plain and usable unless told.
#[derive(Clone, Copy)]
pub struct Inline<'a> {
    glyph: &'a str,
    text: &'a str,
    primary: bool,
    enabled: bool,
}

impl<'a> Inline<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            glyph: "",
            text,
            primary: false,
            enabled: true,
        }
    }

    /// A leading glyph from `ui::icon`.
    pub fn glyph(self, glyph: &'a str) -> Self {
        Self { glyph, ..self }
    }

    /// Painted as the one action of the screen.
    pub fn primary(self) -> Self {
        self.primary_if(true)
    }

    pub fn primary_if(self, primary: bool) -> Self {
        Self { primary, ..self }
    }

    pub fn enabled(self, enabled: bool) -> Self {
        Self { enabled, ..self }
    }
}

/// How wide an [`inline_button`] saying `text`, with `glyph` if any, stands.
pub fn inline_button_w(ui: &Ui, glyph: &str, text: &str) -> f32 {
    words_w(ui, glyph, text, INLINE_PAD)
}

/// Room an inline button keeps around its words, both sides together.
const INLINE_PAD: f32 = 32.0;

/// A button as wide as what it says, to stand in a row of several.
pub fn inline_button(ui: &mut Ui, button: Inline<'_>) -> Response {
    let Inline {
        glyph,
        text,
        primary,
        enabled,
    } = button;
    let size = vec2(inline_button_w(ui, glyph, text), theme::BUTTON_H);
    let skin = if primary && enabled {
        Skin::accent()
    } else {
        Skin::raised_or_off(enabled)
    };
    filled_button(ui, glyph, text, size, &skin, enabled)
}

/// Room a quiet button keeps around its words, both sides together.
const QUIET_PAD: f32 = 24.0;

/// A button with no surface until it is hovered, for the lesser actions of a row: as wide
/// as what it says, and `danger` when what it does cannot be taken back.
pub fn quiet_button(ui: &mut Ui, glyph: &str, text: &str, danger: bool, enabled: bool) -> Response {
    let colors = theme::colors();
    let size = vec2(words_w(ui, glyph, text, QUIET_PAD), theme::BUTTON_H);
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

/// Room a filter chip keeps around its words, both sides together.
const CHIP_PAD: f32 = 20.0;

/// One of a row of filters: bordered, and washed in the accent while it is the one on.
pub fn filter_chip(ui: &mut Ui, text: &str, on: bool) -> Response {
    let colors = theme::colors();
    let size = vec2(words_w(ui, "", text, CHIP_PAD), theme::FIELD_H);
    let skin = if on {
        Skin::on()
    } else {
        Skin {
            fill: Color32::TRANSPARENT,
            foreground: colors.text_mid,
            ..Skin::raised()
        }
    };
    filled_button(ui, "", text, size, &skin, true)
}

/// A button as tall as a field and `width` wide, to stand in a row of fields.
pub fn compact_button(ui: &mut Ui, text: &str, width: f32) -> Response {
    let size = vec2(width, theme::FIELD_H);
    filled_button(ui, "", text, size, &Skin::raised(), true)
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
        ..Skin::accent()
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

/// What a control listens for: a click while it can be used, the hover for its reason
/// while it cannot.
pub(in crate::ui::widgets) fn sense_for(enabled: bool) -> Sense {
    if enabled {
        Sense::click()
    } else {
        Sense::hover()
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

impl Skin {
    /// The ordinary raised, bordered button.
    pub fn raised() -> Self {
        let colors = theme::colors();
        Self {
            fill: colors.raised,
            foreground: colors.text_high,
            border: Some(colors.line),
            hovered: colors.hover,
            pressed: colors.active,
        }
    }

    /// The raised button, or its look when it cannot be used: low ink, nothing on hover.
    pub fn raised_or_off(enabled: bool) -> Self {
        let colors = theme::colors();
        if enabled {
            return Self::raised();
        }
        Self {
            foreground: colors.text_low,
            hovered: colors.raised,
            pressed: colors.raised,
            ..Self::raised()
        }
    }

    /// The one action of a screen, on the accent.
    pub fn accent() -> Self {
        let colors = theme::colors();
        Self {
            fill: colors.accent,
            foreground: colors.on_accent,
            border: None,
            hovered: colors.accent_soft,
            pressed: colors.accent_deep,
        }
    }

    /// A control that stays on: the accent's wash, bordered in its deep tone.
    pub fn on() -> Self {
        let colors = theme::colors();
        Self {
            fill: colors.accent_wash,
            foreground: colors.accent_soft,
            border: Some(colors.accent_deep),
            hovered: colors.accent_wash,
            pressed: colors.accent_wash,
        }
    }
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
    let sense = sense_for(enabled);
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
    let fill = match (enabled, response.is_pointer_button_down_on()) {
        (false, _) => skin.fill,
        (true, true) => skin.pressed,
        (true, false) => hover_mix(ui, response, skin.fill, skin.hovered),
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
