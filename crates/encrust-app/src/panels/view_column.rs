use egui::{Align2, Id, Pos2, Rect, vec2};

use crate::panels::{Window, frame_view, section};
use crate::ui::{card, icon, icon_button};

/// Points between a card and the edge of the stage it is anchored to, and between the
/// two cards where they share an edge.
const INSET: f32 = 12.0;
const GAP: f32 = 8.0;
/// Margin inside each card. Both hold one column of icon buttons, so with the same margin
/// they come out the same width and line up along the edge.
const CARD_PAD: i8 = 3;

/// The cards down the right edge: the view tools at the top of the viewport and the
/// section rail under them. Returns the room they took.
///
/// In Preview the viewport is the left half of the stage and the rail is parked against
/// the mask's edge instead (ADR 0103), with the stage's whole height to itself.
pub fn ui(ui: &egui::Ui, window: &mut Window, viewport: Rect, stage: Rect) -> Vec<Rect> {
    let tools = card_at(
        ui,
        "view-tools",
        viewport,
        viewport.right_top() + vec2(-INSET, INSET),
        Align2::RIGHT_TOP,
        |ui| view_tools(ui, window),
    );
    let mut rects = vec![tools];

    if section::is_available(window) {
        let shares_edge = (stage.right() - viewport.right()).abs() < 1.0;
        let top = if shares_edge {
            tools.bottom() + GAP
        } else {
            stage.top() + INSET
        };
        let bottom = (stage.bottom() - INSET).max(top);
        let slider_h = section::slider_height(bottom - top);
        rects.push(card_at(
            ui,
            "section-rail",
            stage,
            Pos2::new(stage.right() - INSET, (top + bottom) / 2.0),
            Align2::RIGHT_CENTER,
            |ui| section::ui(ui, window, slider_h),
        ));
    }
    rects
}

/// A card pinned by its `pivot` corner to `at`, kept inside `within`.
fn card_at(
    ui: &egui::Ui,
    id: &str,
    within: Rect,
    at: Pos2,
    pivot: Align2,
    add: impl FnOnce(&mut egui::Ui),
) -> Rect {
    egui::Area::new(Id::new(id))
        .order(egui::Order::Middle)
        .fixed_pos(at)
        .pivot(pivot)
        .constrain_to(within)
        .show(ui.ctx(), |ui| {
            card().inner_margin(CARD_PAD).show(ui, add);
        })
        .response
        .rect
}

/// What the viewport draws besides the models, and how to point the camera at them.
fn view_tools(ui: &mut egui::Ui, window: &mut Window) {
    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
    ui.vertical(|ui| {
        if icon_button(ui, icon::FRAME, "Frame view").clicked() {
            frame_view(
                &window.doc.scene,
                &window.doc.plate,
                &mut window.view.camera,
            );
        }
        let grid = &mut window.view.options.grid;
        if icon_button(ui, icon::GRID, "Plate grid").clicked() {
            *grid = !*grid;
        }
        let xray = &mut window.view.options.xray;
        if icon_button(ui, icon::XRAY, "See through models").clicked() {
            *xray = !*xray;
        }
    });
}
