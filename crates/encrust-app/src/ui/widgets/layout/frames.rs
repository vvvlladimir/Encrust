use egui::layers::ShapeIdx;
use egui::{Frame, Sense, Ui, vec2};

use crate::ui::theme;
use crate::ui::widgets::section::trailing_hairline;

/// How far what a switch reveals is set in under it, points.
const NEST_INDENT: i8 = 14;

/// What a switch reveals, set in under it with a rule down its left edge, so it reads as
/// that switch's own.
pub fn nested<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let margin = egui::Margin {
        left: NEST_INDENT,
        ..egui::Margin::ZERO
    };
    let shown = Frame::new().inner_margin(margin).show(ui, add);
    let rect = shown.response.rect;
    ui.painter().vline(
        rect.left() + 4.0,
        rect.y_range(),
        egui::Stroke::new(1.0, theme::colors().line),
    );
    shown.inner
}

/// The band under a card's heading that holds its preset, padded 10 × 12, a hairline under it.
pub fn inset<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let inner = Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = theme::NOTE_GAP;
            add(ui)
        })
        .inner;
    hairline(ui);
    inner
}

/// A tool's modes in its heading: icon toggles sunk together in one bordered group.
pub fn mode_group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let colors = theme::colors();
    // Held to the group's own height: a heading's row is taller, and a frame laid in it
    // unbounded would reach from its top.
    let height = theme::ICON_SIZE + 2.0 * MODE_GROUP_PAD;
    let size = vec2(ui.available_width(), height);
    let layout = egui::Layout::right_to_left(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        Frame::new()
            .fill(colors.sunken)
            .stroke(egui::Stroke::new(1.0, colors.hairline))
            .corner_radius(around_controls(MODE_GROUP_PAD))
            .inner_margin(MODE_GROUP_PAD)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = theme::ICON_GAP;
                add(ui)
            })
            .inner
    })
    .inner
}

/// Room inside a mode group around its toggles.
const MODE_GROUP_PAD: f32 = 2.0;

/// A [`mode_group`] standing as a column, as narrow as its toggles, placed by its parent's
/// layout: the toggles `add` puts in stacked.
pub fn mode_column<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let colors = theme::colors();
    Frame::new()
        .fill(colors.sunken)
        .stroke(egui::Stroke::new(1.0, colors.hairline))
        .corner_radius(around_controls(MODE_GROUP_PAD))
        .inner_margin(MODE_GROUP_PAD)
        .show(ui, |ui| {
            ui.set_width(theme::ICON_SIZE);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = theme::ICON_GAP;
                add(ui)
            })
            .inner
        })
        .inner
}

/// The rounding of a bordered group whose controls sit `pad` inside its 1-point stroke,
/// concentric with theirs.
fn around_controls(pad: f32) -> egui::CornerRadius {
    egui::CornerRadius::same(theme::R_CONTROL.nw + (pad + 1.0) as u8)
}

/// A body that scrolls once it outgrows the card, and a foot under it that stays in
/// view. `max_h` is the card's whole height, points, what is already drawn in it included.
///
/// The foot is measured as it is drawn, so the body is held to last frame's measure, and
/// a frame whose foot changed height is drawn again.
pub fn body_and_foot<T>(
    ui: &mut Ui,
    max_h: f32,
    state: &mut T,
    body: impl FnOnce(&mut Ui, &mut T),
    foot: impl FnOnce(&mut Ui, &mut T),
) {
    let id = ui.id().with("foot-h");
    let foot_h = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or(0.0);
    let room = (max_h - ui.min_rect().height() - foot_h).max(0.0);
    ui.data_mut(|data| data.remove::<(ShapeIdx, f32)>(trailing_hairline()));
    egui::ScrollArea::vertical()
        .max_height(room)
        .auto_shrink([false, true])
        .show(ui, |ui| body(ui, state));
    let body_bottom = ui.min_rect().bottom();
    let drawn = ui.scope(|ui| foot(ui, state)).response.rect.height();
    // With no foot the last block's hairline would stand square over the card's rounded
    // edge, so the card ends on that block instead.
    let trailing = ui.data(|data| data.get_temp::<(ShapeIdx, f32)>(trailing_hairline()));
    if let Some((shape, bottom)) = trailing
        && drawn <= 0.0
        && (bottom - body_bottom).abs() < 0.5
    {
        ui.painter().set(shape, egui::Shape::Noop);
    }
    if (drawn - foot_h).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, drawn));
        ui.ctx()
            .request_discard("the foot of a card changed height");
    }
}

/// A box set into a page on `raised`, ringed in a hairline: a release found, under its name.
pub fn boxed<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let colors = theme::colors();
    Frame::new()
        .fill(colors.raised)
        .stroke(egui::Stroke::new(1.0, colors.hairline))
        .corner_radius(theme::R_SURFACE)
        .inner_margin(egui::Margin::same(BOXED_PAD))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = theme::NOTE_GAP;
            add(ui)
        })
        .inner
}

/// Room inside a [`boxed`] box, points.
const BOXED_PAD: i8 = 14;

/// The frame of anything that floats over the viewport.
pub fn card() -> Frame {
    Frame::new()
        .fill(theme::colors().panel)
        .stroke(egui::Stroke::new(1.0, theme::colors().hairline))
        .corner_radius(theme::R_SURFACE)
        .shadow(theme::card_shadow())
}

/// The foot of a card down a side of the stage, set apart on the window's colour and
/// rounded to fit inside the card's outline.
pub fn card_foot() -> Frame {
    let r = theme::R_SURFACE.sw.saturating_sub(1);
    Frame::new()
        .fill(theme::colors().base)
        .inner_margin(theme::PANEL_MARGIN)
        .corner_radius(egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: r,
            se: r,
        })
}

/// A one pixel divider across the full width of whatever is drawing it.
pub fn hairline(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, theme::colors().hairline);
}

/// A one pixel divider standing `height` points tall, between groups laid out in a row.
pub fn vertical_hairline(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, height), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, theme::colors().hairline);
}
