//! One line of a picker's open list, the eye of a group on it, and the list's foot; see
//! `docs/design/ui-design-system.md`, Picker line.

use egui::{Align2, Color32, Rect, Response, Sense, Ui, pos2, vec2};

use super::button::sense_for;
use super::focus_ring;
use crate::ui::{icon, theme};

/// What one line of the list says.
pub struct PickerLine<'a> {
    pub name: &'a str,
    /// The colour of the group the line stands for, as a dot before its name.
    pub dot: Option<Color32>,
    /// How many the line holds, at its right.
    pub count: Option<usize>,
    pub selected: bool,
    /// How far in the line starts, points.
    pub indent: f32,
    /// A heading only names what is under it, and takes no click.
    pub heading: bool,
    /// A hidden group's line, its name quiet.
    pub quiet: bool,
}

/// A drawn line: its response, whether the pointer is on it, and the room at its right
/// kept for an eye and a bin.
pub struct PickerLineDrawn {
    pub response: Response,
    pub hovered: bool,
    pub icons: Rect,
}

/// A line's height, and the square of an eye or a bin on it, points.
const PICKER_LINE_H: f32 = 32.0;
pub const PICKER_ICON_SQUARE: f32 = 26.0;

/// A line's room at its ends, the square its dot stands in, the room between its parts,
/// the least a count takes, and a glyph of the foot, points.
const LEFT_PAD: f32 = 10.0;
const RIGHT_PAD: f32 = 4.0;
const DOT_W: f32 = 8.0;
const DOT_R: f32 = 4.0;
const GAP: f32 = 8.0;
const COUNT_W: f32 = 20.0;
const GLYPH_W: f32 = 14.0;
const ICONS_W: f32 = 2.0 * PICKER_ICON_SQUARE;
const ICON_GLYPH: f32 = 15.0;
/// The foot's height.
const FOOT_H: f32 = 34.0;

/// Draws `line` across the room: its dot, its name, its count, washed while picked or
/// under the pointer.
pub fn picker_line(ui: &mut Ui, line: &PickerLine<'_>) -> PickerLineDrawn {
    let sense = sense_for(!line.heading);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), PICKER_LINE_H), sense);
    let hovered = ui.rect_contains_pointer(rect);
    let mut right = rect.right() - RIGHT_PAD;
    let icons = Rect::from_x_y_ranges(right - ICONS_W..=right, rect.y_range());
    let count_right = icons.left() - GAP;
    if line.count.is_some() {
        right = count_right - COUNT_W - GAP;
    }
    let left = rect.left() + LEFT_PAD + line.indent + DOT_W + GAP;
    let name = Rect::from_x_y_ranges(left..=right, rect.y_range());
    paint(ui, rect, line, (name, count_right), hovered);
    PickerLineDrawn {
        response,
        hovered,
        icons,
    }
}

fn paint(
    ui: &Ui,
    rect: Rect,
    line: &PickerLine<'_>,
    (name, count_right): (Rect, f32),
    hovered: bool,
) {
    let colors = theme::colors();
    let fill = match (line.selected, hovered && !line.heading) {
        (true, _) => colors.picked_wash,
        (false, true) => colors.hover,
        (false, false) => Color32::TRANSPARENT,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let y = rect.center().y;
    if let Some(dot) = line.dot {
        let x = rect.left() + LEFT_PAD + line.indent + DOT_W / 2.0;
        painter.circle_filled(pos2(x, y), DOT_R, dot);
    }
    let ink = match (line.heading || line.quiet, line.count) {
        (true, _) => colors.text_low,
        (false, None) => colors.text_mid,
        (false, Some(_)) => colors.text_high,
    };
    if let Some(count) = line.count {
        let low = colors.text_low;
        let galley = painter.layout_no_wrap(count.to_string(), theme::figures(12.0), low);
        let at = pos2(count_right - galley.size().x, y - galley.size().y / 2.0);
        painter.galley(at, galley, low);
    }
    painter.with_clip_rect(name).text(
        name.left_center(),
        Align2::LEFT_CENTER,
        line.name,
        theme::label(),
        ink,
    );
}

/// An eye or a like glyph in `rect`, raised while `hot`.
pub fn eye_square(ui: &Ui, rect: Rect, glyph: &str, hot: bool) {
    let colors = theme::colors();
    if hot {
        ui.painter()
            .rect_filled(rect, theme::R_CONTROL, colors.raised);
    }
    let ink = if hot {
        colors.text_high
    } else {
        colors.text_mid
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(ICON_GLYPH),
        ink,
    );
}

/// The list's foot, under its rule: a glyph, what it opens, and the sign that it leads
/// elsewhere.
pub fn picker_foot(ui: &mut Ui, glyph: &str, text: &str) -> Response {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), FOOT_H), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let ink = if response.hovered() {
        colors.text_high
    } else {
        colors.text_mid
    };
    let painter = ui.painter();
    let y = rect.center().y;
    let glyph_x = rect.left() + LEFT_PAD + GLYPH_W / 2.0;
    painter.text(
        pos2(glyph_x, y),
        Align2::CENTER_CENTER,
        glyph,
        theme::icon(GLYPH_W),
        ink,
    );
    let words_at = pos2(glyph_x + GLYPH_W / 2.0 + GAP, y);
    painter.text(words_at, Align2::LEFT_CENTER, text, theme::label(), ink);
    let elsewhere_at = pos2(rect.right() - LEFT_PAD - GLYPH_W / 2.0, y);
    painter.text(
        elsewhere_at,
        Align2::CENTER_CENTER,
        icon::ELSEWHERE,
        theme::icon(GLYPH_W),
        ink,
    );
    focus_ring(ui, &response, theme::R_CONTROL);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
