//! A chain of steps run one after another: each a folding card on a rail, its number on the
//! rail, a grip to drag it by, its switch; between them the seams a step is added or dropped at.

use egui::{
    Align2, Color32, CursorIcon, Frame, Id, Rect, Response, Sense, Stroke, Ui, UiBuilder,
    WidgetInfo, WidgetType, pos2, vec2,
};

use super::figures::{Grows, unfold};
use super::layout::help_mark;
use super::toggle::bare_switch;
use super::{focus_ring, hover_mix, icon_button, small_icon_button};
use crate::ui::theme::motion;
use crate::ui::{icon, theme};

/// The column at the left the rail runs down, and the number's disc on it, points.
const CHAIN_RAIL_W: f32 = 36.0;
const DISC_D: f32 = 22.0;
/// A card's heading, and the seam between two cards, points.
const HEAD_H: f32 = 44.0;
const SEAM_H: f32 = 30.0;
const GRIP_W: f32 = 24.0;
/// A card being dragged stands at this share of its ink, its place kept.
const DRAGGED_OPACITY: f32 = 0.35;

/// One step of a chain as its card shows it.
pub struct ChainStep<'a> {
    pub number: usize,
    pub glyph: &'a str,
    pub name: &'a str,
    /// What the folded card says of the step, each part its own run, quieter than the name.
    pub summary: &'a [&'a str],
    /// What the question mark reads out.
    pub help: &'a [String],
    pub grip_tooltip: &'a str,
    /// The step's own actions at the right of its open heading, left to right: glyph,
    /// tooltip, and whether it can be pressed.
    pub actions: &'a [(&'a str, &'a str, bool)],
    /// Whether the step does not read, so its disc and summary stand in `danger`.
    pub faulty: bool,
    pub open: bool,
    pub dragged: bool,
    /// Whether the step may be changed: a step of a built-in recipe folds, and nothing else.
    pub editable: bool,
}

/// What a card was asked: the heading pressed to fold or unfold it, an action, the grip,
/// and where the card stands.
pub struct ChainAnswer {
    pub folded: bool,
    /// Which of the step's actions was pressed.
    pub action: Option<usize>,
    pub grip: Response,
    pub rect: Rect,
}

/// A card of the chain at `id`: rail and disc, grip, glyph, name and summary, the question
/// mark, its switch over `on` and the caret; `add` its body, unfolded while `step.open`.
pub fn chain_step(
    ui: &mut Ui,
    id: Id,
    step: &ChainStep<'_>,
    on: &mut bool,
    add: impl FnOnce(&mut Ui),
) -> ChainAnswer {
    let rail_x = ui.cursor().min.x + CHAIN_RAIL_W / 2.0;
    let drawn = ui.horizontal_top(|ui| {
        ui.add_space(CHAIN_RAIL_W);
        ui.vertical(|ui| {
            if step.dragged {
                ui.multiply_opacity(DRAGGED_OPACITY);
            }
            card(ui, id, step, on, add)
        })
        .inner
    });
    let (head, mut answer) = drawn.inner;
    let rect = drawn.response.rect;
    let line = Stroke::new(1.0, theme::colors().line);
    ui.painter().line_segment(
        [pos2(rail_x, rect.top()), pos2(rail_x, rect.bottom())],
        line,
    );
    disc(ui, pos2(rail_x, head.center().y), step, *on);
    answer.rect = rect;
    answer
}

fn card(
    ui: &mut Ui,
    id: Id,
    step: &ChainStep<'_>,
    on: &mut bool,
    add: impl FnOnce(&mut Ui),
) -> (Rect, ChainAnswer) {
    let colors = theme::colors();
    let edge = if step.open {
        colors.line
    } else {
        colors.hairline
    };
    Frame::new()
        .fill(colors.panel)
        .stroke(Stroke::new(1.0, edge))
        .corner_radius(theme::R_SURFACE)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let (head, (folded, action), grip) = heading(ui, id, step, on);
            let how_open = motion::fold(ui.ctx(), id.with("fold"), step.open);
            if how_open > 0.0 {
                unfold(ui, id, how_open, Grows::Down, |ui| {
                    Frame::new()
                        .inner_margin(theme::PANEL_MARGIN)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = theme::ROW_GAP;
                            ui.add_enabled_ui(step.editable, add);
                        });
                });
            }
            let answer = ChainAnswer {
                folded,
                action,
                grip,
                rect: Rect::NOTHING,
            };
            (head, answer)
        })
        .inner
}

