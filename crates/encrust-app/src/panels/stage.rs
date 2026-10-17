use egui::{Align2, Id, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

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
        mask(ui, window, stage, clear);
        stage_notice::ui(ui, window, clear);
        return;
    }

    // Preview gives the mask room of its own beside the model: a 300 point column cannot
    // show an 8520 pixel panel. See `docs/decisions/0103`.
    let mut seen = stage;
    if *window.mode == Mode::Preview {
        let middle = divide(stage, clear, window.view.options.mask_split);
        seen.max.x = middle;
        let pane = Rect::from_x_y_ranges(middle..=stage.right(), stage.y_range());
        ui.painter().rect_filled(pane, 0.0, theme::colors().sunken);
        let beside = Rect::from_x_y_ranges(middle..=clear.right(), clear.y_range());
        mask(ui, window, pane, beside);
        divider(ui, window, stage, clear, middle);
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
    let column = view_column::ui(ui, window, clear);
    let beside = clear.with_max_x((column.left() - theme::FLOAT_GAP).max(clear.left()));
    stage_notice::ui(ui, window, beside);
}

/// The exposure mask, drawn in `rect`, its caption kept to `clear`.
fn mask(ui: &mut egui::Ui, window: &mut Window, rect: Rect, clear: Rect) {
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        mask_pane::ui(ui, window.machine, clear);
    });
}

/// Points of grab either side of the divide, and the least of the model or the mask left
/// between it and the side cards.
const GRAB_W: f32 = 4.0;
const SIDE_MIN: f32 = 120.0;

/// Where the model gives way to the mask: `split` of the stage, kept between the side
/// cards so that the divide can always be taken hold of again.
fn divide(stage: Rect, clear: Rect, split: f32) -> f32 {
    let x = stage.left() + split * stage.width();
    let (low, high) = (clear.left() + SIDE_MIN, clear.right() - SIDE_MIN);
    let x = if low < high {
        x.clamp(low, high)
    } else {
        clear.center().x
    };
    x.floor()
}

/// The rule between the model and the mask, which a drag moves.
///
/// The handle is its own area, so the camera beside it does not take the drag as an orbit,
/// and it keeps to `clear`, so it never lies over a card.
fn divider(ui: &egui::Ui, window: &mut Window, stage: Rect, clear: Rect, middle: f32) {
    let colors = theme::colors();
    ui.painter()
        .vline(middle, stage.y_range(), Stroke::new(1.0, colors.hairline));
    let pointer = egui::Area::new(Id::new("mask-divider"))
        .order(egui::Order::Middle)
        .fixed_pos(pos2(middle - GRAB_W, clear.top()))
        .show(ui.ctx(), |ui| {
            // Not focusable: a grip has nothing to do from the keyboard.
            let (strip, response) =
                ui.allocate_exact_size(vec2(2.0 * GRAB_W, clear.height()), Sense::DRAG);
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                ui.painter().vline(
                    strip.center().x,
                    strip.y_range(),
                    Stroke::new(1.0, colors.accent),
                );
            }
            response
                .dragged()
                .then(|| response.interact_pointer_pos())
                .flatten()
        })
        .inner;
    if let Some(pointer) = pointer {
        window.view.options.mask_split = split_at(stage, clear, pointer.x);
    }
}

/// The fraction of the stage a divide dragged to `x` leaves the model.
fn split_at(stage: Rect, clear: Rect, x: f32) -> f32 {
    let range = theme::MASK_SPLIT_RANGE;
    let raw = ((x - stage.left()) / stage.width()).clamp(*range.start(), *range.end());
    (divide(stage, clear, raw) - stage.left()) / stage.width()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stage() -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 600.0))
    }

    #[test]
    fn the_divide_stands_where_it_was_left() {
        let clear = Rect::from_min_max(pos2(200.0, 0.0), pos2(800.0, 600.0));
        assert_eq!(divide(stage(), clear, 0.4), 400.0);
        assert!((split_at(stage(), clear, 400.0) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn the_divide_stays_clear_of_the_side_cards() {
        let clear = Rect::from_min_max(pos2(200.0, 0.0), pos2(800.0, 600.0));
        assert_eq!(divide(stage(), clear, 0.05), 200.0 + SIDE_MIN);
        assert_eq!(divide(stage(), clear, 0.95), 800.0 - SIDE_MIN);
    }

    #[test]
    fn a_stage_too_narrow_to_divide_is_cut_down_the_middle_of_its_room() {
        let clear = Rect::from_min_max(pos2(400.0, 0.0), pos2(600.0, 600.0));
        assert_eq!(divide(stage(), clear, 0.2), 500.0);
    }
}
