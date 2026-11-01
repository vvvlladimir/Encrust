mod destination;
mod edge_fields;
mod find;
mod hint_bar;
mod inspector;
mod mask_pane;
mod plate_strip;
mod plate_summary;
mod printer_card;
mod report;
mod scene_panel;
pub(crate) mod section;
mod settings;
mod shortcuts_sheet;
mod slice;
mod stage;
mod stage_notice;
mod start;
mod support_diagram;
mod support_fields;
mod title_bar;
mod tool_rail;
mod unreadable_profiles;
mod view_column;
mod view_cube;
mod viewport_panel;

pub use scene_panel::duplicate_selection;
pub use section::animate as animate_preview;
pub use settings::{import_dropped, import_profile};
pub use slice::{can_slice, slice_every_plate, slice_this_plate};
pub use start::{open_recent, still_starting};
pub use title_bar::{layers_and_exposure, open_machines, toggle_settings};
pub use viewport_panel::{frame_view, set_orthographic};

use egui::emath::TSTransform;
use egui::{Align2, Id, Pos2, Rect, Sense, pos2, vec2};

use crate::state::{Doc, Machine, Tools, View};
use crate::ui::{card, theme};
use crate::workspace::{Mode, Tool, ViewOptions};

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

    /// Lays the window out: the top bar, then the stage under it with the cards floating
    /// over it, each as tall as what it holds. See `docs/decisions/0221`.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("title")
            .exact_size(theme::TOP_BAR_H)
            .frame(strip_frame())
            .show(ui, |ui| title_bar::ui(ui, self));

        // Over everything: the keys it lists belong to both halves of the window.
        if self.view.options.sheet {
            shortcuts_sheet::ui(ui.ctx(), &mut self.view.options.sheet);
        }
        if self.view.options.find {
            find::ui(ui.ctx(), self);
        }
        report::ui(ui.ctx(), &self.doc.scene, self.tools, self.machine);
        settings::machines::window(ui.ctx(), self);
        self.machine.network.show_pictures(ui.ctx());
        printer_card::sheet::ui(ui.ctx(), self);
        settings::window(ui.ctx(), self);

        if still_starting(self) {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(theme::colors().base))
                .show(ui, |ui| start::ui(ui, self));
            return;
        }

        let stage = ui.available_rect_before_wrap();
        let (clear, row) = self.columns(ui.ctx(), stage);
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| stage::ui(ui, self, clear, row));
    }

    /// The cards over the stage: the inspector at its left, the layer strip and the plate
    /// down its right, the tool row across the top between them (ADR 0382). Answers the
    /// room they leave under the row, which the stage's own cards are placed in, and the row.
    fn columns(&mut self, ctx: &egui::Context, stage: Rect) -> (Rect, Option<Rect>) {
        let gap = theme::FLOAT_GAP;
        let options = self.view.options;
        let top = stage.top() + gap;
        let max_h = (stage.height() - 2.0 * gap - 2.0 * CARD_STROKE).max(0.0);

        let corner = pos2(stage.right() - gap, top);
        let strip = card_at(ctx, "layer-strip", corner, Align2::RIGHT_TOP, max_h, |ui| {
            egui::Frame::new()
                .inner_margin(theme::TOOLBAR_PAD)
                .show(ui, |ui| {
                    let pad = 2.0 * theme::TOOLBAR_PAD;
                    ui.set_width(theme::LAYER_STRIP_W - pad - 2.0 * CARD_STROKE);
                    ui.set_height((max_h - pad).max(0.0));
                    section::ui(ui, self);
                });
        });

        let between = strip.left() - stage.left();
        let wanted = inspector_w(&options, *self.mode);
        let (left_w, plate_w) = fit_side_cards(between, wanted.unwrap_or(0.0), options.plate_w);
        let left = match wanted {
            Some(_) => {
                let corner = pos2(stage.left() + gap, top);
                self.inspector_card(ctx, corner, max_h, left_w).right()
            }
            None => stage.left(),
        };
        let corner = pos2(strip.left() - gap, top);
        let (models, plate) = self.plate_cards(ctx, corner, stage, plate_w);
        for (id, card) in [("models-edge", models), ("plate-edge", plate)] {
            match drag_edge(ctx, id, card, false) {
                Some(Edge::To(raw)) => {
                    self.view.options.plate_w =
                        raw.clamp(*theme::SCENE_W_RANGE.start(), *theme::SCENE_W_RANGE.end());
                }
                Some(Edge::Reset) => self.view.options.plate_w = theme::SCENE_W,
                None => {}
            }
        }

        let left = left + gap;
        let right = (models.left() - gap).max(left);
        // The row stands where it would beside the inspector, put away or not.
        let row_left = match (wanted, inspector_room_w(&options, *self.mode)) {
            (None, Some(width)) => {
                let (width, _) = fit_side_cards(between, width, options.plate_w);
                (stage.left() + 2.0 * gap + width + 2.0 * CARD_STROKE).clamp(left, right)
            }
            _ => left,
        };
        let row = match row_shown(&options, *self.mode) {
            true => {
                let room = (right - row_left - 2.0 * CARD_STROKE).max(0.0);
                let row = card_at(
                    ctx,
                    "tool-row",
                    pos2((row_left + right) / 2.0, top),
                    Align2::CENTER_TOP,
                    max_h,
                    |ui| {
                        tool_rail::ui(ui, self, room);
                    },
                );
                Some(row)
            }
            false => None,
        };
        let under_row = row.map_or(top, |row| row.bottom() + gap);
        let bottom = (stage.bottom() - gap).max(under_row);
        (Rect::from_x_y_ranges(left..=right, under_row..=bottom), row)
    }
}

