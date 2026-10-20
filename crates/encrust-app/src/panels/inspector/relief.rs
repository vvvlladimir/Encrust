use std::sync::Arc;

use core_geometry::Vec2;
use core_volume::Tiling;

use crate::i18n::tr;
use crate::panels::Window;
use crate::relief::{BUILTINS, Picture, ReliefMode};
use crate::state::Doc;
use crate::ui::{
    Segment, Segmented, blocked_hover, busy, describe, icon, meta, number_row, primary_button,
    secondary_button, section, stats, subheading, swatch, switch, theme, tone,
};

/// Bounds on one tile of a picture, millimetres: under a millimetre it is finer than the
/// lattice can carry, and past two hundred one tile covers the plate.
const MIN_TILE_MM: f32 = 1.0;
const MAX_TILE_MM: f32 = 200.0;
const TILE_STEP: f64 = 0.1;

/// Per point of drag: a relief is measured in tenths of a millimetre, precision in
/// hundredths of its own range.
const DEPTH_STEP: f64 = 0.05;
const FINE_STEP: f64 = 0.01;

/// Past two millimetres a relief is a shape of its own rather than a texture, and it
/// would cost a band that wide on every voxel of the field.
const MAX_DEPTH_MM: f32 = 2.0;

/// Past twenty pixels of blur a picture read down to 1024 pixels has lost its pattern.
const MAX_SMOOTHING_PX: f32 = 20.0;
const SMOOTHING_STEP: f64 = 0.1;

/// The tool's three jobs behind one switch at its head — a texture pressed in, text
/// written on, models combined — each with its own sections and its own action.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, &tr!("relief-mode"), None, |ui| modes(ui, window));
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

fn modes(ui: &mut egui::Ui, window: &mut Window) {
    let segments = [
        Segment::new(ReliefMode::Texture, tr!("relief-mode-texture")),
        Segment::new(ReliefMode::Text, tr!("relief-mode-text")),
        Segment::new(ReliefMode::Combine, tr!("relief-mode-combine")),
    ];
    let mode = &mut window.tools.relief.mode;
    Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, mode);
    describe(
        ui,
        &match mode {
            ReliefMode::Texture => tr!("relief-mode-texture-note"),
            ReliefMode::Text => tr!("relief-mode-text-note"),
            ReliefMode::Combine => tr!("relief-mode-combine-note"),
        },
    );
}

/// What is pressed into the models on the plate — a chosen picture as tiles, or the
/// texture each came with — and how deep.
fn texture(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, &tr!("relief-image"), None, |ui| {
        match &window.tools.relief.picture {
            Some(picture) => chosen(ui, picture),
            None => {
                describe(ui, &tr!("relief-image-note"));
                carried(ui, window.doc);
            }
        }
        subheading(ui, &tr!("relief-builtins"));
        gallery(ui, window);
        picture_buttons(ui, window);
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
        number_row(ui, &label, angle, &tr!("unit-degrees"), 1.0, 0.0..=90.0, 0);
    }
    describe(ui, &tr!("relief-hold-note"));
}

/// The edge of the thumbnail a chosen picture is shown at, points.
const THUMB_PT: f32 = 64.0;
/// Pixels a side of that thumbnail.
const THUMB_PX: usize = 64;

/// The picture in hand: a thumbnail, its name and its size.
fn chosen(ui: &mut egui::Ui, picture: &Picture) {
    let colors = theme::colors();
    ui.horizontal(|ui| {
        let texture = thumbnail(ui.ctx(), picture);
        ui.add(egui::Image::new(&texture).fit_to_exact_size(egui::vec2(THUMB_PT, THUMB_PT)));
        ui.vertical(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(truncated(&picture.name))
                        .font(theme::code(12.0))
                        .color(colors.text_high),
                )
                .truncate(),
            );
            let [width, height] = picture.size_px;
            tone(
                ui,
                &tr!("relief-picture-size", width = width, height = height),
                colors.text_low,
            );
        });
    });
}

