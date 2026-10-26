use egui::Color32;

use crate::panels::Window;
use crate::shortcuts::{self, Action};
use crate::ui::{rail_button, theme, vertical_hairline};
use crate::workspace::Tool;

/// Points a tool's name keeps clear of either side of its button.
const NAME_PAD: f32 = 3.0;

/// The widths the rail stands at in full, points: with the tools' names under their
/// glyphs, and with the glyphs alone in squares. See `docs/design/ui-design-system.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailWidths {
    pub named: f32,
    pub glyphs: f32,
}

impl RailWidths {
    /// The widths for these names, `names` points wide in the rail's order.
    fn of(names: &[f32]) -> Self {
        let named: f32 = names.iter().copied().map(named_button_w).sum();
        Self {
            named: chrome() + named,
            glyphs: chrome() + names.len() as f32 * theme::RAIL_ICON_H,
        }
    }
}

/// The rail's widths in the language the window is written in.
pub fn widths(ctx: &egui::Context) -> RailWidths {
    RailWidths::of(&name_widths(ctx))
}

fn name_widths(ctx: &egui::Context) -> Vec<f32> {
    ctx.fonts_mut(|fonts| {
        Tool::on_rail()
            .map(|tool| {
                fonts
                    .layout_no_wrap(tool.label(), theme::rail_name(), Color32::WHITE)
                    .size()
                    .x
            })
            .collect()
    })
}

/// A named button is as wide as its name, and never narrower than a glyph's square.
fn named_button_w(name_w: f32) -> f32 {
    (name_w + 2.0 * NAME_PAD).max(theme::RAIL_ICON_H)
}

/// What the rail takes besides its buttons: its padding, the gaps and the rules.
fn chrome() -> f32 {
    let tools: usize = Tool::RAIL.iter().map(|group| group.len()).sum();
    let rules = Tool::RAIL.len() - 1;
    2.0 * theme::TOOLBAR_PAD
        + (tools - 1 - rules) as f32 * theme::ICON_GAP
        + rules as f32 * (2.0 * theme::BUTTON_GAP + 1.0)
}

/// How the rail fits `width`: each button's width, their height, and whether the names
/// show. Named buttons share what is over their need; glyphs stand in squares.
#[derive(Debug, Clone, PartialEq)]
struct Fit {
    buttons_w: Vec<f32>,
    button_h: f32,
    labelled: bool,
}

impl Fit {
    fn into(width: f32, names: &[f32]) -> Self {
        let widths = RailWidths::of(names);
        let count = names.len() as f32;
        // Half a point of slack, so a column sized to the names keeps them under rounding.
        if width + 0.5 >= widths.named {
            let over = (width - widths.named).max(0.0) / count;
            return Self {
                buttons_w: names.iter().map(|&w| named_button_w(w) + over).collect(),
                button_h: theme::RAIL_BUTTON_H,
                labelled: true,
            };
        }
        let side = ((width - chrome()) / count).clamp(0.0, theme::RAIL_ICON_H);
        Self {
            buttons_w: vec![side; names.len()],
            button_h: side,
            labelled: false,
        }
    }

    /// What is left of `width` once the buttons stand, split either side of them.
    fn margin(&self, width: f32) -> f32 {
        let used: f32 = chrome() + self.buttons_w.iter().sum::<f32>();
        ((width - used) / 2.0).max(0.0)
    }
}

/// The card over the inspector, `width` points wide: every tool in one row, in the order a
/// plate is worked, a rule between each group (ADR 0348).
pub fn ui(ui: &mut egui::Ui, window: &mut Window, width: f32) {
    let fit = Fit::into(width, &name_widths(ui.ctx()));
    ui.set_width(width);
    egui::Frame::new()
        .inner_margin(theme::TOOLBAR_PAD)
        .show(ui, |ui| tools(ui, window, &fit, fit.margin(width)));
}

fn tools(ui: &mut egui::Ui, window: &mut Window, fit: &Fit, margin: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(margin);
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
                ui.allocate_ui(size, |ui| button(ui, window, *tool, fit.labelled));
            }
        }
    });
}

fn button(ui: &mut egui::Ui, window: &mut Window, tool: Tool, labelled: bool) {
    let active = window.tool.card() == tool;
    let tooltip = shortcuts::tooltip(Action::Pick(tool));
    let badge = badge(window, tool);
    let glyph = tool.glyph();
    if rail_button(ui, glyph, &tool.label(), labelled, &tooltip, active, badge).clicked() {
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

    /// Nine names as wide as the English ones, in points.
    const NAMES: [f32; 9] = [28.0, 40.0, 33.0, 33.0, 26.0, 17.0, 28.0, 43.0, 30.0];

    #[test]
    fn a_rail_as_wide_as_its_names_shows_them_and_shares_what_is_over() {
        let named = RailWidths::of(&NAMES).named;
        let fit = Fit::into(named + 16.0, &NAMES);
        assert!(fit.labelled);
        let used = chrome() + fit.buttons_w.iter().sum::<f32>();
        assert!(
            (used - (named + 16.0)).abs() < 1e-3,
            "expected the buttons to fill {} points, they take {used}",
            named + 16.0
        );
        assert_eq!(fit.button_h, theme::RAIL_BUTTON_H);
    }

    #[test]
    fn a_rail_short_of_its_names_stands_its_glyphs_in_squares() {
        let widths = RailWidths::of(&NAMES);
        let fit = Fit::into(widths.named - 1.0, &NAMES);
        assert!(!fit.labelled);
        assert!(
            fit.buttons_w
                .iter()
                .all(|&w| (w - fit.button_h).abs() < 1e-4),
            "every button a square, got {:?} by {}",
            fit.buttons_w,
            fit.button_h
        );
        let squeezed = Fit::into(widths.glyphs - 18.0, &NAMES);
        let side = theme::RAIL_ICON_H - 2.0;
        assert!(
            (squeezed.button_h - side).abs() < 1e-4,
            "18 points short of nine glyphs takes 2 off each: {side} expected, {} got",
            squeezed.button_h
        );
    }

    /// The rail's names, as the window's own fonts set them in `language`.
    fn widths_in(language: crate::i18n::Language) -> RailWidths {
        let keys = [
            "tool-select",
            "tool-measure",
            "tool-repair",
            "tool-hollow",
            "tool-drain",
            "tool-cut",
            "tool-relief",
            "tool-supports",
            "tool-check",
        ];
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::ui::fonts::definitions());
        let mut names = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            names = keys
                .iter()
                .map(|key| {
                    let name = crate::i18n::message(language, key, None);
                    ui.painter()
                        .layout_no_wrap(name, theme::rail_name(), Color32::WHITE)
                        .size()
                        .x
                })
                .collect();
        });
        output.textures_delta.clear();
        RailWidths::of(&names)
    }

    #[test]
    fn every_language_names_its_tools_within_the_widest_column() {
        for language in crate::i18n::Language::ALL {
            let named = widths_in(language).named;
            let widest = *theme::INSPECTOR_W_RANGE.end();
            assert!(
                named <= widest,
                "{language:?} needs {named} points for the rail's names, the column takes {widest}"
            );
        }
    }

    #[test]
    fn glyphs_with_room_to_spare_stand_centred() {
        let widths = RailWidths::of(&NAMES);
        let fit = Fit::into(widths.glyphs + 20.0, &NAMES);
        assert_eq!(fit.button_h, theme::RAIL_ICON_H);
        assert!((fit.margin(widths.glyphs + 20.0) - 10.0).abs() < 1e-4);
    }
}
