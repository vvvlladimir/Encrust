//! What stands over the mask: tags naming the layer, the panel and the issues on it at the
//! top left, and the zoom at the bottom right.

use core_analysis::Risk;
use egui::{Align2, Id, Rect, Vec2, pos2, vec2};

use crate::i18n::tr;
use crate::preview::Preview;
use crate::ui::{ZoomPress, card, tag, tag_w, theme, zoom_bar};

/// Points between the tags or the zoom and the pane's edge, and between two tags.
const PAD: f32 = 12.0;
const TAG_GAP: f32 = 6.0;
/// How much one press of a zoom button zooms by.
const ZOOM_STEP: f32 = 1.25;

/// The layer and its height, the panel's pixels, and any issue on the layer in `danger`.
pub(super) fn tags(
    ui: &mut egui::Ui,
    clear: Rect,
    preview: &Preview,
    panel_px: Vec2,
    risks: &[Risk],
) {
    let colors = theme::colors();
    let layer = preview.layer();
    let height_mm = preview
        .plan()
        .and_then(|plan| plan.top_of(layer))
        .unwrap_or_default();
    let quiet = (colors.text_mid, colors.panel);
    let mut shown = vec![
        (
            tr!(
                "mask-layer",
                layer = layer + 1,
                height = format!("{height_mm:.2}")
            ),
            quiet,
        ),
        (
            tr!(
                "mask-pixels",
                width = panel_px.x as u32,
                height = panel_px.y as u32
            ),
            quiet,
        ),
    ];
    if !risks.is_empty() {
        shown.push((
            tr!("mask-issues", count = risks.len()),
            (colors.danger, colors.danger_wash),
        ));
    }
    let mut x = clear.left() + PAD;
    let y = clear.top() + PAD + theme::ROW_H / 2.0;
    for (index, (text, tint)) in shown.iter().enumerate() {
        x += tag_w(ui, text);
        tag(ui, pos2(x, y), Id::new(("mask-tag", index)), text, *tint);
        x += TAG_GAP;
    }
}

/// The zoom in a card at the bottom right. `fit` is the rect the whole panel fills at no
/// zoom.
pub(super) fn zoom(ui: &egui::Ui, clear: Rect, preview: &mut Preview, fit: Rect, panel_px: Vec2) {
    let pressed = egui::Area::new(Id::new("mask-zoom"))
        .order(egui::Order::Middle)
        .fixed_pos(clear.right_bottom() - vec2(PAD, PAD))
        .pivot(Align2::RIGHT_BOTTOM)
        .show(ui.ctx(), |ui| {
            card()
                .inner_margin(3)
                .show(ui, |ui| {
                    ui.horizontal(|ui| zoom_bar(ui, preview.view.zoom)).inner
                })
                .inner
        })
        .inner;
    let view = &mut preview.view;
    match pressed {
        Some(ZoomPress::Out) => view.zoom_at(1.0 / ZOOM_STEP, fit.center(), fit, panel_px),
        Some(ZoomPress::In) => view.zoom_at(ZOOM_STEP, fit.center(), fit, panel_px),
        Some(ZoomPress::Whole) => *view = Default::default(),
        None => {}
    }
}
