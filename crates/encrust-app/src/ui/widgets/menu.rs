//! The File, Edit and View menus: a row with its glyph, name and keys, a reason under one that
//! cannot run, a row that opens a submenu, and the rule between groups. See
//! `docs/design/ui-design-system.md`, Menu.

use egui::containers::menu::SubMenu;
use egui::{Align2, Color32, Rect, Response, Sense, Ui, WidgetInfo, WidgetType};
use egui::{pos2, vec2};

use super::{focus_ring, hover_mix};
use crate::ui::{icon, theme};

/// One row of a menu. `keys` is the shortcut, or a figure such as when a file was opened.
pub struct MenuRow<'a> {
    pub glyph: Option<&'a str>,
    pub name: &'a str,
    pub keys: Option<String>,
    /// Why the row cannot run, printed under it while it is off.
    pub why: Option<&'a str>,
    pub enabled: bool,
    /// A setting that is on, or the one of a choice in force: a tick where the glyph stands.
    pub ticked: bool,
}

impl<'a> MenuRow<'a> {
    pub fn new(glyph: Option<&'a str>, name: &'a str) -> Self {
        Self {
            glyph,
            name,
            keys: None,
            why: None,
            enabled: true,
            ticked: false,
        }
    }

    pub fn keys(self, keys: impl Into<String>) -> Self {
        let keys = keys.into();
        Self {
            keys: (!keys.is_empty()).then_some(keys),
            ..self
        }
    }

    pub fn enabled(self, enabled: bool) -> Self {
        Self { enabled, ..self }
    }

    /// Off unless `why` is `None`, and saying why when it is not.
    pub fn unless(self, why: Option<&'a str>) -> Self {
        Self {
            enabled: why.is_none(),
            why,
            ..self
        }
    }

    pub fn ticked(self, ticked: bool) -> Self {
        Self { ticked, ..self }
    }
}

/// Draws a [`MenuRow`]; a press closes the menu.
pub fn menu_row(ui: &mut Ui, row: &MenuRow<'_>) -> Response {
    row_with(ui, row, false)
}

/// A [`MenuRow`] with a caret that opens `add` beside it on hover; nothing while it is off.
pub fn menu_sub<R>(ui: &mut Ui, row: &MenuRow<'_>, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let response = row_with(ui, row, true);
    if !row.enabled {
        return None;
    }
    SubMenu::new()
        .show(ui, &response, |ui| {
            ui.set_width(SUB_W);
            add(ui)
        })
        .map(|shown| shown.inner)
}

/// The hairline between two groups of a menu.
pub fn menu_rule(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), RULE_H), Sense::hover());
    let y = rect.center().y;
    ui.painter().hline(
        rect.x_range().shrink(PAD),
        y,
        egui::Stroke::new(1.0, theme::colors().hairline),
    );
}

/// What every menu of the bar is dressed in: `raised` ringed in `line`, rows a point apart.
pub fn menu_style(style: &mut egui::Style) {
    let colors = theme::colors();
    style.visuals.window_fill = colors.raised;
    style.visuals.window_stroke = egui::Stroke::new(1.0, colors.line);
    style.visuals.menu_corner_radius = theme::R_SURFACE;
    style.spacing.menu_margin = egui::Margin::same(MARGIN);
    style.spacing.item_spacing.y = 0.0;
}

/// How wide a menu of the bar stands at least, and a submenu, points.
pub const MENU_W: f32 = 264.0;
const SUB_W: f32 = 200.0;