impl Window<'_> {
    /// The open tool's card from `corner`, `width` wide, and the edge that sizes it, a
    /// double click giving it back its default width.
    fn inspector_card(
        &mut self,
        ctx: &egui::Context,
        corner: Pos2,
        max_h: f32,
        width: f32,
    ) -> Rect {
        let inspector = card_at(ctx, "inspector", corner, Align2::LEFT_TOP, max_h, |ui| {
            exactly_wide(ui, width, |ui| inspector::ui(ui, self, max_h));
        });
        let range = theme::INSPECTOR_W_RANGE;
        match drag_edge(ctx, "inspector-edge", inspector, true) {
            Some(Edge::To(raw)) => {
                self.view.options.inspector_w = Some(raw.clamp(*range.start(), *range.end()));
            }
            Some(Edge::Reset) => self.view.options.inspector_w = None,
            None => {}
        }
        inspector
    }

    /// The models from `corner` down, and this plate's figures and Slice standing on the
    /// stage's foot under them, both `width` wide; in Preview the plate's alone.
    /// Answers the two cards.
    fn plate_cards(
        &mut self,
        ctx: &egui::Context,
        corner: Pos2,
        stage: Rect,
        width: f32,
    ) -> (Rect, Rect) {
        let gap = theme::FLOAT_GAP;
        let at = pos2(corner.x, stage.bottom() - gap);
        let plate = card_at(
            ctx,
            "this-plate",
            at,
            Align2::RIGHT_BOTTOM,
            f32::INFINITY,
            |ui| {
                ui.set_width(width);
                plate_summary::ui(ui, self);
            },
        );
        if *self.mode == Mode::Preview {
            return (plate, plate);
        }
        let max_h = (plate.top() - gap - corner.y - 2.0 * CARD_STROKE).max(0.0);
        let models = card_at(ctx, "models", corner, Align2::RIGHT_TOP, max_h, |ui| {
            ui.set_width(width);
            scene_panel::ui(ui, self, max_h);
        });
        (models, plate)
    }
}

/// The inspector's width before the window has its say: the width it was dragged to, else
/// the default; `None` while it is put away or Preview gives the whole stage to the mask.
fn inspector_w(options: &ViewOptions, mode: Mode) -> Option<f32> {
    inspector_room_w(options, mode).filter(|_| options.inspector)
}

/// The inspector's width shown or put away; `None` where Preview gives the whole stage to
/// the mask.
fn inspector_room_w(options: &ViewOptions, mode: Mode) -> Option<f32> {
    if !row_shown(options, mode) {
        return None;
    }
    let range = theme::INSPECTOR_W_RANGE;
    let wanted = options.inspector_w.unwrap_or(theme::INSPECTOR_W);
    Some(wanted.clamp(*range.start(), *range.end()))
}

/// Whether the tool row stands over the stage: everywhere but the mask alone.
fn row_shown(options: &ViewOptions, mode: Mode) -> bool {
    !(mode == Mode::Preview && options.mask_only)
}

