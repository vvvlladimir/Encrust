use super::PLAY_LAYERS_PER_S;
use crate::panels::Window;
use crate::preview::Preview;
use crate::state::Doc;
use crate::workspace::{Mode, Section};
use core_geometry::Scalar;

/// Moves the cut by `layers`: stack layers in Preview, print layer heights in Prepare. Keys
/// and buttons both come here; see `docs/decisions/0061`.
pub(crate) fn step(window: &mut Window, layers: i64) {
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
            window.view.section.playing = false;
        }
    }
}

/// Starts or stops the transport: the stack in the preview, the cut up the model in the
/// prepare mode.
pub(crate) fn play(window: &mut Window) {
    match *window.mode {
        Mode::Preview => {
            let playing = window.machine.preview.is_playing();
            window.machine.preview.set_playing(!playing);
        }
        Mode::Prepare => {
            let running = window.view.section.playing;
            window.view.section.playing = !running && height_range(window.doc).is_some();
        }
    }
}

/// Where the cut goes next frame of play, or `None` past the top, where play stops. A cut
/// parked at the top starts again from the bottom.
pub(super) fn run_up(
    height_mm: Option<Scalar>,
    (bottom, top): (Scalar, Scalar),
    step_mm: Scalar,
) -> Option<Scalar> {
    let from = height_mm.unwrap_or(bottom - step_mm);
    let next = from + step_mm;
    (next < top).then(|| next.max(bottom))
}

/// Runs the cut up the model while the prepare mode is playing. Returns whether the
/// window has to keep repainting, the way `animate` does for the stack.
pub fn animate_section(
    ctx: &egui::Context,
    section: &mut Section,
    doc: &Doc,
    layer_height_mm: Scalar,
) -> bool {
    if !section.playing {
        return false;
    }
    let Some(range) = height_range(doc) else {
        section.playing = false;
        return false;
    };
    let step_mm = ctx.input(|input| input.stable_dt) * PLAY_LAYERS_PER_S * layer_height_mm;
    section.height_mm = run_up(section.height_mm, range, step_mm);
    section.playing = section.height_mm.is_some();
    true
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

/// Where a height sits in its range, from zero at the bottom to one at the top.
pub(super) fn fraction(bottom: Scalar, top: Scalar, height: Scalar) -> f32 {
    if top <= bottom {
        return 1.0;
    }
    ((height - bottom) / (top - bottom)).clamp(0.0, 1.0)
}
