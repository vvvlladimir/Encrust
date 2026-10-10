use egui::{Align, Layout};

use crate::panels::{Window, toggle_settings};
use crate::shortcuts::{self, Action};
use crate::ui::{hairline, icon, icon_button, rail_button};
use crate::workspace::{Mode, Tool};

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
            let groups: [&[Tool]; 3] = [&Tool::PLACING, &Tool::SHAPING, &Tool::PRINTING];
            for (index, group) in groups.into_iter().enumerate() {
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

/// A tool is lit only while the plate is being edited with it: the layer views have no
/// tool in hand, and a press on one brings the model back.
fn button(ui: &mut egui::Ui, window: &mut Window, tool: Tool) {
    let active = *window.tool == tool && *window.mode == Mode::Prepare;
    let tooltip = shortcuts::tooltip(Action::Pick(tool));
    if rail_button(ui, tool.glyph(), tool.rail_name(), &tooltip, active).clicked() {
        shortcuts::pick(window, tool);
    }
}
