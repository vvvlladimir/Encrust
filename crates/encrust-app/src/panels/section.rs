use core_geometry::Scalar;
use egui::{Align, Layout, vec2};

use crate::i18n::tr;
use crate::panels::Window;
use crate::pick::Shown;
use crate::preview::Preview;
use crate::shortcuts::{self, Action};
use crate::ui::{icon, icon_button, icon_toggle, mode_column, theme};
use crate::workspace::{Mode, Section};

mod playback;
mod reading;
mod track;
pub use playback::{animate, animate_section};
pub(crate) use playback::{height_range, play, step};
use reading::Reading;
use track::track;

/// How many layers a second the play button steps through. Fast enough to read the shape
/// of a print, slow enough to see a layer.
const PLAY_LAYERS_PER_S: f32 = 30.0;

/// The readout under the views; the box the layer stands in, and the least room the typed
/// layer is given, so an emptied box still takes a click; and the room between groups.
const READOUT_H: f32 = 62.0;
const LAYER_BOX_H: f32 = 26.0;
const LAYER_FIELD_MIN_W: f32 = 8.0;
const GAP: f32 = 10.0;

/// The track: how thick its rule and the thumbs on it.
const RULE_W: f32 = 3.0;
const HANDLE: egui::Vec2 = vec2(26.0, 12.0);
/// How many ticks the scale at the right carries, how long each is and how far in from
/// the edge.
const TICKS: usize = 18;
const TICK_W: f32 = 8.0;
const TICK_INSET: f32 = 4.0;
/// The cured-area profile: one width every so many points up the track, the widest
/// standing so far either side of the rule.
const PROFILE_PITCH: f32 = 2.0;
const PROFILE_HALF_W: f32 = 22.0;
/// A risk's bar across the rule, as it stands and under the pointer, and how far above or
/// below it a press still takes it, points.
const RISK: egui::Vec2 = vec2(28.0, 3.0);
const RISK_HOT: egui::Vec2 = vec2(32.0, 5.0);
const RISK_REACH: f32 = 3.0;

/// Where the viewport cuts the plate's contents, plate millimetres, or `None` for the whole
/// model. Preview always cuts at its layer; Prepare only once the strip leaves the top.
pub fn cut_height(mode: Mode, section: &Section, preview: &Preview) -> Option<Scalar> {
    match mode {
        Mode::Preview => preview.section_z(),
        Mode::Prepare => section.height_mm,
    }
}

/// Where the viewport cuts away what stands below, plate millimetres, or `None`. The floor
/// never passes the top cut: Preview keeps at least the layer shown.
pub fn floor_height(mode: Mode, section: &Section, preview: &Preview) -> Option<Scalar> {
    let floor = section.floor_mm?;
    match mode {
        Mode::Preview if preview.layer_count() == 0 => None,
        Mode::Preview => Some(floor.min(preview.bottom_of(preview.layer())?)),
        Mode::Prepare => Some(section.height_mm.map_or(floor, |top| floor.min(top))),
    }
}

/// The heights the viewport leaves drawn this frame, which is all a click can land on.
pub fn shown(window: &Window) -> Shown {
    let (mode, section, preview) = (*window.mode, &window.view.section, &window.machine.preview);
    Shown {
        floor_mm: floor_height(mode, section, preview),
        top_mm: cut_height(mode, section, preview),
    }
}

/// The strip down the stage's right edge, in every view: view, cut, layer steps, the track
/// and play. See `docs/decisions/0217`, `0221`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.add_space(theme::TOOLBAR_PAD);
        views(ui, window);
        ui.add_space(GAP);
        let readout = Reading::of(window);
        if let Some(layer) = readout.show(ui) {
            go_to_layer(window, layer);
        }
        ui.add_space(GAP);
        let movable = readout.at.is_some();
        ui.add_enabled_ui(movable, |ui| step_button(ui, window, 1));
        let below = 2.0 * theme::ICON_SIZE + GAP + 3.0 * ui.spacing().item_spacing.y;
        let below = below + theme::TOOLBAR_PAD;
        let track_h = (ui.available_height() - below).max(0.0);
        track(ui, window, track_h, &readout);
        ui.add_enabled_ui(movable, |ui| {
            step_button(ui, window, -1);
            ui.add_space(GAP);
            play_button(ui, window);
        });
    });
}

