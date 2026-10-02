use core_geometry::Scalar;
use egui::{Id, Rect, Stroke, vec2};

use crate::panels::Window;
use crate::panels::inspector::risk_tint;
use crate::preview::Preview;
use crate::state::{Doc, Machine};
use crate::ui::{icon, icon_button, theme};
use crate::workspace::{Mode, Section};

/// How many layers a second the play button steps through. Fast enough to read the shape
/// of a print, slow enough to see a layer.
const PLAY_LAYERS_PER_S: f32 = 30.0;

/// Width of the rail card, and the room its slider is given: tall enough to aim a layer
/// with, never so tall that it reaches the view tools in the corner above it.
const CARD_W: f32 = 30.0;
const SLIDER_H: f32 = 340.0;
const MIN_SLIDER_H: f32 = 110.0;
/// Points of the viewport the card's buttons and margins take beside the slider.
const CARD_CHROME_H: f32 = 150.0;

/// Thickness of the trough, and the width the slider is allocated, which is what egui
/// sizes the handle from: it draws one of `width / 2.5`.
const TROUGH_W: f32 = 4.0;
const HANDLE_SPAN: f32 = 17.0;
/// The handle is drawn as a bar this much flatter than it is wide, so that it reads as a
/// mark on the rail rather than as a knob over it.
const HANDLE_ASPECT: f32 = 0.42;

/// How long the readout stays up after the rail was last moved, seconds, and the gap
/// between it and the card it is pinned to.
const LINGER_S: f64 = 1.4;
const READOUT_GAP: f32 = 8.0;
const READOUT_PAD: egui::Vec2 = vec2(7.0, 4.0);
/// How far a risk's tick reaches out of the rail to the left, and into it.
const RISK_TICK_OUT: f32 = 5.0;
const RISK_TICK_IN: f32 = 4.0;

/// Where the viewport cuts the plate's contents this frame, plate millimetres, or `None`
/// when the whole model is drawn.
///
/// The Preview mode always cuts at the layer it is showing, because that is what a layer
/// preview is; the Prepare mode cuts only once the slider has been moved off the top.
pub fn cut_height(mode: Mode, section: &Section, preview: &Preview) -> Option<Scalar> {
    match mode {
        Mode::Preview => preview.layer_z(),
        Mode::Prepare => section.height_mm,
    }
}

/// How tall the slider can be inside a viewport of `viewport_h` points.
pub fn slider_height(viewport_h: f32) -> f32 {
    (viewport_h - CARD_CHROME_H).clamp(MIN_SLIDER_H, SLIDER_H)
}

/// Whether the rail has anything to scrub through, which is what decides if the card is
/// drawn at all. A hint for the empty cases lives in the inspector, not over the plate.
pub fn is_available(window: &Window) -> bool {
    match *window.mode {
        Mode::Preview => {
            window.machine.preview.is_building() || window.machine.preview.layer_count() > 0
        }
        Mode::Prepare => height_range(window.doc).is_some(),
    }
}

/// The card: the slider that moves the cut and the buttons that step it, with the height
/// it is at shown beside the handle while the rail is in use. Drawn down the right edge
/// of the viewport in both modes.
pub fn ui(ui: &mut egui::Ui, window: &mut Window, slider_h: f32) {
    ui.set_width(CARD_W);
    if window.machine.preview.is_building() {
        building(ui, window.machine);
        return;
    }

    let mut rail = memory(ui);
    let now = ui.input(|input| input.time);
    // The card's own rectangle is read from the frame before, because the readout has to
    // be placed beside a rail that has not been laid out yet. One frame of lag only shows
    // on the frame the card moves, which is when the window is resized.
    let hovered = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|pointer| rail.rect.contains(pointer));

    let mut touched = false;
    let mut scrub = None;
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 6.0);
        if icon_button(ui, icon::NEXT, "Up one layer").clicked() {
            step(window, 1);
            touched = true;
        }
        let moved = slider(ui, window, slider_h);
        touched |= moved.changed;
        scrub = Some(moved);
        if icon_button(ui, icon::PREVIOUS, "Down one layer").clicked() {
            step(window, -1);
            touched = true;
        }
        ui.add_space(2.0);
        footer(ui, window);
    });

    if touched {
        rail.touched_at_s = now;
    }
    if let Some(scrub) = scrub.filter(|_| showing(hovered, now - rail.touched_at_s)) {
        readout(ui, window, &scrub);
        // The readout goes away on its own, so the window has to be asked for the frame
        // that takes it away.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(LINGER_S));
    }

    rail.rect = ui.min_rect();
    ui.data_mut(|data| data.insert_temp(memory_id(), rail));
}

