use egui::{Align2, Frame, Id, Pos2, Rect, Vec2, vec2};

use crate::panels::{Window, frame_view, mask_pane, section, viewport_panel};
use crate::ui::{card, icon, icon_button, secondary_button, theme};
use crate::workspace::Mode;

/// Points between a floating card and the edge of the stage it is anchored to.
const INSET: f32 = 12.0;

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

    rects.push(overlay(
        ui,
        "view-tools",
        viewport,
        Align2::RIGHT_TOP,
        card().inner_margin(3),
        |ui| view_tools(ui, window),
    ));

    // The section rail is centred down the right edge of the stage rather than laid
    // across the bottom, so that one control moves the cut in both modes, and so that
    // Preview parks it against the mask; see `docs/decisions/0061`.
    if section::is_available(window) {
        let slider_h = section::slider_height(stage.height());
        rects.push(overlay(
            ui,
            "section-rail",
            stage,
            Align2::RIGHT_CENTER,
            card(),
            |ui| section::ui(ui, window, slider_h),
        ));
    }

    window.view.overlays.rects = rects;
}

/// A card floating against one edge of `against`. Returns the room it took.
fn overlay(
    ui: &egui::Ui,
    id: &str,
    against: Rect,
    corner: Align2,
    frame: Frame,
    add: impl FnOnce(&mut egui::Ui),
) -> Rect {
    // `to_sign` is -1 at a Min edge and +1 at a Max one, so subtracting it walks the
    // anchor inwards whichever edge the card is pinned to.
    let sign = Vec2::new(corner.x().to_sign(), corner.y().to_sign());
    let anchor = corner.pos_in_rect(&against) - sign * INSET;

    egui::Area::new(Id::new(id))
        .order(egui::Order::Middle)
        .fixed_pos(anchor)
        .pivot(corner)
        .constrain_to(against)
        .show(ui.ctx(), |ui| {
            frame.show(ui, add);
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
    });
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
                            egui::RichText::new("Open an STL, or drop one into the window.")
                                .font(theme::label())
                                .color(theme::colors().text_mid),
                        );
                        ui.add_space(12.0);
                        if secondary_button(ui, icon::OPEN, "Open model").clicked() {
                            window
                                .doc
                                .imports
                                .open_dialog(&window.doc.plate, &mut window.machine.status);
                        }
                    });
                });
        })
        .response
        .rect
}
