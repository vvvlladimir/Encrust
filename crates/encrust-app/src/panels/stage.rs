use egui::{Align2, Frame, Id, Pos2, Rect};

use crate::files::{self, Wanted};
use crate::panels::{Window, mask_pane, view_column, viewport_panel};
use crate::ui::{card, icon, primary_button, theme};
use crate::workspace::Mode;

/// Where the cards over the stage ended up last frame.
///
/// The viewport reads the raw pointer before the cards are drawn, so without this a press
/// on a card would orbit the camera as well as press the card. One frame of lag only
/// matters on the frame a card moves, and they move when the window is resized.
#[derive(Debug, Default)]
pub struct Overlays {
    rects: Vec<Rect>,
    /// Where the open floating window is, if there is one. It is drawn before the stage
    /// rather than after it, so unlike the cards this is not a frame behind.
    floating: Option<Rect>,
}

impl Overlays {
    /// The pointer is over a card rather than over the model behind it.
    pub fn covers(&self, position: Pos2) -> bool {
        self.rects.iter().any(|rect| rect.contains(position))
            || self.floating.is_some_and(|rect| rect.contains(position))
    }

    /// Where the floating window ended up this frame, or `None` when none is open.
    pub fn set_floating(&mut self, rect: Option<Rect>) {
        self.floating = rect;
    }
}

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
    let mut rects = Vec::new();

    if window.doc.scene.is_empty() {
        rects.push(empty_state(ui, viewport, window));
    }

    rects.extend(view_column::ui(ui, window, viewport, stage));

    window.view.overlays.rects = rects;
}

/// The whole viewport when there is nothing on the plate: one card, one thing to do.
fn empty_state(ui: &egui::Ui, viewport: Rect, window: &mut Window) -> Rect {
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
        })
        .response
        .rect
}
