mod inspector;
mod mask_pane;
mod plate_bar;
mod report;
mod scene_panel;
pub(crate) mod section;
mod settings;
mod shortcuts_sheet;
mod stage;
mod status_bar;
mod support_diagram;
mod support_fields;
mod title_bar;
mod tool_rail;
mod view_column;
mod viewport_panel;

pub use inspector::slice_this_plate;
pub use scene_panel::duplicate_selection;
pub use section::animate as animate_preview;
pub use title_bar::toggle_settings;
pub use viewport_panel::frame_view;

use crate::state::{Doc, Machine, Tools, View};
use crate::ui::theme;
use crate::workspace::{Mode, Tool};

/// Everything the chrome draws against, borrowed for one frame.
///
/// The panels borrow the window's state rather than owning it so that the application
/// struct stays the single owner of the scene, and so that a panel is a plain function of
/// that state; see `docs/design/ui-design-system.md`.
pub struct Window<'a> {
    pub mode: &'a mut Mode,
    pub tool: &'a mut Tool,
    pub doc: &'a mut Doc,
    pub view: &'a mut View,
    pub tools: &'a mut Tools,
    pub machine: &'a mut Machine,
}

impl Window<'_> {
    /// What the previewed stack cures, once measured for the printer and resin in hand.
    pub fn measured(&self) -> Option<&core_analysis::Measured> {
        let settings = self.machine.slicing.raster_settings()?;
        self.machine
            .preview
            .measured(&settings, self.machine.slicing.fold())
    }

    /// Lays the window out: the strips top and bottom, the plate down the left, the
    /// inspector and the rail down the right, and the stage in what is left. Fixed, in
    /// that order; see `docs/decisions/0025`, `0102` and `0103`.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("title")
            .exact_size(theme::TITLE_H)
            .frame(strip_frame())
            .show(ui, |ui| title_bar::ui(ui, self));

        egui::Panel::bottom("status")
            .exact_size(theme::STATUS_H)
            .frame(strip_frame())
            .show(ui, |ui| {
                let tally = status_bar::tally(ui.ctx(), &self.doc.scene);
                status_bar::ui(
                    ui,
                    &self.machine.status,
                    &tally,
                    &self.machine.slicing.material,
                );
            });

        // Over everything, the Settings screen included: the keys it lists belong to both
        // halves of the window.
        if self.view.options.sheet {
            shortcuts_sheet::ui(ui.ctx(), &mut self.view.options.sheet);
        }
        report::ui(ui.ctx(), &self.doc.scene, self.tools, self.machine);
        support_diagram::ui(ui.ctx(), self);
        settings::calculators(ui.ctx(), self.machine);
        settings::confirm(ui.ctx(), self);

        // The Settings screen takes the whole window under the strips: a profile is
        // edited instead of the plate, not beside it.
        if self.machine.settings.open {
            egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ui, |ui| settings::ui(ui, self));
            return;
        }

        egui::Panel::top("plates")
            .exact_size(theme::PLATE_BAR_H)
            .frame(strip_frame())
            .show(ui, |ui| plate_bar::ui(ui, self.doc));

        // Preview reads the stack rather than editing the plate, so neither the rail nor
        // the plate panel has anything to offer it, and the stage takes their room.
        let editing = *self.mode == Mode::Prepare;

        // The rail goes in before the inspector so that it ends up the outermost of the
        // two: a tool and the panel it owns are one control and belong side by side.
        if editing {
            egui::Panel::right("rail")
                .exact_size(theme::RAIL_W)
                .resizable(false)
                .frame(panel_frame())
                .show(ui, |ui| tool_rail::ui(ui, self));
        }

        let inspector = egui::Panel::right("inspector")
            .exact_size(self.view.options.inspector_w)
            .resizable(false)
            .frame(panel_frame())
            .show(ui, |ui| inspector::ui(ui, self))
            .response
            .rect;

        let mut plate = None;
        if editing && self.view.options.plate_panel {
            plate = Some(
                egui::Panel::left("plate")
                    .exact_size(self.view.options.plate_w)
                    .resizable(false)
                    .frame(panel_frame())
                    .show(ui, |ui| scene_panel::ui(ui, self))
                    .response
                    .rect,
            );
        } else if editing {
            folded_plate_edge(ui, &mut self.view.options.plate_panel);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| stage::ui(ui, self));

        // The handles go on last so that a scroll area against the boundary cannot eat
        // the drag, which is what egui's own resizable panels do.
        if let Some(rect) = plate
            && let Some(raw) = drag_edge(ui, "plate-edge", rect, true)
        {
            match plate_width(raw) {
                Some(width) => self.view.options.plate_w = width,
                None => self.view.options.plate_panel = false,
            }
        }
        if let Some(raw) = drag_edge(ui, "inspector-edge", inspector, false) {
            self.view.options.inspector_w = raw.clamp(
                *theme::INSPECTOR_W_RANGE.start(),
                *theme::INSPECTOR_W_RANGE.end(),
            );
        }
    }
}

