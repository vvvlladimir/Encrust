use egui::{Align, Color32, Layout};

use crate::panels::{Window, toggle_settings};
use crate::shortcuts::{self, Action};
use crate::ui::{hairline, icon, icon_button, rail_button, theme};
use crate::workspace::Tool;

/// Points between the rail's edge and its buttons, and between one group and the next.
const RAIL_PAD: i8 = 6;
const GROUP_GAP: f32 = 6.0;

/// The window's right edge: every tool, one column, a rule between each group, and the
/// sheet of keys and the Settings screen at its foot.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(RAIL_PAD, RAIL_PAD))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (index, group) in Tool::RAIL.into_iter().enumerate() {
                if index > 0 {
                    ui.add_space(GROUP_GAP);
                    hairline(ui);
                    ui.add_space(GROUP_GAP);
                }
                for tool in group {
                    button(ui, window, *tool);
                }
            }
            // What belongs to the whole window rather than to a tool stands at the foot.
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                if icon_button(ui, icon::SETTINGS, &shortcuts::tooltip(Action::Settings)).clicked()
                {
                    toggle_settings(window.machine);
                }
                if icon_button(ui, icon::KEYBOARD, &shortcuts::tooltip(Action::Sheet)).clicked() {
                    window.view.options.sheet = true;
                }
            });
        });
}

fn button(ui: &mut egui::Ui, window: &mut Window, tool: Tool) {
    let active = *window.tool == tool;
    let tooltip = shortcuts::tooltip(Action::Pick(tool));
    let badge = badge(window, tool);
    if rail_button(ui, tool.glyph(), tool.label(), &tooltip, active, badge).clicked() {
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
