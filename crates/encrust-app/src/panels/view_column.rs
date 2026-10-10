use egui::{Align2, Id, Rect, vec2};

use crate::panels::{Window, frame_view};
use crate::shortcuts::{self, Action};
use crate::ui::{card, icon, icon_button, icon_toggle};

/// Points between the card and the corner of the viewport it is anchored to, and the
/// margin inside it.
const INSET: f32 = 12.0;
const CARD_PAD: i8 = 3;

/// Top left of the viewport: how it is looked at.
pub fn ui(ui: &egui::Ui, window: &mut Window, viewport: Rect) {
    egui::Area::new(Id::new("view-tools"))
        .order(egui::Order::Middle)
        .fixed_pos(viewport.left_top() + vec2(INSET, INSET))
        .pivot(Align2::LEFT_TOP)
        .constrain_to(viewport)
        .show(ui.ctx(), |ui| {
            card()
                .inner_margin(CARD_PAD)
                .show(ui, |ui| view_tools(ui, window));
        });
}

/// What the viewport draws besides the models, and how to point the camera at them.
fn view_tools(ui: &mut egui::Ui, window: &mut Window) {
    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
    ui.vertical(|ui| {
        if icon_button(ui, icon::FRAME, &shortcuts::tooltip(Action::FrameView)).clicked() {
            frame_view(
                &window.doc.scene,
                &window.doc.plate,
                &mut window.view.camera,
            );
        }
        let grid = &mut window.view.options.grid;
        if icon_toggle(ui, icon::GRID, "Plate grid", *grid).clicked() {
            *grid = !*grid;
        }
        let xray = &mut window.view.options.xray;
        if icon_toggle(ui, icon::XRAY, "See through models", *xray).clicked() {
            *xray = !*xray;
        }
    });
}
