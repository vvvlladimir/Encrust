mod find;
mod inspector;
mod mask_pane;
mod plate_strip;
mod plate_summary;
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
mod view_column;
mod view_cube;
mod viewport_panel;

pub use scene_panel::duplicate_selection;
pub use section::animate as animate_preview;
pub use slice::slice_this_plate;
pub use start::still_starting;
pub use title_bar::{layers_and_exposure, open_machines, toggle_settings};
pub use viewport_panel::{frame_view, set_orthographic};

use egui::{Align2, Id, Pos2, Rect, Sense, pos2, vec2};

use crate::state::{Doc, Machine, Tools, View};
use crate::ui::{card, theme};
use crate::workspace::{Mode, Tool};

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

        // Over everything, the Settings screen included: the keys it lists belong to both
        // halves of the window.
        if self.view.options.sheet {
            shortcuts_sheet::ui(ui.ctx(), &mut self.view.options.sheet);
        }
        if self.view.options.find {
            find::ui(ui.ctx(), self);
        }
        report::ui(ui.ctx(), &self.doc.scene, self.tools, self.machine);
        settings::machines::window(ui.ctx(), self);

        // The Settings screen takes the whole window under the bar: a profile is edited
        // instead of the plate, not beside it.
        if self.machine.settings.open {
            egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ui, |ui| settings::ui(ui, self));
            return;
        }
        if still_starting(self) {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(theme::colors().base))
                .show(ui, |ui| start::ui(ui, self));
            return;
        }

        let stage = ui.available_rect_before_wrap();
        let room = self.columns(ui.ctx(), stage);
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| stage::ui(ui, self, room));
    }

    /// The cards over the stage: the rail and the inspector down its left, the layer strip
    /// and the plate down its right. Answers the room they leave between them, which the
    /// stage's own cards are placed in.
    fn columns(&mut self, ctx: &egui::Context, stage: Rect) -> Rect {
        let gap = theme::FLOAT_GAP;
        let options = self.view.options;
        let top = stage.top() + gap;
        let max_h = stage.height() - 2.0 * gap - 2.0 * CARD_STROKE;

        let corner = pos2(stage.left() + gap, top);
        let rail = card_at(ctx, "rail", corner, Align2::LEFT_TOP, max_h, |ui| {
            tool_rail::ui(ui, self, max_h);
        });
        // The whole stage given to the mask has nothing for a tool to work on.
        let left = if (*self.mode == Mode::Preview && options.mask_only) || !options.inspector {
            rail
        } else {
            self.inspector(ctx, pos2(rail.right() + gap, top), max_h)
        };

        let corner = pos2(stage.right() - gap, top);
        let strip = card_at(ctx, "layer-strip", corner, Align2::RIGHT_TOP, max_h, |ui| {
            egui::Frame::new().inner_margin(STRIP_PAD).show(ui, |ui| {
                let pad = f32::from(2 * STRIP_PAD);
                ui.set_width(theme::LAYER_STRIP_W - pad - 2.0 * CARD_STROKE);
                ui.set_height(max_h - pad);
                section::ui(ui, self);
            });
        });
        let (models, plate) = self.plate_cards(ctx, pos2(strip.left() - gap, top), stage);
        for (id, card) in [("models-edge", models), ("plate-edge", plate)] {
            if let Some(raw) = drag_edge(ctx, id, card, false) {
                self.view.options.plate_w =
                    raw.clamp(*theme::SCENE_W_RANGE.start(), *theme::SCENE_W_RANGE.end());
            }
        }

        let left = left.right() + gap;
        let right = (models.left() - gap).max(left);
        Rect::from_x_y_ranges(left..=right, top..=stage.bottom() - gap)
    }
}

impl Window<'_> {
    /// The open tool's card from `corner` down, and the edge that sizes it.
    fn inspector(&mut self, ctx: &egui::Context, corner: Pos2, max_h: f32) -> Rect {
        let width = self.view.options.inspector_w;
        let inspector = card_at(ctx, "inspector", corner, Align2::LEFT_TOP, max_h, |ui| {
            ui.set_width(width);
            inspector::ui(ui, self, max_h);
        });
        if let Some(raw) = drag_edge(ctx, "inspector-edge", inspector, true) {
            self.view.options.inspector_w = raw.clamp(
                *theme::INSPECTOR_W_RANGE.start(),
                *theme::INSPECTOR_W_RANGE.end(),
            );
        }
        inspector
    }

    /// The models from `corner` down, and this plate's figures and Slice standing on the
    /// stage's foot under them, both as wide as the plate panel. Answers the two cards.
    fn plate_cards(&mut self, ctx: &egui::Context, corner: Pos2, stage: Rect) -> (Rect, Rect) {
        let gap = theme::FLOAT_GAP;
        let width = self.view.options.plate_w;
        let foot = pos2(corner.x, stage.bottom() - gap);
        let plate = card_at(
            ctx,
            "this-plate",
            foot,
            Align2::RIGHT_BOTTOM,
            f32::INFINITY,
            |ui| {
                ui.set_width(width);
                scene_panel::this_plate(ui, self);
            },
        );
        let max_h = plate.top() - gap - corner.y - 2.0 * CARD_STROKE;
        let models = card_at(ctx, "models", corner, Align2::RIGHT_TOP, max_h, |ui| {
            ui.set_width(width);
            scene_panel::ui(ui, self, max_h);
        });
        (models, plate)
    }
}

/// Points between the layer strip and the edge of its card.
const STRIP_PAD: i8 = 4;

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
    egui::Area::new(Id::new(id))
        .order(egui::Order::Middle)
        .fixed_pos(corner)
        .pivot(pivot)
        .show(ctx, |ui| {
            card().inner_margin(0).show(ui, |ui| {
                ui.set_max_height(max_h);
                ui.spacing_mut().item_spacing.y = 0.0;
                add(ui);
            });
        })
        .response
        .rect
}

/// Points of grab beside a card's inner edge.
const GRAB_W: f32 = 6.0;

/// A drag on the inner edge of a card. Returns the width the pointer is asking for,
/// unclamped; the caller holds it to the card's range.
///
/// The handle is its own area just outside the card, so the camera under it does not
/// take the drag as an orbit.
fn drag_edge(ctx: &egui::Context, id: &str, card: Rect, fixed_left: bool) -> Option<f32> {
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
            let (strip, response) = ui.allocate_exact_size(size, Sense::DRAG);
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
            let pointer = response.interact_pointer_pos()?;
            Some(if fixed_left {
                pointer.x - card.left()
            } else {
                card.right() - pointer.x
            })
        })
        .inner
}

/// The top bar sits on the window's own colour, with a hairline between it and the work.
fn strip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::colors().base)
        .inner_margin(theme::STRIP_MARGIN)
}
