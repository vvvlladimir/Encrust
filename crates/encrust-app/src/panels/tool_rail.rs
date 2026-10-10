use egui::Color32;

use crate::panels::Window;
use crate::shortcuts::{self, Action};
use crate::ui::{hairline, rail_button, theme};
use crate::workspace::Tool;

/// Points between the rail's edge and its buttons.
const RAIL_PAD: f32 = 6.0;
/// The room between two buttons of a group, and either side of the rule between groups:
/// the least the rail squeezes them to, and the most a tall screen spreads them to.
const BUTTON_GAP: (f32, f32) = (2.0, 8.0);
const GROUP_GAP: (f32, f32) = (4.0, 14.0);

/// How the rail fits the height it has: whether the tools' names show, and how far apart
/// the buttons stand.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fit {
    labelled: bool,
    button_gap: f32,
    group_gap: f32,
}

impl Fit {
    /// The roomiest fit in `room` points: names first, then the gaps spread up to their
    /// most; short of room the names go, then the gaps close, and past that the rail
    /// scrolls at its tightest.
    fn into(room: f32) -> Self {
        let tight = |labelled| Self {
            labelled,
            button_gap: BUTTON_GAP.0,
            group_gap: GROUP_GAP.0,
        };
        for labelled in [true, false] {
            let least = tight(labelled).height();
            let most = Self {
                button_gap: BUTTON_GAP.1,
                group_gap: GROUP_GAP.1,
                ..tight(labelled)
            }
            .height();
            if least <= room {
                let t = ((room - least) / (most - least)).clamp(0.0, 1.0);
                return Self {
                    labelled,
                    button_gap: egui::lerp(BUTTON_GAP.0..=BUTTON_GAP.1, t),
                    group_gap: egui::lerp(GROUP_GAP.0..=GROUP_GAP.1, t),
                };
            }
        }
        tight(false)
    }

    /// How tall the tools stand at this fit, the rail's padding included.
    fn height(&self) -> f32 {
        let tools: usize = Tool::RAIL.iter().map(|group| group.len()).sum();
        let rules = Tool::RAIL.len() - 1;
        let button_h = if self.labelled {
            theme::RAIL_BUTTON_H
        } else {
            theme::RAIL_ICON_H
        };
        2.0 * RAIL_PAD
            + tools as f32 * button_h
            + (tools - 1 - rules) as f32 * self.button_gap
            + rules as f32 * (2.0 * self.group_gap + 1.0)
    }
}

/// The card at the stage's left edge, `max_h` points tall whatever it holds: every tool,
/// one column, in the order a plate is worked, a rule between each group.
pub fn ui(ui: &mut egui::Ui, window: &mut Window, max_h: f32) {
    let fit = Fit::into(max_h);
    ui.set_width(theme::RAIL_W);
    ui.set_height(max_h);
    egui::ScrollArea::vertical()
        .max_height(max_h)
        .auto_shrink([false, true])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(RAIL_PAD)
                .show(ui, |ui| tools(ui, window, fit));
        });
}

fn tools(ui: &mut egui::Ui, window: &mut Window, fit: Fit) {
    ui.spacing_mut().item_spacing.y = 0.0;
    for (index, group) in Tool::RAIL.into_iter().enumerate() {
        if index > 0 {
            ui.add_space(fit.group_gap);
            hairline(ui);
            ui.add_space(fit.group_gap);
        }
        for (place, tool) in group.iter().enumerate() {
            if place > 0 {
                ui.add_space(fit.button_gap);
            }
            button(ui, window, *tool, fit.labelled);
        }
    }
}

fn button(ui: &mut egui::Ui, window: &mut Window, tool: Tool, labelled: bool) {
    let active = *window.tool == tool;
    let tooltip = shortcuts::tooltip(Action::Pick(tool));
    let badge = badge(window, tool);
    let name = labelled.then(|| tool.label());
    if rail_button(ui, tool.glyph(), name, &tooltip, active, badge).clicked() {
        shortcuts::pick(window, tool);
    }
}

/// The dot a tool carries while it has something the user must deal with. Only the tools
/// that finish a plate carry one, so a dot is never noise; see `docs/decisions/0218`.
fn badge(window: &Window, tool: Tool) -> Option<Color32> {
    let colors = theme::colors();
    match tool {
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
        Tool::Export => window
            .machine
            .network
            .sent
            .is_some()
            .then_some(colors.accent),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tall_screen_spreads_the_tools_no_further_than_the_widest_gaps() {
        let fit = Fit::into(5000.0);
        assert!(fit.labelled);
        assert_eq!((fit.button_gap, fit.group_gap), (BUTTON_GAP.1, GROUP_GAP.1));
    }

    #[test]
    fn a_short_screen_drops_the_names_before_the_rail_scrolls() {
        let named = Fit {
            labelled: true,
            button_gap: BUTTON_GAP.0,
            group_gap: GROUP_GAP.0,
        };
        let fit = Fit::into(named.height() - 1.0);
        assert!(!fit.labelled, "one point short of the names fitting");
        assert!(
            fit.height() <= named.height() - 1.0,
            "and the bare glyphs fit"
        );
    }

    #[test]
    fn past_the_tightest_fit_the_rail_stays_at_it() {
        let fit = Fit::into(10.0);
        assert!(!fit.labelled);
        assert_eq!((fit.button_gap, fit.group_gap), (BUTTON_GAP.0, GROUP_GAP.0));
    }
}