/// Points of grab either side of a panel's inner edge.
const GRAB_W: f32 = 6.0;

/// Where a drag to `raw` points leaves the plate panel: a width, or `None` once the drag
/// has gone past the floor, which is how the panel is folded away.
fn plate_width(raw: f32) -> Option<f32> {
    (raw >= *theme::SCENE_W_RANGE.start()).then(|| raw.min(*theme::SCENE_W_RANGE.end()))
}

/// A drag on the inner edge of a column. Returns the width the pointer is asking for,
/// unclamped, so that the caller can read a drag past the floor as a fold.
fn drag_edge(ui: &egui::Ui, id: &str, panel: egui::Rect, fixed_left: bool) -> Option<f32> {
    let edge = if fixed_left {
        panel.right()
    } else {
        panel.left()
    };
    let strip = egui::Rect::from_x_y_ranges(
        egui::Rangef::new(edge - GRAB_W / 2.0, edge + GRAB_W / 2.0),
        panel.y_range(),
    );
    let response = ui.interact(strip, egui::Id::new(id), egui::Sense::drag());

    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        ui.painter().vline(
            edge,
            panel.y_range(),
            egui::Stroke::new(1.0, theme::colors().accent),
        );
    }

    let pointer = response.interact_pointer_pos()?;
    Some(if fixed_left {
        pointer.x - panel.left()
    } else {
        panel.right() - pointer.x
    })
}

/// The folded plate keeps its edge: a hairline that lights up under the pointer and
/// brings the panel back on a click.
fn folded_plate_edge(ui: &mut egui::Ui, open: &mut bool) {
    egui::Panel::left("plate-folded")
        .exact_size(theme::EDGE_W)
        .resizable(false)
        .frame(egui::Frame::new().fill(theme::colors().base))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            let response = ui.interact(rect, ui.id().with("grab"), egui::Sense::click());
            let colors = theme::colors();
            if response.hovered() {
                ui.painter().rect_filled(rect, 0.0, colors.accent);
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            } else {
                ui.painter().vline(
                    rect.center().x,
                    rect.y_range(),
                    egui::Stroke::new(1.0, colors.hairline),
                );
            }
            if response.clicked() {
                *open = true;
            }
        });
}

/// The title and status strips sit on the window's own colour, with a hairline between
/// them and the work.
fn strip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::colors().base)
        .inner_margin(theme::STRIP_MARGIN)
}

fn panel_frame() -> egui::Frame {
    egui::Frame::new().fill(theme::colors().panel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_past_the_floor_folds_the_plate_away() {
        let floor = *theme::SCENE_W_RANGE.start();
        assert_eq!(plate_width(floor - 1.0), None);
        assert_eq!(plate_width(floor), Some(floor));
    }

    #[test]
    fn a_drag_past_the_ceiling_stops_at_it() {
        let ceiling = *theme::SCENE_W_RANGE.end();
        assert_eq!(plate_width(ceiling + 200.0), Some(ceiling));
    }
}
