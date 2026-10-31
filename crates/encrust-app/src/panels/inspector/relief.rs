mod picture;

use picture::picker;

use core_volume::Tiling;

use crate::i18n::tr;
use crate::panels::Window;
use crate::relief::ReliefMode;
use crate::state::Doc;
use crate::ui::{
    blocked_hover, busy, describe, icon, icon_toggle, meta, number_row, primary_button, readings,
    secondary_button, section, switch, theme, toggle_button, tone,
};

use super::paint::{COARSE_STEP, MAX_BRUSH_MM, MIN_BRUSH_MM};

/// Bounds on one tile of a picture, millimetres: under a millimetre it is finer than the
/// lattice can carry, and past two hundred one tile covers the plate.
const MIN_TILE_MM: f32 = 1.0;
const MAX_TILE_MM: f32 = 200.0;
const TILE_STEP: f64 = 0.1;

/// Per point of drag: a relief is measured in tenths of a millimetre, precision in whole
/// percent.
const DEPTH_STEP: f64 = 0.05;

/// Past two millimetres a relief is a shape of its own rather than a texture, and it
/// would cost a band that wide on every voxel of the field.
const MAX_DEPTH_MM: f32 = 2.0;

/// Past twenty pixels of blur a picture read down to 1024 pixels has lost its pattern.
const MAX_SMOOTHING_PX: f32 = 20.0;
const SMOOTHING_STEP: f64 = 0.1;

/// The tool's three jobs behind one switch at its head — a texture pressed in, text
/// written on, models combined — each with its own sections and its own action.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    match window.tools.relief.mode {
        ReliefMode::Texture => texture(ui, window),
        ReliefMode::Text => super::text::sections(ui, window),
        ReliefMode::Combine => super::combine::sections(ui, window),
    }
}

/// The action at the foot belongs to the job in hand, never to all three.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    match window.tools.relief.mode {
        ReliefMode::Texture => press(ui, window),
        ReliefMode::Text => super::text::action(ui, window),
        ReliefMode::Combine => super::combine::action(ui, window),
    }
}

/// Texture, Text or Combine, as icons in the card's heading, laid right to left.
pub fn modes(ui: &mut egui::Ui, window: &mut Window) {
    let mode = &mut window.tools.relief.mode;
    for (value, glyph, name) in [
        (
            ReliefMode::Combine,
            icon::COMBINE,
            tr!("relief-mode-combine"),
        ),
        (ReliefMode::Text, icon::TEXT, tr!("relief-mode-text")),
        (
            ReliefMode::Texture,
            icon::TEXTURE,
            tr!("relief-mode-texture"),
        ),
    ] {
        if icon_toggle(ui, glyph, &name, *mode == value).clicked() {
            *mode = value;
        }
    }
}

/// What the mode in hand does, read from the question mark beside the card's name.
pub fn mode_note(window: &Window) -> String {
    match window.tools.relief.mode {
        ReliefMode::Texture => tr!("relief-mode-texture-note"),
        ReliefMode::Text => tr!("relief-mode-text-note"),
        ReliefMode::Combine => tr!("relief-mode-combine-note"),
    }
}

/// What is pressed into the models on the plate — a chosen picture as tiles, or the
/// texture each came with — and how deep.
fn texture(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, &tr!("relief-paint"), None, |ui| painting(ui, window));
    section(ui, &tr!("relief-image"), None, |ui| {
        if window.tools.relief.picture.is_none() {
            describe(ui, &tr!("relief-image-note"));
        }
        picker(ui, window);
        reading(ui, window);
    });
    section(ui, &tr!("relief-depth"), None, |ui| {
        depth(ui, window);
        if window.tools.relief.picture.is_some() {
            tiling(ui, &mut window.tools.relief.tiling);
        }
    });
    section(ui, &tr!("relief-hold"), None, |ui| holding(ui, window));
}

/// The brush that paints where the texture is pressed, and the way to take it all off.
fn painting(ui: &mut egui::Ui, window: &mut Window) {
    let brushing = window.tools.relief.brushing;
    if toggle_button(ui, icon::PAINT, &tr!("relief-paint-brush"), brushing).clicked() {
        window.tools.relief.brushing = !brushing;
    }
    if window.tools.relief.brushing {
        number_row(
            ui,
            &tr!("paint-brush"),
            &mut window.tools.relief.state.brush_radius_mm,
            &tr!("unit-mm"),
            COARSE_STEP,
            MIN_BRUSH_MM..=MAX_BRUSH_MM,
            2,
        );
    }
    describe(ui, &tr!("relief-paint-note"));
    let painted = window
        .doc
        .scene
        .targets()
        .any(|object| !object.relief_painted.is_empty());
    if painted && secondary_button(ui, icon::REMOVE, &tr!("relief-paint-clear")).clicked() {
        for object in window.doc.scene.targets_mut() {
            object.relief_painted.clear();
        }
    }
}