fn row_with(ui: &mut Ui, row: &MenuRow<'_>, caret: bool) -> Response {
    let colors = theme::colors();
    let width = ui.available_width();
    let why = row.why.filter(|_| !row.enabled).map(|why| {
        let wrap = width - NAME_X - PAD;
        ui.painter()
            .layout(why.to_owned(), theme::small(), colors.text_low, wrap)
    });
    let height = ROW_H + why.as_ref().map_or(0.0, |galley| galley.size().y + WHY_GAP);
    let sense = if row.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, row.enabled, row.name));
    let fill = match row.enabled {
        true => hover_mix(ui, &response, Color32::TRANSPARENT, colors.hover),
        false => Color32::TRANSPARENT,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let line = Rect::from_min_size(rect.min, vec2(width, ROW_H));
    let glyph_ink = match (row.enabled, response.hovered()) {
        (false, _) => colors.text_low.gamma_multiply(DIM),
        (true, true) => colors.text_high,
        (true, false) => colors.text_low,
    };
    paint_left(painter, line, row, glyph_ink);
    paint_right(painter, line, row, caret, glyph_ink);
    if let Some(galley) = why {
        let at = pos2(line.left() + NAME_X, line.bottom() - WHY_GAP / 2.0);
        painter.galley(at, galley, colors.text_low);
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response
}

/// The glyph, or the tick of what is on, and the name.
fn paint_left(painter: &egui::Painter, line: Rect, row: &MenuRow<'_>, glyph_ink: Color32) {
    let colors = theme::colors();
    let y = line.center().y;
    let glyph_at = pos2(line.left() + PAD + GLYPH_W / 2.0, y);
    let glyph = match (row.ticked, row.glyph) {
        (true, _) => Some((icon::IN_HAND, colors.accent_soft)),
        (false, Some(glyph)) => Some((glyph, glyph_ink)),
        (false, None) => None,
    };
    if let Some((glyph, ink)) = glyph {
        let font = theme::icon(GLYPH_PT);
        painter.text(glyph_at, Align2::CENTER_CENTER, glyph, font, ink);
    }
    let ink = if row.enabled {
        colors.text_high
    } else {
        colors.text_low
    };
    let name_at = pos2(line.left() + NAME_X, y);
    painter.text(name_at, Align2::LEFT_CENTER, row.name, theme::label(), ink);
}

/// The caret of a submenu, or the keys.
fn paint_right(painter: &egui::Painter, line: Rect, row: &MenuRow<'_>, caret: bool, ink: Color32) {
    let right = pos2(line.right() - PAD, line.center().y);
    if caret {
        let font = theme::icon(KEYS_PT);
        painter.text(right, Align2::RIGHT_CENTER, icon::CARET_RIGHT, font, ink);
    } else if let Some(keys) = &row.keys {
        let colors = theme::colors();
        let keys_ink = if row.enabled {
            colors.text_low
        } else {
            colors.text_low.gamma_multiply(DIM)
        };
        let font = theme::figures(KEYS_PT);
        painter.text(right, Align2::RIGHT_CENTER, keys, font, keys_ink);
    }
}

/// Points: a row's height, the room at its ends, the square its glyph stands in and where
/// its name starts; a rule's height; the gap between a row and the reason under it.
const ROW_H: f32 = 28.0;
const PAD: f32 = 8.0;
const GLYPH_W: f32 = 18.0;
const NAME_X: f32 = PAD + GLYPH_W + 8.0;
const RULE_H: f32 = 9.0;
const WHY_GAP: f32 = 4.0;
const MARGIN: i8 = 4;
const GLYPH_PT: f32 = 15.0;
const KEYS_PT: f32 = 12.0;
/// What a glyph and keys keep of their ink on a row that is off.
const DIM: f32 = 0.6;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::fixtures;

    #[test]
    fn a_row_that_cannot_run_says_why_under_itself_and_takes_no_press() {
        let ctx = fixtures::context();
        let (mut free, mut off) = (None, None);
        fixtures::frame(&ctx, egui::RawInput::default(), |ui| {
            ui.set_width(MENU_W);
            free = Some(menu_row(ui, &MenuRow::new(Some(icon::SLICE), "Slice")));
            let row = MenuRow::new(Some(icon::SLICE), "Slice").unless(Some("Add a model first"));
            off = Some(menu_row(ui, &row));
        });
        let (free, off) = (free.expect("drawn"), off.expect("drawn"));
        assert_eq!(
            free.rect.height(),
            ROW_H,
            "a row that runs is one line, 28 points"
        );
        assert!(off.rect.height() > ROW_H, "the reason stands under the row");
        assert!(
            !off.sense.senses_click(),
            "a row that is off takes no press"
        );
    }
}
