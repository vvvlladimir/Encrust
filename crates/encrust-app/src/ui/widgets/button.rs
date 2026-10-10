use egui::{Align2, Color32, Rect, Response, Sense, StrokeKind, Ui, vec2};

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

/// A rail button: a glyph over the tool's name, raised with both in the accent while it is
/// the tool in use, and a dot in `badge` at its corner while the tool needs attention.
pub fn rail_button(
    ui: &mut Ui,
    glyph: &str,
    name: &str,
    tooltip: &str,
    active: bool,
    badge: Option<Color32>,
) -> Response {
    let size = vec2(ui.available_width(), theme::RAIL_BUTTON_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let colors = theme::colors();
    let (fill, foreground) = match (active, response.hovered()) {
        (true, _) => (colors.raised, colors.accent_soft),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (Color32::TRANSPARENT, colors.text_mid),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    painter.text(
        rect.center_top() + vec2(0.0, 16.0),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(17.0),
        foreground,
    );
    painter.text(
        rect.center_bottom() - vec2(0.0, 9.0),
        Align2::CENTER_CENTER,
        name,
        theme::rail_name(),
        foreground,
    );
    if let Some(badge) = badge {
        let at = rect.center_top() + vec2(RAIL_BADGE_OFFSET.x, RAIL_BADGE_OFFSET.y);
        painter.circle_filled(at, RAIL_BADGE_R + 1.5, colors.base);
        painter.circle_filled(at, RAIL_BADGE_R, badge);
    }
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

    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

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

/// The one action a screen is for. There is never a second one of these in view.
pub fn primary_button(ui: &mut Ui, glyph: &str, text: &str, enabled: bool) -> Response {
    let colors = theme::colors();
    let (fill, foreground) = if enabled {
        (colors.accent, colors.on_accent)
    } else {
        (colors.raised, colors.text_low)
    };
    filled_button(
        ui,
        glyph,
        text,
        vec2(ui.available_width(), theme::PRIMARY_H),
        &Skin {
            fill,
            foreground,
            border: None,
            hovered: colors.accent_soft,
            pressed: colors.accent_deep,
        },
        enabled,
    )
}

/// A row that opens a chooser: a leading glyph, what is chosen, and a caret.
pub fn picker(ui: &mut Ui, glyph: &str, label: &str) -> Response {
    let colors = theme::colors();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());

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
    let galley = painter.layout(
        label.to_owned(),
        theme::label(),
        colors.text_high,
        text_rect.width(),
    );
    painter.galley(
        egui::pos2(
            text_rect.left(),
            text_rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        colors.text_high,
    );

    response
}

/// Space kept clear either side of a button's label.
const LABEL_PAD: f32 = 10.0;

/// What a text button is painted in, in each of the states it can be in.
struct Skin {
    fill: Color32,
    foreground: Color32,
    border: Option<Color32>,
    hovered: Color32,
    pressed: Color32,
}

/// One drawing behind every text button: a filled rounded rect, a glyph and a label.
fn filled_button(
    ui: &mut Ui,
    glyph: &str,
    text: &str,
    size: egui::Vec2,
    skin: &Skin,
    enabled: bool,
) -> Response {
    let Skin {
        fill,
        foreground,
        border,
        hovered,
        pressed,
    } = *skin;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);

    let fill = match (
        enabled,
        response.is_pointer_button_down_on(),
        response.hovered(),
    ) {
        (false, _, _) => fill,
        (true, true, _) => pressed,
        (true, false, true) => hovered,
        (true, false, false) => fill,
    };

    let painter = ui.painter();
    let stroke = border.map_or(egui::Stroke::NONE, |border| egui::Stroke::new(1.0, border));
    painter.rect(rect, theme::R_CONTROL, fill, stroke, StrokeKind::Inside);

    let font = theme::label();
    let glyph_width = if glyph.is_empty() { 0.0 } else { 22.0 };
    // A label longer than its button is cut rather than drawn past the edge: the text
    // comes from a file name or a printer, and neither has a length we chose.
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, foreground);
    job.wrap = egui::text::TextWrapping::truncate_at_width(
        (size.x - glyph_width - 2.0 * LABEL_PAD).max(0.0),
    );
    let galley = painter.layout_job(job);
    let content_left = rect.center().x - (glyph_width + galley.size().x) / 2.0;
    if !glyph.is_empty() {
        painter.text(
            egui::pos2(content_left + 8.0, rect.center().y),
            Align2::CENTER_CENTER,
            glyph,
            theme::icon(16.0),
            foreground,
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
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Height of a summary card: two lines of text and room around them.
const SUMMARY_H: f32 = 52.0;