/// How the grey of the texture is read before it is pressed.
fn reading(ui: &mut egui::Ui, window: &mut Window) {
    let state = &mut window.tools.relief.state;
    switch(ui, &mut state.invert, &tr!("relief-invert"));
    number_row(
        ui,
        &tr!("relief-smoothing"),
        &mut state.smoothing_px,
        &tr!("unit-px"),
        SMOOTHING_STEP,
        0.0..=MAX_SMOOTHING_PX,
        1,
    );
}

/// The faces turned to the plate or straight up that the relief leaves as they are.
fn holding(ui: &mut egui::Ui, window: &mut Window) {
    let state = &mut window.tools.relief.state;
    for (label, angle) in [
        (tr!("relief-hold-bottom"), &mut state.bottom_deg),
        (tr!("relief-hold-top"), &mut state.top_deg),
    ] {
        number_row(ui, &label, angle, &tr!("unit-deg"), 1.0, 0.0..=90.0, 0);
    }
    describe(ui, &tr!("relief-hold-note"));
}

/// How big a tile of the picture is, and how far it is turned.
fn tiling(ui: &mut egui::Ui, tiling: &mut Tiling) {
    number_row(
        ui,
        &tr!("relief-tile-size"),
        &mut tiling.tile_mm,
        &tr!("unit-mm"),
        TILE_STEP,
        MIN_TILE_MM..=MAX_TILE_MM,
        1,
    );
    number_row(
        ui,
        &tr!("relief-angle"),
        &mut tiling.angle_deg,
        &tr!("unit-deg"),
        1.0,
        0.0..=360.0,
        0,
    );
    describe(ui, &tr!("relief-tiling-note"));
}

fn depth(ui: &mut egui::Ui, window: &mut Window) {
    number_row(
        ui,
        &tr!("relief-depth"),
        &mut window.tools.relief.state.amplitude_mm,
        &tr!("unit-mm"),
        DEPTH_STEP,
        -MAX_DEPTH_MM..=MAX_DEPTH_MM,
        2,
    );
    switch(
        ui,
        &mut window.tools.relief.state.symmetric,
        &tr!("relief-symmetric"),
    );
    let precision = &mut window.tools.relief.state.precision;
    let mut percent = *precision * 100.0;
    if number_row(
        ui,
        &tr!("relief-precision"),
        &mut percent,
        "%",
        1.0,
        0.0..=100.0,
        1,
    ) {
        *precision = percent / 100.0;
    }

    let lattice_mm = window.tools.relief.settings().voxel_mm(
        window
            .doc
            .scene
            .targets()
            .map(|o| o.mesh.surface_area())
            .fold(0.0_f32, f32::max),
    );
    describe(
        ui,
        &tr!("relief-lattice-note", lattice = format!("{lattice_mm:.2}")),
    );
}

/// Which of the models on the plate still carry a texture to press.
fn carried(ui: &mut egui::Ui, doc: &Doc) {
    let mut mapped: Vec<&str> = doc
        .scene
        .targets()
        .filter_map(|object| object.mapped.as_ref())
        .flat_map(|mapped| mapped.names.iter().map(String::as_str))
        .collect();
    mapped.sort_unstable();
    mapped.dedup();

    let [first, rest @ ..] = mapped.as_slice() else {
        tone(ui, &tr!("relief-no-texture"), theme::colors().text_low);
        return;
    };

    let label = tr!("relief-textures", count = rest.len() + 1);
    readings(ui, &[(label, truncated(first))]);
    // The rest are a list rather than a second cell: a file name is long enough that two
    // to a row would be two ellipses.
    meta(
        ui,
        &rest.iter().map(|name| truncated(name)).collect::<Vec<_>>(),
    );
}

/// How wide a file name may be before its middle is cut out. A panel is narrow, and the
/// end of an image's name is what tells two of them apart.
const NAME_CHARS: usize = 22;

fn truncated(name: &str) -> String {
    let count = name.chars().count();
    if count <= NAME_CHARS {
        return name.to_owned();
    }
    let keep = NAME_CHARS / 2 - 1;
    let head: String = name.chars().take(keep).collect();
    let tail: String = name.chars().skip(count - keep).collect();
    format!("{head}…{tail}")
}

/// Pressing the texture in, or the progress of the run that is going.
fn press(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.relief.is_busy() {
        let step = window
            .tools
            .relief
            .counted()
            .map(|counted| (counted.label(), counted.share));
        let step = step.as_ref().map(|(label, share)| (label.as_str(), *share));
        if busy(ui, &tr!("relief-pressing"), step, true) {
            window.tools.relief.cancel(&mut window.machine.status);
        }
        return;
    }

    let blocked = window.tools.relief.blocker(&window.doc.scene);
    let button = primary_button(ui, &tr!("relief-press"), blocked.is_none());
    if blocked_hover(button, blocked).clicked() {
        let started = window.tools.relief.start(&window.doc.scene);
        window
            .machine
            .status
            .report(&tr!("relief-pressing-status"), started);
    }
}