/// The picture shrunk to a thumbnail, made into a texture once per picture.
fn thumbnail(ctx: &egui::Context, picture: &Picture) -> egui::TextureHandle {
    let id = egui::Id::new(("relief-thumbnail", Arc::as_ptr(&picture.heights) as usize));
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let pixels = (0..THUMB_PX * THUMB_PX)
        .map(|index| {
            let (x, y) = (index % THUMB_PX, index / THUMB_PX);
            let uv = Vec2::new(
                (x as f32 + 0.5) / THUMB_PX as f32,
                1.0 - (y as f32 + 0.5) / THUMB_PX as f32,
            );
            let grey = (picture.heights.sample(uv).clamp(0.0, 1.0) * 255.0).round() as u8;
            theme::mask_pixel(grey, false)
        })
        .collect();
    let image = egui::ColorImage::new([THUMB_PX, THUMB_PX], pixels);
    let texture = ctx.load_texture("relief-thumbnail", image, egui::TextureOptions::NEAREST);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

/// Textures in a row of the gallery, and the gap between two, points.
const GALLERY_COLUMNS: usize = 4;
const GALLERY_GAP: f32 = 6.0;

/// The textures that come with the window, a click on one taking it as the picture.
fn gallery(ui: &mut egui::Ui, window: &mut Window) {
    let picked = window
        .tools
        .relief
        .picture
        .as_ref()
        .and_then(|picture| picture.builtin);
    let side = (ui.available_width() - GALLERY_GAP * (GALLERY_COLUMNS - 1) as f32)
        / GALLERY_COLUMNS as f32;
    let mut clicked = None;
    for (row, builtins) in BUILTINS.chunks(GALLERY_COLUMNS).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GALLERY_GAP;
            for (column, builtin) in builtins.iter().enumerate() {
                let index = row * GALLERY_COLUMNS + column;
                let Some(texture) = builtin_thumbnail(ui.ctx(), index) else {
                    continue;
                };
                let name = (builtin.name)();
                if swatch(ui, &texture, side, picked == Some(index), &name).clicked() {
                    clicked = Some(index);
                }
            }
        });
        ui.add_space(GALLERY_GAP);
    }
    if let Some(index) = clicked {
        let relief = &mut window.tools.relief;
        crate::relief::choose_builtin(relief, &mut window.machine.status, index);
    }
}

/// One shipped texture shrunk to a thumbnail, made into a texture the first time it is
/// shown. `None` only for one that does not decode, which a test rules out.
fn builtin_thumbnail(ctx: &egui::Context, index: usize) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("relief-builtin", index));
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let grey = BUILTINS.get(index)?.thumbnail(THUMB_PX as u32).ok()?;
    let pixels = grey
        .into_iter()
        .map(|grey| theme::mask_pixel(grey, false))
        .collect();
    let image = egui::ColorImage::new([THUMB_PX, THUMB_PX], pixels);
    let texture = ctx.load_texture("relief-builtin", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

/// Choosing a picture, and going back to the models' own textures.
fn picture_buttons(ui: &mut egui::Ui, window: &mut Window) {
    let relief = &mut window.tools.relief;
    let label = if relief.picture.is_some() {
        tr!("relief-choose-another")
    } else {
        tr!("relief-choose-picture")
    };
    if secondary_button(ui, icon::OPEN, &label).clicked() {
        crate::relief::open_picture(relief, &mut window.machine.status);
    }
    if relief.picture.is_some()
        && secondary_button(ui, icon::RESET, &tr!("relief-own-textures")).clicked()
    {
        relief.picture = None;
    }
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
        &tr!("unit-degrees"),
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
    number_row(
        ui,
        &tr!("relief-precision"),
        &mut window.tools.relief.state.precision,
        "",
        FINE_STEP,
        0.0..=1.0,
        2,
    );

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
    stats(ui, &[(label, truncated(first))]);
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
    let button = primary_button(ui, icon::RELIEF, &tr!("relief-press"), blocked.is_none());
    if blocked_hover(button, blocked).clicked() {
        let started = window.tools.relief.start(&window.doc.scene);
        window
            .machine
            .status
            .report(&tr!("relief-pressing-status"), started);
    }
}
