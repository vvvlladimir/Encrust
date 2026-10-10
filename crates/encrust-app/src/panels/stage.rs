use egui::{Align2, Id, Rect, UiBuilder};

use crate::files::{self, Wanted};
use crate::panels::{Window, mask_pane, stage_notice, view_column, view_cube, viewport_panel};
use crate::ui::{card, icon, primary_button, theme};
use crate::workspace::Mode;

/// The stage: the viewport under the whole of it, the mask beside the model or in its
/// place, and the cards over them, which keep to `clear`, the room the side cards leave.
pub fn ui(ui: &mut egui::Ui, window: &mut Window, clear: Rect) {
    let stage = ui.available_rect_before_wrap();

    if *window.mode == Mode::Preview && window.view.options.mask_only {
        ui.painter().rect_filled(stage, 0.0, theme::colors().sunken);
        mask(ui, window, clear);
        stage_notice::ui(ui, window, clear);
        return;
    }

    // Preview gives the mask the same room as the model: a 300 point column cannot show
    // an 8520 pixel panel. See `docs/decisions/0103`.
    let mut seen = stage;
    if *window.mode == Mode::Preview {
        let middle = clear.center().x.floor();
        seen.max.x = middle;
        let pane = Rect::from_x_y_ranges(middle..=stage.right(), stage.y_range());
        ui.painter().rect_filled(pane, 0.0, theme::colors().sunken);
        mask(
            ui,
            window,
            Rect::from_x_y_ranges(middle..=clear.right(), clear.y_range()),
        );
    }

    let viewport = ui
        .scope_builder(UiBuilder::new().max_rect(seen), |ui| {
            viewport_panel::ui(ui, window)
        })
        .inner;
    let clear = clear.intersect(viewport);
    if window.doc.scene.is_empty() {
        empty_state(ui, clear.center(), viewport, window);
    }
    view_cube::ui(ui, window, clear);
    view_column::ui(ui, window, clear);
    stage_notice::ui(ui, window, clear);
}

/// The exposure mask, drawn in `rect`.
fn mask(ui: &mut egui::Ui, window: &mut Window, rect: Rect) {
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        mask_pane::ui(ui, window.machine);
    });
}

/// The whole viewport when there is nothing on the plate: one card, one thing to do,
/// centred on `at` and kept inside the viewport.
fn empty_state(ui: &egui::Ui, at: egui::Pos2, viewport: Rect, window: &mut Window) {
    egui::Area::new(Id::new("empty-plate"))
        .order(egui::Order::Middle)
        .fixed_pos(at)
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
