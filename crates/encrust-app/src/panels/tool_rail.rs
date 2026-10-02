use egui::{Align, Layout};

use crate::panels::Window;
use crate::shortcuts::{self, Action};
use crate::ui::{hairline, icon, tool_button};
use crate::workspace::Tool;

pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;

        for tool in Tool::PLACING {
            button(ui, window, tool);
        }
        ui.add_space(6.0);
        divider(ui);
        ui.add_space(6.0);
        for tool in Tool::SHAPING {
            button(ui, window, tool);
        }
        ui.add_space(6.0);
        divider(ui);
        ui.add_space(6.0);
        for tool in Tool::PRINTING {
            button(ui, window, tool);
        }
    });

    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        ui.add_space(8.0);
        let tooltip = shortcuts::tooltip(Action::OpenModel);
        if tool_button(ui, icon::ADD, &tooltip, false, true).clicked() {
            window
                .doc
                .imports
                .open_dialog(&window.doc.plate, &mut window.machine.status);
        }
    });
}

fn button(ui: &mut egui::Ui, window: &mut Window, tool: Tool) {
    let active = *window.tool == tool;
    let tooltip = shortcuts::tooltip(Action::Pick(tool));
    if tool_button(ui, tool.glyph(), &tooltip, active, true).clicked() {
        *window.tool = tool;
    }
}

/// A short rule between one group of tools and the next.
fn divider(ui: &mut egui::Ui) {
    ui.scope(|ui| {
        ui.set_width(24.0);
        hairline(ui);
    });
}
