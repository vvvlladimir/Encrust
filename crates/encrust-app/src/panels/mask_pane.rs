use core_analysis::Risk;
use core_raster::RasterSettings;
use egui::{Align2, Rect, Sense, StrokeKind, UiBuilder, Vec2};

use crate::i18n::tr;
use crate::panels::inspector::risk_tint;
use crate::preview::{PixelRect, Preview};
use crate::state::Machine;
use crate::status::Status;
use crate::ui::theme;

/// Points between the mask and the edge of the pane it is centred in.
const INSET: f32 = 18.0;
/// How much a point of scrolling zooms by: a notch of a wheel is about a fifth.
const ZOOM_PER_POINT: f32 = 0.004;
/// The ring around an issue on the shown layer, points: wide enough to find a pixel by.
const RISK_RING_R: f32 = 12.0;

/// The exposure mask, beside the model rather than under the inspector.
///
/// A 300 point column cannot show an 8520 pixel panel, so Preview gives the mask a pane of
/// the stage, or all of it; see `docs/decisions/0103`. The picture fills the pane, under
/// the cards as the model does; the caption keeps to `clear`, the room they leave.
pub fn ui(ui: &mut egui::Ui, machine: &mut Machine, clear: Rect) {
    let Some(settings) = machine.slicing.raster_settings() else {
        caption(ui, clear, &tr!("mask-pick-machine"));
        return;
    };
    if machine.preview.layer_count() == 0 {
        caption(ui, clear, &tr!("mask-no-stack"));
        return;
    }

    let risks = layer_risks(machine);
    let full = picture(
        ui,
        &mut machine.preview,
        &settings,
        &risks,
        &mut machine.status,
    );
    let scale = if full {
        1
    } else {
        machine.preview.downsample_factor(&settings)
    };
    let zoom = machine.preview.view.zoom;
    let hint = if zoom > 1.0 {
        tr!("mask-zoomed", zoom = format!("{zoom:.1}"))
    } else {
        tr!("mask-how")
    };
    caption(ui, clear, &tr!("mask-caption", scale = scale, hint = hint));
}

/// What the check found on the layer shown, once the stack is measured.
fn layer_risks(machine: &Machine) -> Vec<Risk> {
    let layer = machine.preview.layer();
    machine
        .slicing
        .raster_settings()
        .and_then(|settings| machine.preview.measured(&settings, machine.slicing.fold()))
        .map(|measured| {
            let risks = measured.risks();
            risks
                .into_iter()
                .filter(|risk| risk.layer == layer)
                .collect()
        })
        .unwrap_or_default()
}

fn caption(ui: &egui::Ui, clear: Rect, text: &str) {
    let room = ui.max_rect().intersect(clear);
    ui.painter().text(
        room.left_top() + Vec2::splat(INSET),
        Align2::LEFT_TOP,
        text,
        theme::small(),
        theme::colors().text_low,
    );
}

/// Draws the mask as the view has it and answers the pointer. Returns whether what is on
/// screen is the panel's own pixels rather than the shrunk picture.
///
/// Only a failure reaches the stage notice: a picture that is drawn says so by being there,
/// and reporting it every frame would wipe out whatever else is written.
fn picture(
    ui: &mut egui::Ui,
    preview: &mut Preview,
    settings: &RasterSettings,
    risks: &[Risk],
    status: &mut Status,
) -> bool {
    let texture = match preview.texture(ui.ctx(), settings) {
        Ok(texture) => texture,
        Err(error) => {
            *status = Status::failed(&error.context(tr!("mask-cannot-draw")));
            return false;
        }
    };

    // The panel's pixels are not square in millimetres, so the picture is sized by the
    // area it covers rather than by the texture.
    let panel_px = Vec2::new(settings.width_px as f32, settings.height_px as f32);
    let panel_mm = panel_px * Vec2::new(settings.pitch.x, settings.pitch.y);
    let room = ui.max_rect().shrink(INSET);
    let fit_rect = Rect::from_center_size(room.center(), fit(room.size(), panel_mm.x / panel_mm.y));

    steer(ui, preview, room, fit_rect, panel_px);
    let placed = preview.view.placement(fit_rect, panel_px);

    let mut clipped = ui.new_child(UiBuilder::new().max_rect(room));
    clipped.set_clip_rect(room.intersect(ui.clip_rect()));
    let colors = theme::colors();
    clipped.painter().rect(
        placed,
        theme::R_SURFACE,
        colors.sunken,
        egui::Stroke::new(1.0, colors.hairline),
        StrokeKind::Inside,
    );
    egui::Image::from_texture((texture.id(), placed.size())).paint_at(&clipped, placed);

    let visible = visible_pixels(room.intersect(placed), placed, settings);
    let full = match preview.detail(ui.ctx(), settings, visible) {
        Ok(Some((region, detail))) => {
            let at = |x: u32, y: u32| {
                placed.min + Vec2::new(x as f32, y as f32) / panel_px * placed.size()
            };
            let rect = Rect::from_min_max(at(region.x0, region.y0), at(region.x1, region.y1));
            egui::Image::from_texture((detail.id(), rect.size())).paint_at(&clipped, rect);
            true
        }
        Ok(None) => false,
        Err(error) => {
            *status = Status::failed(&error.context(tr!("mask-cannot-draw-full")));
            false
        }
    };
    ring_risks(&clipped, risks, placed, panel_mm);
    full
}

