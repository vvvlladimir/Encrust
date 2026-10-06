use egui::{Align2, Frame, Id, Rect};

use crate::files::{self, Wanted};
use crate::panels::{Window, mask_pane, view_column, viewport_panel};
use crate::ui::{card, icon, primary_button, theme};
use crate::workspace::Mode;

/// The stage: the viewport, the mask beside it in Preview, and the cards over it.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let stage = ui.max_rect();

    // Preview gives the mask the same room as the model: a 300 point column cannot show
    // an 8520 pixel panel. See `docs/decisions/0103`.
    if *window.mode == Mode::Preview {
        let half = (ui.available_width() * 0.5).floor();
        egui::Panel::right("mask-pane")
            .exact_size(half)
            .resizable(false)
            .frame(Frame::new().fill(theme::colors().sunken))
            .show(ui, |ui| mask_pane::ui(ui, window.machine));
    }

    let viewport = viewport_panel::ui(ui, window);
    if window.doc.scene.is_empty() {
        empty_state(ui, viewport, window);
    }
    view_column::ui(ui, window, viewport, stage);
}

/// The whole viewport when there is nothing on the plate: one card, one thing to do.
fn empty_state(ui: &egui::Ui, viewport: Rect, window: &mut Window) {
    egui::Area::new(Id::new("empty-plate"))
        .order(egui::Order::Middle)
        .fixed_pos(viewport.center())
        .pivot(Align2::CENTER_CENTER)
        .constrain_to(viewport)
        .show(ui.ctx(), |ui| {
            card()
                .inner_margin(egui::Margin::symmetric(26, 22))
                .show(ui, |ui| {
                    ui.set_width(250.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(icon::EMPTY_PLATE)
                                .font(theme::icon(32.0))
                                .color(theme::colors().text_low),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("No model on the plate")
                                .font(theme::body())
                                .color(theme::colors().text_high),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Open a model or a project, or drop one into the window.",
                            )
                            .font(theme::label())
                            .color(theme::colors().text_mid),
                        );
                        ui.add_space(14.0);
                        if primary_button(ui, icon::OPEN, "Open...", true).clicked()
                            && let Some(file) = files::pick(Wanted::ModelOrProject)
                        {
                            crate::app::open_by_what_it_is(window, file);
                        }
                    });
                });
        });
}