/// Whether the height is shown: while the pointer is on the rail, and for a moment after
/// it was last moved, so that a step or a drag says where it landed.
fn showing(hovered: bool, since_touch_s: f64) -> bool {
    hovered || since_touch_s < LINGER_S
}

/// What the rail remembers between frames: where it was drawn, and when it was last
/// moved, on egui's own clock.
#[derive(Debug, Clone, Copy)]
struct Rail {
    rect: Rect,
    touched_at_s: f64,
}

impl Default for Rail {
    fn default() -> Self {
        Self {
            rect: Rect::NOTHING,
            touched_at_s: f64::NEG_INFINITY,
        }
    }
}

fn memory_id() -> Id {
    Id::new("section-rail-state")
}

fn memory(ui: &egui::Ui) -> Rail {
    ui.data(|data| data.get_temp(memory_id()))
        .unwrap_or_default()
}

fn building(ui: &mut egui::Ui, machine: &mut Machine) {
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);
        ui.spinner();
        if icon_button(ui, icon::CANCEL, "Cancel").clicked() {
            machine.preview.cancel();
        }
    });
}

/// The height the cut is at, and which layer that is when there are layers to count,
/// drawn as a pill beside the handle rather than inside the card, so that the rail itself
/// stays one column wide.
fn readout(ui: &egui::Ui, window: &Window, scrub: &Scrub) {
    let mut text = match cut_height(*window.mode, &window.view.section, &window.machine.preview) {
        Some(z) => format!("{z:.3} mm"),
        None => "-- mm".to_owned(),
    };
    if *window.mode == Mode::Preview && window.machine.preview.layer_count() > 0 {
        text = format!(
            "{text}   {}/{}",
            window.machine.preview.layer() + 1,
            window.machine.preview.layer_count()
        );
    }

    let colors = theme::colors();
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(text, theme::mono(11.0), colors.text_high);
    let center = egui::pos2(
        ui.min_rect().left() - READOUT_GAP - galley.size().x / 2.0 - READOUT_PAD.x,
        scrub.handle_y,
    );
    let pill = Rect::from_center_size(center, galley.size() + 2.0 * READOUT_PAD);
    painter.rect(
        pill,
        theme::R_CONTROL,
        colors.panel,
        Stroke::new(1.0, colors.hairline),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        pill.center() - galley.size() / 2.0,
        galley,
        colors.text_high,
    );
}

/// What the slider reported this frame: where its handle ended up, in points down the
/// viewport, and whether the user moved it.
struct Scrub {
    handle_y: f32,
    changed: bool,
    /// The slider's own rectangle, which the marks are laid along.
    rail: Rect,
}

/// The scrubber itself. It counts layers where there are layers, and millimetres of the
/// scene's own height where there are not.
fn slider(ui: &mut egui::Ui, window: &mut Window, slider_h: f32) -> Scrub {
    let marks = match *window.mode {
        Mode::Preview => window
            .measured()
            .map(|measured| Marks::of(measured, slider_h as usize)),
        Mode::Prepare => None,
    };
    // egui wraps a vertical slider in a left-aligned column of the full width it is
    // given, so the rail only lands in the middle of the card if the room it is handed is
    // the width of the rail itself.
    ui.allocate_ui(vec2(HANDLE_SPAN, slider_h), |ui| {
        style_rail(ui, slider_h);
        let under = ui.painter().add(egui::Shape::Noop);
        let scrub = match *window.mode {
            Mode::Preview => layer_slider(ui, &mut window.machine.preview),
            Mode::Prepare => height_slider(ui, window),
        };
        if let Some(marks) = marks {
            marks.paint(ui, under, scrub.rail);
        }
        scrub
    })
    .inner
}

