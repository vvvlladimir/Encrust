use egui::{Align2, Id, Rect, vec2};

use crate::i18n::tr;
use crate::panels::{Window, frame_view, set_orthographic};
use crate::shortcuts::{self, Action};
use crate::ui::{card, icon, icon_button, icon_toggle};

/// The margin inside the card.
const CARD_PAD: i8 = 3;

/// The bottom right of the viewport, beside this plate's card: how the viewport is looked
/// at. Answers where the card stands.
pub fn ui(ui: &egui::Ui, window: &mut Window, viewport: Rect) -> Rect {
    egui::Area::new(Id::new("view-tools"))
        .order(egui::Order::Middle)
        .fixed_pos(viewport.right_bottom())
        .pivot(Align2::RIGHT_BOTTOM)
        .constrain_to(viewport)
        .show(ui.ctx(), |ui| {
            card()
                .inner_margin(CARD_PAD)
                .show(ui, |ui| view_tools(ui, window));
        })
        .response
        .rect
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
        if icon_toggle(ui, icon::GRID, &tr!("menu-plate-grid"), *grid).clicked() {
            *grid = !*grid;
        }
        let xray = &mut window.view.options.xray;
        if icon_toggle(ui, icon::XRAY, &tr!("menu-xray"), *xray).clicked() {
            *xray = !*xray;
        }
        let flat = window.view.camera.orthographic;
        if icon_toggle(ui, icon::PERSPECTIVE, &tr!("menu-orthographic"), flat).clicked() {
            set_orthographic(window.view, !flat);
        }
    });
}