/// The heading: a press anywhere on it folds the card; the grip drags it, and the question
/// mark and switch at its right answer for themselves.
fn heading(
    ui: &mut Ui,
    id: Id,
    step: &ChainStep<'_>,
    on: &mut bool,
) -> (Rect, (bool, Option<usize>), Response) {
    let colors = theme::colors();
    let (rect, fold) = ui.allocate_exact_size(vec2(ui.available_width(), HEAD_H), Sense::click());
    fold.widget_info(|| {
        WidgetInfo::selected(WidgetType::CollapsingHeader, true, step.open, step.name)
    });
    let fold = fold.on_hover_cursor(CursorIcon::PointingHand);
    focus_ring(ui, &fold, theme::R_SURFACE);

    let grip_rect = Rect::from_min_size(rect.min, vec2(GRIP_W, HEAD_H));
    let sense = if step.editable {
        Sense::drag()
    } else {
        Sense::hover()
    };
    let grip = ui
        .interact(grip_rect, id.with("grip"), sense)
        .on_hover_cursor(CursorIcon::Grab)
        .on_hover_text(step.grip_tooltip);
    if grip.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }
    let middle = rect.center().y;
    if step.editable {
        let grip_ink = hover_mix(ui, &grip, colors.text_low, colors.text_mid);
        ui.painter().text(
            pos2(grip_rect.center().x + 2.0, middle),
            Align2::CENTER_CENTER,
            icon::GRIP,
            theme::icon(theme::glyph::ROW),
            grip_ink,
        );
    }

    let inner = rect.with_min_x(grip_rect.right());
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner.with_max_x(inner.right() - theme::PANEL_PAD))
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let action = controls(&mut child, step, on);
    let right = child.min_rect().left() - theme::LINE_GAP;
    title_line(ui, inner.min.x, middle, right, step, *on);
    (rect, (fold.clicked(), action), grip)
}

/// The caret, the switch, the question mark and, while open, the step's actions, laid right
/// to left; which action was pressed.
fn controls(ui: &mut Ui, step: &ChainStep<'_>, on: &mut bool) -> Option<usize> {
    ui.spacing_mut().item_spacing.x = theme::LINE_GAP;
    let caret = if step.open {
        icon::CARET_DOWN
    } else {
        icon::CARET_RIGHT
    };
    let (caret_rect, _) = ui.allocate_exact_size(vec2(12.0, HEAD_H), Sense::hover());
    ui.painter().text(
        caret_rect.center(),
        Align2::CENTER_CENTER,
        caret,
        theme::icon(theme::glyph::CARET),
        theme::colors().text_low,
    );
    ui.add_enabled_ui(step.editable, |ui| bare_switch(ui, on, step.name));
    if !step.help.is_empty() {
        help_mark(ui, step.help);
    }
    if !(step.open && step.editable) {
        return None;
    }
    ui.spacing_mut().item_spacing.x = theme::ICON_GAP;
    let mut pressed = None;
    for (at, (glyph, tooltip, enabled)) in step.actions.iter().enumerate().rev() {
        let button = ui.add_enabled_ui(*enabled, |ui| icon_button(ui, glyph, tooltip));
        if button.inner.clicked() {
            pressed = Some(at);
        }
    }
    pressed
}

/// The kind's glyph, the name and, quieter after it, the summary, cut short at `right`.
fn title_line(ui: &Ui, left: f32, middle: f32, right: f32, step: &ChainStep<'_>, on: bool) {
    let colors = theme::colors();
    let painter = ui.painter();
    let (glyph_ink, name_ink) = if on {
        (colors.text_mid, colors.text_high)
    } else {
        (colors.text_low, colors.text_low)
    };
    let glyph = painter.text(
        pos2(left + theme::glyph::ROW / 2.0, middle),
        Align2::CENTER_CENTER,
        step.glyph,
        theme::icon(theme::glyph::ROW),
        glyph_ink,
    );
    let mut x = glyph.right() + theme::LINE_GAP;
    x = cut_run(
        painter,
        x,
        middle,
        right,
        step.name,
        theme::section(),
        name_ink,
    );
    let summary_ink = if step.faulty {
        colors.danger
    } else {
        colors.text_low
    };
    for part in step.summary {
        x += theme::LINE_GAP;
        if x >= right {
            return;
        }
        let dot = painter.text(
            pos2(x, middle),
            Align2::LEFT_CENTER,
            "·",
            theme::small(),
            colors.text_low,
        );
        x = cut_run(
            painter,
            dot.right() + theme::LINE_GAP,
            middle,
            right,
            part,
            theme::small(),
            summary_ink,
        );
    }
}

