use egui::Color32;

use crate::panels::Window;
use crate::shortcuts::{self, Action};
use crate::ui::{RailButton, rail_button, theme, vertical_hairline};
use crate::workspace::Tool;

/// The least a named button is wide: narrower, the names go (`ui-screens.md`, Tool row).
const NAMED_MIN_W: f32 = 56.0;

/// The most a named button is wide: its height, or the least a name needs if that is more.
const NAMED_MAX_W: f32 = theme::RAIL_BUTTON_H.max(NAMED_MIN_W);

/// What the row takes besides its buttons: its padding, the gaps and the rules.
fn chrome() -> f32 {
    let tools: usize = Tool::RAIL.iter().map(|group| group.len()).sum();
    let rules = Tool::RAIL.len() - 1;
    2.0 * theme::TOOLBAR_PAD
        + (tools - 1 - rules) as f32 * theme::ICON_GAP
        + rules as f32 * (2.0 * theme::BUTTON_GAP + 1.0)
}

/// How the row fits `width`: named buttons share it up to `NAMED_MAX_W` each; under
/// `NAMED_MIN_W` each, the glyphs stand alone in squares.
#[derive(Debug, Clone, PartialEq)]
struct Fit {
    buttons_w: Vec<f32>,
    button_h: f32,
    labelled: bool,
}

impl Fit {
    fn into(width: f32, count: usize) -> Self {
        let share = ((width - chrome()) / count as f32).max(0.0);
        if share >= NAMED_MIN_W {
            return Self {
                buttons_w: vec![share.min(NAMED_MAX_W); count],
                button_h: theme::RAIL_BUTTON_H,
                labelled: true,
            };
        }
        let side = share.min(theme::RAIL_ICON_H);
        Self {
            buttons_w: vec![side; count],
            button_h: side,
            labelled: false,
        }
    }

    /// The row's width with its buttons standing, padding included.
    fn width(&self) -> f32 {
        chrome() + self.buttons_w.iter().sum::<f32>()
    }
}

/// How wide the tool row is given `room`: as wide as its buttons, never wider than `room`.
fn width(room: f32) -> f32 {
    Fit::into(room, Tool::on_rail().count()).width().min(room)
}

/// The tool row over the stage, at most `room` points wide: every tool in the order a
/// plate is worked, a rule between each group (ADR 0382).
pub fn ui(ui: &mut egui::Ui, window: &mut Window, room: f32) {
    let fit = Fit::into(room, Tool::on_rail().count());
    ui.set_width(width(room));
    // The row's rect is last frame's: the digits show while the pointer is anywhere on it.
    let row_id = ui.id().with("tool-row");
    let last_row = ui.data(|data| data.get_temp::<egui::Rect>(row_id));
    let keys = last_row.is_some_and(|row| ui.rect_contains_pointer(row));
    let row = egui::Frame::new()
        .inner_margin(theme::TOOLBAR_PAD)
        .show(ui, |ui| tools(ui, window, &fit, keys))
        .response
        .rect;
    ui.data_mut(|data| data.insert_temp(row_id, row));
}

fn tools(ui: &mut egui::Ui, window: &mut Window, fit: &Fit, keys: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let mut widths = fit.buttons_w.iter();
        for (index, group) in Tool::RAIL.into_iter().enumerate() {
            if index > 0 {
                ui.add_space(theme::BUTTON_GAP);
                vertical_hairline(ui, fit.button_h);
                ui.add_space(theme::BUTTON_GAP);
            }
            for (place, (tool, &w)) in group.iter().zip(&mut widths).enumerate() {
                if place > 0 {
                    ui.add_space(theme::ICON_GAP);
                }
                let size = egui::vec2(w, fit.button_h);
                ui.allocate_ui(size, |ui| button(ui, window, *tool, fit.labelled, keys));
            }
        }
    });
}