/// What the rail shows of the stack beside its handle: the cured area up the print, as
/// widths behind the trough, and a tick at every layer named as a risk.
struct Marks {
    /// The widest layer in each row of the rail, from the plate up, over the widest of all.
    widths: Vec<f32>,
    /// Where each risk sits up the stack, from zero to one, and its colour.
    risks: Vec<(f32, egui::Color32)>,
}

impl Marks {
    fn of(measured: &core_analysis::Measured, rows: usize) -> Self {
        let layers = measured.layers();
        let rows = rows.max(1);
        let mut widths = vec![0.0f32; rows];
        for (index, layer) in layers.iter().enumerate() {
            let row = index * rows / layers.len().max(1);
            widths[row] = widths[row].max(layer.area_mm2);
        }
        let widest = widths.iter().copied().fold(0.0, f32::max);
        if widest > 0.0 {
            widths.iter_mut().for_each(|width| *width /= widest);
        }
        let last = layers.len().saturating_sub(1).max(1) as f32;
        let risks = measured
            .risks()
            .iter()
            .map(|risk| (risk.layer as f32 / last, risk_tint(risk)))
            .collect();
        Self { widths, risks }
    }

    fn paint(&self, ui: &egui::Ui, under: egui::layers::ShapeIdx, rail: Rect) {
        let colors = theme::colors();
        let rows = self.widths.len() as f32;
        let profile = self
            .widths
            .iter()
            .enumerate()
            .filter(|(_, width)| **width > 0.0)
            .map(|(row, width)| {
                let y = handle_y(rail, (row as f32 + 0.5) / rows);
                let half = width * rail.width() / 2.0;
                let band = Rect::from_center_size(
                    egui::pos2(rail.center().x, y),
                    vec2(2.0 * half, (rail.height() / rows).max(1.0)),
                );
                egui::Shape::rect_filled(band, 0.0, colors.line)
            })
            .collect();
        ui.painter().set(under, egui::Shape::Vec(profile));

        for (fraction, tint) in &self.risks {
            let y = handle_y(rail, *fraction);
            let tick = [
                egui::pos2(rail.left() - RISK_TICK_OUT, y),
                egui::pos2(rail.left() + RISK_TICK_IN, y),
            ];
            ui.painter().line_segment(tick, Stroke::new(1.5, *tint));
        }
    }
}

/// The rail's own look: a thin trough sunk into the card, the accent under the handle,
/// and a flat bar for the handle itself. The handle takes its size from the width the
/// slider is allocated, so that width is the setting that makes it small.
fn style_rail(ui: &mut egui::Ui, slider_h: f32) {
    let colors = theme::colors();
    let spacing = ui.spacing_mut();
    spacing.slider_width = slider_h;
    spacing.slider_rail_height = TROUGH_W;
    spacing.interact_size.y = HANDLE_SPAN;

    let visuals = ui.visuals_mut();
    visuals.selection.bg_fill = colors.accent;
    visuals.widgets.inactive.bg_fill = colors.sunken;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.5, colors.text_high);
    visuals.widgets.hovered.bg_fill = colors.text_high;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, colors.text_high);
    visuals.widgets.active.bg_fill = colors.accent;
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, colors.text_high);
    // egui runs the accent past the handle's middle by the inactive corner radius, so
    // anything rounder than the trough would show the fill above the handle.
    let round = egui::CornerRadius::same((TROUGH_W / 2.0) as u8);
    for state in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        state.corner_radius = round;
    }
}

fn layer_slider(ui: &mut egui::Ui, preview: &mut Preview) -> Scrub {
    let last = preview.layer_count().saturating_sub(1);
    let mut index = preview.layer();
    let response = ui.add(vertical(egui::Slider::new(&mut index, 0..=last)));
    if response.changed() {
        preview.set_layer(index);
        preview.set_playing(false);
    }
    let fraction = if last == 0 {
        1.0
    } else {
        preview.layer() as f32 / last as f32
    };
    Scrub {
        handle_y: handle_y(response.rect, fraction),
        changed: response.changed(),
        rail: response.rect,
    }
}