/// `text` from `left`, cut with an ellipsis at `right`; answers where it ends.
fn cut_run(
    painter: &egui::Painter,
    left: f32,
    middle: f32,
    right: f32,
    text: &str,
    font: egui::FontId,
    ink: Color32,
) -> f32 {
    let room = right - left;
    if room <= 0.0 {
        return left;
    }
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let galley = painter.layout_job(job);
    let width = galley.size().x;
    painter.galley(pos2(left, middle - galley.size().y / 2.0), galley, ink);
    left + width
}

/// The step's number on the rail: washed in the accent while on, sunken while off, ringed
/// in `danger` while it does not read.
fn disc(ui: &Ui, centre: egui::Pos2, step: &ChainStep<'_>, on: bool) {
    let colors = theme::colors();
    let (fill, ink) = match (on, step.open) {
        (false, _) => (colors.sunken, colors.text_low),
        (true, true) => (colors.accent_wash, colors.accent_soft),
        (true, false) => (colors.raised, colors.text_mid),
    };
    let ring = match (step.faulty, on && step.open) {
        (true, _) => colors.danger,
        (false, true) => colors.accent,
        (false, false) => colors.line,
    };
    let painter = ui.painter();
    painter.circle(centre, DISC_D / 2.0, fill, Stroke::new(1.0, ring));
    painter.text(
        centre,
        Align2::CENTER_CENTER,
        step.number.to_string(),
        theme::small(),
        ink,
    );
}

/// The seam before, between or after the cards: the rail through it, and on the rail a +
/// whose press answers, while `add` says a step may be added there. `above` and `below` say
/// whether a card stands there.
pub fn chain_seam(
    ui: &mut Ui,
    tooltip: &str,
    (above, below): (bool, bool),
    add: bool,
) -> (Rect, Option<Response>) {
    let colors = theme::colors();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), SEAM_H), Sense::hover());
    let rail_x = rect.left() + CHAIN_RAIL_W / 2.0;
    let from = if above { rect.top() } else { rect.center().y };
    let to = if below {
        rect.bottom()
    } else {
        rect.center().y
    };
    if from < to {
        let line = Stroke::new(1.0, colors.line);
        ui.painter()
            .line_segment([pos2(rail_x, from), pos2(rail_x, to)], line);
    }
    if !add {
        return (rect, None);
    }
    let square = Rect::from_center_size(
        pos2(rail_x, rect.center().y),
        vec2(theme::SMALL_ICON_SIZE, theme::SMALL_ICON_SIZE),
    );
    // The + stands on the rail's own ground, so the line does not run through it.
    ui.painter()
        .rect_filled(square.shrink(3.0), theme::R_CONTROL, colors.base);
    let mut child = ui.new_child(UiBuilder::new().max_rect(square));
    let pressed = small_icon_button(&mut child, icon::ADD, tooltip);
    (rect, Some(pressed))
}

/// Where a dragged card would land: an accent line across `seam` from the rail, a dot on it.
pub fn chain_drop_mark(ui: &Ui, seam: Rect) {
    let colors = theme::colors();
    let at = pos2(seam.left() + CHAIN_RAIL_W / 2.0, seam.center().y);
    let painter = ui.painter();
    painter.line_segment(
        [at, pos2(seam.right(), at.y)],
        Stroke::new(2.0, colors.accent),
    );
    painter.circle(at, 4.0, colors.accent, Stroke::NONE);
}

/// The card being dragged, as a small one by the pointer: its glyph and name.
pub fn chain_ghost(ctx: &egui::Context, glyph: &str, name: &str) {
    let Some(pointer) = ctx.pointer_interact_pos() else {
        return;
    };
    let colors = theme::colors();
    egui::Area::new(Id::new("chain-ghost"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pointer + vec2(12.0, -HEAD_H / 2.0))
        .interactable(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(colors.raised)
                .stroke(Stroke::new(1.0, colors.accent))
                .corner_radius(theme::R_SURFACE)
                .shadow(theme::window_shadow())
                .inner_margin(theme::CARD_MARGIN)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = theme::LINE_GAP;
                        let glyph = egui::RichText::new(glyph)
                            .font(theme::icon(theme::glyph::ROW))
                            .color(colors.text_mid);
                        ui.add(egui::Label::new(glyph).selectable(false));
                        let name = egui::RichText::new(name)
                            .font(theme::section())
                            .color(colors.text_high);
                        ui.add(egui::Label::new(name).selectable(false));
                    });
                });
        });
}