/// A ring in its colour around each issue on the layer, where the check found it: a cup,
/// a thin run or a neck is not drawn into the pixels the way an island is.
fn ring_risks(ui: &egui::Ui, risks: &[Risk], placed: Rect, panel_mm: Vec2) {
    for risk in risks {
        let along = Vec2::new(risk.at_mm[0], risk.at_mm[1]) / panel_mm;
        let centre = placed.min + along * placed.size();
        let tint = risk_tint(risk);
        let painter = ui.painter();
        painter.circle_filled(centre, RISK_RING_R, tint.gamma_multiply(0.18));
        painter.circle_stroke(centre, RISK_RING_R, egui::Stroke::new(2.0, tint));
    }
}

/// Scrolling or pinching over the pane zooms about the cursor, dragging moves the picture,
/// and a double click shows the whole panel again.
fn steer(ui: &egui::Ui, preview: &mut Preview, room: Rect, fit_rect: Rect, panel_px: Vec2) {
    // Not focusable: the picture is steered by the pointer alone.
    let response = ui.interact(room, ui.id().with("mask-view"), Sense::CLICK | Sense::DRAG);
    let view = &mut preview.view;
    if response.double_clicked() {
        *view = Default::default();
        return;
    }
    if response.dragged() {
        view.pan(response.drag_delta(), fit_rect, panel_px);
    }
    if let Some(at) = response.hover_pos() {
        let (scroll, pinch) = ui.input(|input| (input.smooth_scroll_delta.y, input.zoom_delta()));
        let factor = pinch * (scroll * ZOOM_PER_POINT).exp();
        if factor != 1.0 {
            view.zoom_at(factor, at, fit_rect, panel_px);
        }
    }
}

/// The panel pixels a rectangle of the screen shows, whole pixels out.
fn visible_pixels(shown: Rect, placed: Rect, settings: &RasterSettings) -> PixelRect {
    let panel = |at: egui::Pos2| {
        let along = (at - placed.min) / placed.size();
        Vec2::new(
            along.x * settings.width_px as f32,
            along.y * settings.height_px as f32,
        )
    };
    let (min, max) = (panel(shown.min), panel(shown.max));
    let clamp = |value: f32, extent: u32| (value.max(0.0) as u32).min(extent);
    PixelRect {
        x0: clamp(min.x.floor(), settings.width_px),
        y0: clamp(min.y.floor(), settings.height_px),
        x1: clamp(max.x.ceil(), settings.width_px),
        y1: clamp(max.y.ceil(), settings.height_px),
    }
}

/// The largest box of `aspect` width over height that fits inside `available`.
fn fit(available: Vec2, aspect: f32) -> Vec2 {
    let available = available.max(Vec2::ZERO);
    if aspect <= 0.0 || !aspect.is_finite() {
        return available;
    }
    if available.x / aspect <= available.y {
        Vec2::new(available.x, available.x / aspect)
    } else {
        Vec2::new(available.y * aspect, available.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_box_is_filled_to_its_height() {
        let size = fit(Vec2::new(400.0, 100.0), 1.0);
        assert_eq!(size, Vec2::new(100.0, 100.0));
    }

    #[test]
    fn a_tall_box_is_filled_to_its_width() {
        let size = fit(Vec2::new(100.0, 400.0), 2.0);
        assert_eq!(size, Vec2::new(100.0, 50.0));
    }

    #[test]
    fn a_pane_that_has_no_room_left_asks_for_nothing() {
        let size = fit(Vec2::new(-10.0, 50.0), 1.5);
        assert_eq!(size, Vec2::new(0.0, 0.0));
    }
}
