use egui::{Id, Pos2, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

use crate::panels::{
    Window, hint_bar, mask_pane, stage_notice, view_column, view_cube, viewport_panel,
};
use crate::ui::theme;
use crate::workspace::Mode;

/// The stage: the viewport, the mask beside or instead of the model, and the cards over
/// them within `clear`, the room the side cards leave under the tool `row`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window, clear: Rect, row: Option<Rect>) {
    let stage = ui.available_rect_before_wrap();

    if *window.mode == Mode::Preview && window.view.options.mask_only {
        ui.painter().rect_filled(stage, 0.0, theme::colors().sunken);
        mask(ui, window, stage, to_strip(stage, clear, clear.left()));
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
        mask(ui, window, pane, to_strip(stage, clear, middle));
        divider(ui, window, stage, clear, middle);
    }

    let viewport = ui
        .scope_builder(UiBuilder::new().max_rect(seen), |ui| {
            viewport_panel::ui(ui, window)
        })
        .inner;
    let clear = clear.intersect(viewport);
    let cube = view_cube::ui(ui, window, cube_corner(clear, row), clear);
    // Centred under the tool row, so it keeps clear of the cube on both sides.
    let inset = (clear.right() - cube.left() + theme::FLOAT_GAP).min(clear.width() / 2.0);
    let beside_mask = *window.mode == Mode::Preview;
    hint_bar::ui(
        ui,
        window,
        clear.shrink2(egui::vec2(inset, 0.0)),
        row,
        beside_mask,
    );
    let column = view_column::ui(ui, window, clear);
    let beside = clear.with_max_x((column.left() - theme::FLOAT_GAP).max(clear.left()));
    stage_notice::ui(ui, window, beside);
}

/// Where the view cube's top right stands: level with the tool row while it fits beside
/// it, under the row when it does not.
fn cube_corner(clear: Rect, row: Option<Rect>) -> Pos2 {
    match row {
        Some(row) if clear.right() - row.right() - theme::FLOAT_GAP >= view_cube::SIZE => {
            pos2(clear.right(), row.top())
        }
        _ => clear.right_top(),
    }
}

/// The mask's room over the stage from `left`: the plate's card stands on the foot, so over
/// it the room runs to the layer strip.
fn to_strip(stage: Rect, clear: Rect, left: f32) -> Rect {
    let strip = stage.right() - 2.0 * theme::FLOAT_GAP - theme::LAYER_STRIP_W;
    Rect::from_x_y_ranges(left..=strip, clear.y_range())
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

/// The rule between the model and the mask, which a drag moves. Its own area, so the camera
/// does not orbit on it, and kept to `clear` so it never lies over a card.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stage() -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 600.0))
    }

    #[test]
    fn the_cube_stands_level_with_the_row_while_it_fits_beside_it() {
        let clear = Rect::from_min_max(pos2(200.0, 70.0), pos2(800.0, 600.0));
        let row = |right: f32| Some(Rect::from_min_max(pos2(400.0, 4.0), pos2(right, 62.0)));
        let fits = 800.0 - theme::FLOAT_GAP - view_cube::SIZE;
        assert_eq!(cube_corner(clear, row(fits)), pos2(800.0, 4.0));
        assert_eq!(cube_corner(clear, row(fits + 1.0)), pos2(800.0, 70.0));
        assert_eq!(cube_corner(clear, None), pos2(800.0, 70.0));
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