fn height_slider(ui: &mut egui::Ui, window: &mut Window) -> Scrub {
    let Some((bottom, top)) = height_range(window.doc) else {
        return Scrub {
            handle_y: ui.min_rect().center().y,
            changed: false,
            rail: ui.min_rect(),
        };
    };
    let mut height = window.view.section.height_mm.unwrap_or(top);
    let response = ui.add(vertical(egui::Slider::new(&mut height, bottom..=top)));
    if response.changed() {
        // A slider parked at the top cuts nothing away, and saying so with `None` keeps
        // the section shading off as well as the plane.
        window.view.section.height_mm = (height < top).then_some(height);
    }
    Scrub {
        handle_y: handle_y(response.rect, fraction(bottom, top, height)),
        changed: response.changed(),
        rail: response.rect,
    }
}

/// Where down the slider's rectangle a handle at `fraction` of the range sits. egui pulls
/// the travel in by the handle's own half-width at each end, and the readout has to sit
/// on the handle rather than on the rail.
fn handle_y(rect: Rect, fraction: f32) -> f32 {
    let inset = rect.width() / 2.5 * HANDLE_ASPECT;
    let travel = rect.y_range().shrink(inset);
    egui::lerp(travel.max..=travel.min, fraction.clamp(0.0, 1.0))
}

/// Where a height sits in its range, from zero at the bottom to one at the top.
fn fraction(bottom: Scalar, top: Scalar, height: Scalar) -> f32 {
    if top <= bottom {
        return 1.0;
    }
    ((height - bottom) / (top - bottom)).clamp(0.0, 1.0)
}

fn vertical<'a>(slider: egui::Slider<'a>) -> egui::Slider<'a> {
    slider
        .vertical()
        .show_value(false)
        .handle_shape(egui::style::HandleShape::Rect {
            aspect_ratio: HANDLE_ASPECT,
        })
}

/// Play in the preview, where there is a stack to run through; a way back to the whole
/// model in the prepare mode, where the cut is the user's own.
fn footer(ui: &mut egui::Ui, window: &mut Window) {
    match *window.mode {
        Mode::Preview => {
            let playing = window.machine.preview.is_playing();
            let (glyph, tooltip) = if playing {
                (icon::PAUSE, "Pause")
            } else {
                (icon::PLAY, "Play")
            };
            if icon_button(ui, glyph, tooltip).clicked() {
                window.machine.preview.set_playing(!playing);
            }
        }
        Mode::Prepare => {
            if icon_button(ui, icon::SECTION, "Show the whole model").clicked() {
                window.view.section.height_mm = None;
            }
        }
    }
}

/// Moves the cut by `layers`, which is a layer of the stack in the preview and a layer
/// height of the print in the prepare mode.
fn step(window: &mut Window, layers: i64) {
    match *window.mode {
        Mode::Preview => {
            window.machine.preview.step(layers);
            window.machine.preview.set_playing(false);
        }
        Mode::Prepare => {
            let Some((bottom, top)) = height_range(window.doc) else {
                return;
            };
            let height = window.view.section.height_mm.unwrap_or(top);
            let moved = height + layers as Scalar * window.machine.slicing.layer_height_mm();
            window.view.section.height_mm = Some(moved.clamp(bottom, top)).filter(|z| *z < top);
        }
    }
}

/// The height the cut may be moved between: the bottom and the top of everything visible
/// on the plate. `None` when there is nothing there.
pub(crate) fn height_range(doc: &Doc) -> Option<(Scalar, Scalar)> {
    let bounds = doc.scene.world_bounds()?;
    let (bottom, top) = (bounds.mins.z, bounds.maxs.z);
    (top > bottom).then_some((bottom, top))
}