/// The three ways of looking at the plate: the model, the model and its layer side by
/// side, and the layer alone. The two that read the layers are the Preview mode.
fn views(ui: &mut egui::Ui, window: &mut Window) {
    let showing = showing(window);
    let keys = shortcuts::text(Action::View(1));
    let views = [
        (icon::MODEL, tr!("view-model")),
        (icon::SIDE_BY_SIDE, tr!("view-side-by-side")),
        (icon::MASK, tr!("view-mask")),
    ];
    let mut picked = None;
    mode_column(ui, |ui| {
        for (index, (glyph, name)) in views.iter().enumerate() {
            let tooltip = format!("{name}  {keys}");
            if icon_toggle(ui, glyph, &tooltip, index == showing).clicked() {
                picked = Some(index);
            }
        }
    });
    if let Some(index) = picked {
        show_view(window, index);
    }
}

/// Which of the three views is up: the model, the two side by side, or the layer alone.
fn showing(window: &Window) -> usize {
    match (*window.mode, window.view.options.mask_only) {
        (Mode::Prepare, _) => 0,
        (Mode::Preview, false) => 1,
        (Mode::Preview, true) => 2,
    }
}

fn show_view(window: &mut Window, index: usize) {
    *window.mode = if index == 0 {
        Mode::Prepare
    } else {
        Mode::Preview
    };
    window.view.options.mask_only = index == 2;
}

/// Moves `step` views along the three, wrapping round at either end.
pub(crate) fn turn_view(window: &mut Window, step: i8) {
    let index = (showing(window) as i64 + i64::from(step)).rem_euclid(3);
    show_view(window, index as usize);
}

/// A layer up over the track, or a layer down under it.
fn step_button(ui: &mut egui::Ui, window: &mut Window, layers: i64) {
    let glyph = if layers > 0 {
        icon::LAYER_UP
    } else {
        icon::LAYER_DOWN
    };
    if icon_button(ui, glyph, &shortcuts::tooltip(Action::Step(layers))).clicked() {
        step(window, layers);
    }
}

/// Runs the cut up the print, or stops it. Moving the cut back to the top shows the whole
/// model.
fn play_button(ui: &mut egui::Ui, window: &mut Window) {
    let playing = match *window.mode {
        Mode::Preview => window.machine.preview.is_playing(),
        Mode::Prepare => window.view.section.playing,
    };
    let (glyph, tooltip) = if playing {
        (icon::PAUSE, tr!("section-pause"))
    } else {
        (icon::PLAY, shortcuts::tooltip(Action::Play))
    };
    if icon_button(ui, glyph, &tooltip).clicked() {
        play(window);
    }
}

/// Moves the cut to `layer`, counted from one at the plate as the readout counts it.
fn go_to_layer(window: &mut Window, layer: usize) {
    match *window.mode {
        Mode::Preview => {
            let preview = &mut window.machine.preview;
            preview.set_layer(layer.saturating_sub(1));
            preview.set_playing(false);
        }
        Mode::Prepare => {
            let Some(range) = height_range(window.doc) else {
                return;
            };
            let layer_height_mm = window.machine.slicing.layer_height_mm();
            window.view.section.height_mm = height_of_layer(layer, range, layer_height_mm);
            window.view.section.playing = false;
        }
    }
}

/// Where a cut at `layer` stands, `layer` layer heights over the bottom of the range, or
/// `None` at the top and past it, where the whole model is shown.
fn height_of_layer(
    layer: usize,
    (bottom, top): (Scalar, Scalar),
    layer_height_mm: Scalar,
) -> Option<Scalar> {
    let height = bottom + layer as Scalar * layer_height_mm;
    (height < top).then_some(height)
}

/// Which layer from the plate up a cut at `height` is, counted in layers of
/// `layer_height_mm` from the bottom of the range, and how many layers the range holds.
fn layer_at(
    height: Scalar,
    (bottom, top): (Scalar, Scalar),
    layer_height_mm: Scalar,
) -> (usize, usize) {
    if layer_height_mm <= 0.0 || top <= bottom {
        return (0, 0);
    }
    let layers = ((top - bottom) / layer_height_mm).ceil() as usize;
    let layer = ((height - bottom) / layer_height_mm).round().max(0.0) as usize;
    (layer.min(layers), layers)
}

#[cfg(test)]
mod tests;