/// The least of the stage the side cards leave clear, in points: the tool row's glyphs, and
/// the view cube and the view buttons under it.
const STAGE_MIN_W: f32 = 300.0;

/// The left column's width and the plate cards' width that fit `between` the stage's left
/// edge and the layer strip with `STAGE_MIN_W` of stage left between them. Each gives way
/// in proportion to what it has above the least its range allows, and no further.
fn fit_side_cards(between: f32, left_w: f32, plate_w: f32) -> (f32, f32) {
    let gaps = 4.0 * theme::FLOAT_GAP + 4.0 * CARD_STROKE;
    let room = between - gaps - STAGE_MIN_W;
    let over = (left_w + plate_w - room).max(0.0);
    let slack = [
        left_w - left_w.min(*theme::INSPECTOR_W_RANGE.start()),
        plate_w - plate_w.min(*theme::SCENE_W_RANGE.start()),
    ];
    let total = slack[0] + slack[1];
    let give = |slack: f32| match total > 0.0 {
        true => (over * slack / total).min(slack),
        false => 0.0,
    };
    (left_w - give(slack[0]), plate_w - give(slack[1]))
}

/// The width of a card's outline, which the room inside it is short of.
const CARD_STROKE: f32 = 1.0;

/// One card floating over the stage, its `pivot` corner at `corner`, no taller than
/// `max_h` points. Answers the rectangle it took.
///
/// An area offers its contents last frame's size, so a card would never grow; the room
/// is set to the most the card may take instead.
fn card_at(
    ctx: &egui::Context,
    id: &str,
    corner: Pos2,
    pivot: Align2,
    max_h: f32,
    add: impl FnOnce(&mut egui::Ui),
) -> Rect {
    let mut drawn_at = corner;
    let shown = egui::Area::new(Id::new(id))
        .order(egui::Order::Middle)
        .fixed_pos(corner)
        .pivot(pivot)
        .show(ctx, |ui| {
            drawn_at = ui.max_rect().min;
            // Clipped to the window before the shift below, what it grew past the edge would
            // be cut off until it stopped growing.
            ui.set_clip_rect(Rect::EVERYTHING);
            card().inner_margin(0).show(ui, |ui| {
                ui.set_max_height(max_h);
                ui.spacing_mut().item_spacing.y = 0.0;
                add(ui);
            });
        })
        .response;
    // egui places an area from last frame's size, so a card held by any corner but its top
    // left is drawn off it by what it grew; its layer is moved there for the frame instead.
    let meant = shown.rect;
    let shift = meant.min - drawn_at;
    let transform = match shift.length() > SHIFT_SLACK {
        true => TSTransform::from_translation(shift),
        false => TSTransform::IDENTITY,
    };
    ctx.set_transform_layer(shown.layer_id, transform);
    meant
}

/// Points a card may stand off where it is meant to and be left there.
const SHIFT_SLACK: f32 = 0.01;

/// Lays `add` out in `width` points and takes exactly that much: what a long word pushes
/// past it is cut at the edge rather than widening the card, in every language.
fn exactly_wide(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    let room = Rect::from_min_size(ui.cursor().min, vec2(width, ui.available_height()));
    let mut inside = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(*ui.layout()));
    inside.set_clip_rect(room.intersect(ui.clip_rect()));
    inside.set_width(width);
    add(&mut inside);
    ui.allocate_exact_size(vec2(width, inside.min_rect().height()), Sense::hover());
}

/// Points of grab beside a card's inner edge.
const GRAB_W: f32 = 6.0;

/// What the inner edge of a card was asked for.
enum Edge {
    /// The width the pointer is dragging to, unclamped; the caller holds it to the range.
    To(f32),
    /// A double click: the card's own width back.
    Reset,
}