/// Advances the stack while the play button is down. Returns whether the window has to
/// keep repainting.
pub fn animate(ctx: &egui::Context, preview: &mut Preview) -> bool {
    if !preview.is_playing() || preview.layer_count() == 0 {
        return false;
    }
    let step = (ctx.input(|input| input.stable_dt) * PLAY_LAYERS_PER_S).round() as i64;
    preview.step(step.max(1));
    if preview.layer() + 1 == preview.layer_count() {
        preview.set_playing(false);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rail_marks_the_widest_layer_full_width_and_an_island_where_it_starts() {
        use core_analysis::{Measured, cure};
        use core_raster::{LayerRuns, PixelPitch};

        // Four layers on a 10 x 10 panel of 1 mm pixels: a pixel, a row, a pixel, and a
        // pixel standing on nothing.
        let layer = |lit: &[u32]| {
            let mut builder = LayerRuns::builder(10, 10);
            for &at in lit {
                builder.pad_to(at);
                builder.push(1, 255);
            }
            cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 })
        };
        let mut measured = Measured::new(0);
        for lit in [&[0][..], &[0, 1, 2, 3], &[0], &[55]] {
            measured.push(layer(lit), 0.05);
        }

        let marks = Marks::of(&measured, 4);
        assert_eq!(marks.widths, vec![0.25, 1.0, 0.25, 0.25]);
        assert_eq!(marks.risks.len(), 1);
        assert!((marks.risks[0].0 - 1.0).abs() < 1e-6, "the top layer");
    }

    #[test]
    fn the_prepare_mode_cuts_where_the_slider_was_left() {
        let section = Section {
            height_mm: Some(3.5),
        };
        let preview = Preview::default();
        assert_eq!(
            cut_height(Mode::Prepare, &section, &preview),
            Some(3.5),
            "the prepare mode carries its own height"
        );
    }

    #[test]
    fn the_preview_mode_cuts_at_the_layer_it_is_showing() {
        // An empty preview is showing no layer, so it asks for no cut whatever the
        // prepare mode was left at.
        let section = Section {
            height_mm: Some(3.5),
        };
        assert_eq!(
            cut_height(Mode::Preview, &section, &Preview::default()),
            None
        );
    }

    #[test]
    fn the_slider_never_outgrows_its_viewport() {
        assert_eq!(slider_height(2000.0), SLIDER_H, "a tall window caps it");
        assert_eq!(slider_height(0.0), MIN_SLIDER_H, "a short one floors it");
        let middle = slider_height(400.0);
        assert!(
            (MIN_SLIDER_H..=SLIDER_H).contains(&middle),
            "{middle} is inside the range the card allows"
        );
    }

    #[test]
    fn the_height_is_shown_while_the_rail_is_in_use_and_for_a_moment_after() {
        assert!(showing(true, 30.0), "a pointer on the rail shows it");
        assert!(showing(false, LINGER_S / 2.0), "a fresh move shows it");
        assert!(
            !showing(false, LINGER_S + 0.1),
            "a rail nobody has touched shows nothing over the model"
        );
        assert!(
            !showing(false, f64::INFINITY),
            "a rail that was never moved shows nothing"
        );
    }

    #[test]
    fn the_handle_sits_at_the_top_of_its_travel_at_the_top_of_the_range() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 100.0), vec2(HANDLE_SPAN, 200.0));
        let top = handle_y(rect, 1.0);
        let bottom = handle_y(rect, 0.0);
        assert!(top < bottom, "a fuller value sits higher up the screen");
        assert!(
            rect.y_range().contains(top) && rect.y_range().contains(bottom),
            "the handle stays inside the rail it belongs to"
        );
        let middle = handle_y(rect, 0.5);
        assert!(
            (middle - rect.center().y).abs() < 0.5,
            "half of the range is the middle of the rail"
        );
    }

    #[test]
    fn a_range_of_no_height_reads_as_full() {
        assert_eq!(fraction(5.0, 5.0, 5.0), 1.0);
        assert_eq!(fraction(0.0, 10.0, 2.5), 0.25);
        assert_eq!(
            fraction(0.0, 10.0, 40.0),
            1.0,
            "a value past the top clamps"
        );
    }
}