fn button(ui: &mut egui::Ui, window: &mut Window, tool: Tool, labelled: bool, keys: bool) {
    let (name, tooltip) = (tool.label(), shortcuts::tooltip(Action::Pick(tool)));
    let key = keys.then(|| shortcuts::text(Action::Pick(tool)));
    let button = RailButton {
        glyph: tool.glyph(),
        name: &name,
        tooltip: &tooltip,
        labelled,
        active: window.tool.card() == tool,
        badge: badge(window, tool),
        key: key.as_deref(),
    };
    if rail_button(ui, &button).clicked() {
        shortcuts::pick(window, tool);
    }
}

/// The dot a tool carries while it has something the user must deal with. Only Repair,
/// Drain and Check carry one, so a dot is never noise; see `docs/decisions/0218`, `0354`.
fn badge(window: &Window, tool: Tool) -> Option<Color32> {
    let colors = theme::colors();
    match tool {
        Tool::Repair => {
            (crate::repair::broken_on_the_plate(&window.doc.scene) > 0).then_some(colors.warn)
        }
        Tool::Drain => window
            .doc
            .scene
            .targets()
            .any(|object| !object.traps.found().is_empty())
            .then_some(colors.danger),
        Tool::Check => window
            .measured()
            .and_then(core_analysis::Measured::worst)
            .map(|worst| super::inspector::risk_tint(&worst)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The row's width at which each of nine buttons gets `share`.
    fn row_w(share: f32) -> f32 {
        chrome() + 9.0 * share
    }

    #[test]
    fn a_row_short_of_room_shares_it_equally_among_named_buttons() {
        let share = (NAMED_MIN_W + NAMED_MAX_W) / 2.0;
        let fit = Fit::into(row_w(share), 9);
        assert!(fit.labelled);
        assert!(
            fit.buttons_w.iter().all(|&w| (w - share).abs() < 1e-3),
            "{:?}",
            fit.buttons_w
        );
        assert_eq!(fit.button_h, theme::RAIL_BUTTON_H);
    }

    #[test]
    fn under_fifty_six_points_a_tool_the_names_go() {
        let fit = Fit::into(row_w(NAMED_MIN_W - 1.0), 9);
        assert!(!fit.labelled);
        assert!(
            fit.buttons_w
                .iter()
                .all(|&w| (w - theme::RAIL_ICON_H).abs() < 1e-4),
            "every button a {} square, got {:?}",
            theme::RAIL_ICON_H,
            fit.buttons_w
        );
    }

    #[test]
    fn a_row_short_of_nine_squares_shrinks_them_and_never_below_nought() {
        let squeezed = Fit::into(row_w(theme::RAIL_ICON_H - 2.0), 9);
        assert!((squeezed.button_h - (theme::RAIL_ICON_H - 2.0)).abs() < 1e-4);
        assert_eq!(Fit::into(0.0, 9).button_h, 0.0);
    }

    #[test]
    fn named_buttons_grow_no_wider_than_their_cap() {
        let room = row_w(NAMED_MAX_W) + 400.0;
        let fit = Fit::into(room, 9);
        assert!(
            fit.buttons_w
                .iter()
                .all(|&w| (w - NAMED_MAX_W).abs() < 1e-4),
            "every button {NAMED_MAX_W} wide, got {:?}",
            fit.buttons_w
        );
        assert!((width(room) - row_w(NAMED_MAX_W)).abs() < 1e-3);
    }

    #[test]
    fn glyphs_with_room_to_spare_take_only_their_squares() {
        let room = row_w(theme::RAIL_ICON_H) + 20.0;
        assert!(!Fit::into(room, 9).labelled);
        assert!((width(room) - row_w(theme::RAIL_ICON_H)).abs() < 1e-3);
    }

    /// What the row is given in a window of `width`: the stage between the inspector and
    /// the models card at their defaults, less the layer strip, the gaps and the outlines.
    fn row_in_window(width: f32) -> f32 {
        let cards = theme::INSPECTOR_W + theme::SCENE_W + theme::LAYER_STRIP_W;
        width - cards - 5.0 * theme::FLOAT_GAP - 6.0
    }

    #[test]
    fn the_names_show_in_a_window_of_1280_and_go_in_the_least_one() {
        assert!(Fit::into(row_in_window(1280.0), 9).labelled);
        assert!(!Fit::into(row_in_window(theme::WINDOW_MIN_W), 9).labelled);
    }
}