/// A drag or a double click on the inner edge of a card.
///
/// The handle is its own area just outside the card, so the camera under it does not
/// take the drag as an orbit.
fn drag_edge(ctx: &egui::Context, id: &str, card: Rect, fixed_left: bool) -> Option<Edge> {
    let edge = if fixed_left {
        card.right()
    } else {
        card.left()
    };
    let left = if fixed_left { edge } else { edge - GRAB_W };
    egui::Area::new(Id::new(id))
        .order(egui::Order::Middle)
        .fixed_pos(pos2(left, card.top()))
        .show(ctx, |ui| {
            let size = vec2(GRAB_W, card.height());
            // Not focusable: a grip has nothing to do from the keyboard.
            let (strip, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            if response.double_clicked() {
                return Some(Edge::Reset);
            }
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                let x = if fixed_left {
                    strip.left()
                } else {
                    strip.right()
                };
                ui.painter().vline(
                    x,
                    strip.y_range().shrink(theme::R_SURFACE.nw.into()),
                    egui::Stroke::new(1.0, theme::colors().accent),
                );
            }
            let pointer = response
                .interact_pointer_pos()
                .filter(|_| response.dragged())?;
            Some(Edge::To(if fixed_left {
                pointer.x - card.left()
            } else {
                card.right() - pointer.x
            }))
        })
        .inner
}

/// The top bar sits on the window's own colour, with a hairline between it and the work.
fn strip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::colors().base)
        .inner_margin(theme::STRIP_MARGIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the stage's left edge and the layer strip leave between them for cards of these
    /// widths and the least stage, with the gaps and outlines `fit_side_cards` counts.
    fn between(inspector_w: f32, plate_w: f32) -> f32 {
        inspector_w + plate_w + STAGE_MIN_W + 4.0 * theme::FLOAT_GAP + 4.0 * CARD_STROKE
    }

    #[test]
    fn side_cards_with_room_keep_their_widths() {
        let fitted = fit_side_cards(between(328.0, 272.0) + 50.0, 328.0, 272.0);
        assert_eq!(fitted, (328.0, 272.0));
    }

    #[test]
    fn side_cards_give_way_in_proportion_to_what_they_have_above_their_least() {
        // 68 above the inspector's least of 260 and 102 above the plate's 170: 2 to 3.
        let fitted = fit_side_cards(between(328.0, 272.0) - 50.0, 328.0, 272.0);
        assert_eq!(fitted, (308.0, 242.0));
    }

    #[test]
    fn no_card_shrinks_below_the_least_its_range_allows() {
        let fitted = fit_side_cards(0.0, 328.0, 272.0);
        assert_eq!(fitted, (260.0, 170.0));
    }

    #[test]
    fn a_card_already_at_its_least_leaves_the_giving_to_the_other() {
        let fitted = fit_side_cards(between(260.0, 272.0) - 40.0, 260.0, 272.0);
        assert_eq!(fitted, (260.0, 232.0));
    }

    #[test]
    fn an_undragged_inspector_stands_at_its_default_width() {
        let options = ViewOptions::default();
        assert_eq!(
            inspector_w(&options, Mode::Prepare),
            Some(theme::INSPECTOR_W)
        );
    }

    #[test]
    fn a_dragged_inspector_keeps_the_width_it_was_dragged_to() {
        let options = ViewOptions {
            inspector_w: Some(300.0),
            ..ViewOptions::default()
        };
        assert_eq!(inspector_w(&options, Mode::Prepare), Some(300.0));
    }

    #[test]
    fn a_closed_inspector_leaves_its_room_and_the_row_stays() {
        let options = ViewOptions {
            inspector: false,
            inspector_w: Some(440.0),
            ..ViewOptions::default()
        };
        assert_eq!(inspector_w(&options, Mode::Prepare), None);
        assert!(row_shown(&options, Mode::Prepare));
        let (min, max) = theme::INSPECTOR_W_RANGE.into_inner();
        assert_eq!(
            inspector_room_w(&options, Mode::Prepare),
            Some(440.0_f32.clamp(min, max)),
            "the row keeps standing beside the room it leaves"
        );
    }

    #[test]
    fn the_mask_alone_takes_the_inspector_and_the_row() {
        let options = ViewOptions {
            mask_only: true,
            ..ViewOptions::default()
        };
        assert_eq!(inspector_w(&options, Mode::Preview), None);
        assert!(!row_shown(&options, Mode::Preview));
        assert!(inspector_w(&options, Mode::Prepare).is_some());
    }

    #[test]
    fn the_least_window_holds_both_side_cards_and_the_stage() {
        let room = theme::WINDOW_MIN_W - theme::FLOAT_GAP - theme::LAYER_STRIP_W;
        let least = between(
            *theme::INSPECTOR_W_RANGE.start(),
            *theme::SCENE_W_RANGE.start(),
        );
        assert!(
            room >= least,
            "{room} points between the stage's edge and the strip, {least} needed"
        );
    }
}
