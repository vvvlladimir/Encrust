//! What stands over the mask at its top right, beside the layer strip: the zoom, and under
//! it a tag naming the panel.

use egui::{Align2, Id, Rect, Vec2, pos2};

use crate::i18n::tr;
use crate::preview::Preview;
use crate::ui::{ZoomPress, card, tag, theme, zoom_bar};

/// Points between the zoom and the tag under it.
const TAG_GAP: f32 = 6.0;
/// How much one press of a zoom button zooms by.
const ZOOM_STEP: f32 = 1.25;

/// The panel's pixels, under the `zoom` card and flush with its right.
pub(super) fn tags(ui: &mut egui::Ui, zoom: Rect, panel_px: Vec2) {
    let colors = theme::colors();
    let text = tr!(
        "mask-pixels",
        width = panel_px.x as u32,
        height = panel_px.y as u32
    );
    let x = zoom.right();
    let y = zoom.bottom() + TAG_GAP + theme::ROW_H / 2.0;
    let tint = (colors.text_mid, colors.panel);
    tag(ui, pos2(x, y), Id::new("mask-tag"), &text, tint);
}

/// The zoom in a card at the top right of `clear`, from the top of the `pane` level with the
/// layer strip. `fit` is the rect the whole panel fills at no zoom. Answers where it stands.
pub(super) fn zoom(
    ui: &egui::Ui,
    (pane, clear): (Rect, Rect),
    preview: &mut Preview,
    fit: Rect,
    panel_px: Vec2,
) -> Rect {
    let shown = egui::Area::new(Id::new("mask-zoom"))
        .order(egui::Order::Middle)
        .fixed_pos(pos2(clear.right(), pane.top() + theme::FLOAT_GAP))
        .pivot(Align2::RIGHT_TOP)
        .show(ui.ctx(), |ui| {
            card()
                .inner_margin(3)
                .show(ui, |ui| {
                    ui.horizontal(|ui| zoom_bar(ui, preview.view.zoom)).inner
                })
                .inner
        });
    let view = &mut preview.view;
    match shown.inner {
        Some(ZoomPress::Out) => view.zoom_at(1.0 / ZOOM_STEP, fit.center(), fit, panel_px),
        Some(ZoomPress::In) => view.zoom_at(ZOOM_STEP, fit.center(), fit, panel_px),
        Some(ZoomPress::Whole) => *view = Default::default(),
        None => {}
    }
    shown.response.rect
}
