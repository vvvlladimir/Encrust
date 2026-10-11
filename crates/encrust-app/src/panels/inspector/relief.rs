use std::sync::Arc;

use core_geometry::Vec2;
use core_volume::Tiling;

use crate::panels::Window;
use crate::relief::Picture;
use crate::state::Doc;
use crate::ui::{
    describe, hint, icon, meta, number_row, primary_button, progress_bar, secondary_button,
    section, stats, theme, tone,
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

/// What is pressed into the models on the plate — a chosen picture as tiles, or the
/// texture each came with — and how deep.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, "Image", None, |ui| {
        match &window.tools.relief.picture {
            Some(picture) => chosen(ui, picture),
            None => {
                describe(
                    ui,
                    "A model's own texture becomes geometry: white moves the surface out, \
                     black leaves it where it is. Or choose a picture to tile over any model.",
                );
                carried(ui, window.doc);
            }
        }
        picture_buttons(ui, window);
    });
    section(ui, "Depth", None, |ui| {
        depth(ui, window);
        if window.tools.relief.picture.is_some() {
            tiling(ui, &mut window.tools.relief.tiling);
        }
    });
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
            tone(ui, &format!("{width} \u{d7} {height} px"), colors.text_low);
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

/// Choosing a picture, and going back to the models' own textures.
fn picture_buttons(ui: &mut egui::Ui, window: &mut Window) {
    let relief = &mut window.tools.relief;
    let label = if relief.picture.is_some() {
        "Choose another"
    } else {
        "Choose a picture"
    };
    if secondary_button(ui, icon::OPEN, label).clicked() {
        crate::relief::open_picture(relief, &mut window.machine.status);
    }
    if relief.picture.is_some()
        && secondary_button(ui, icon::RESET, "Use the models' own textures").clicked()
    {
        relief.picture = None;
    }
}

/// How big a tile of the picture is, and how far it is turned.
fn tiling(ui: &mut egui::Ui, tiling: &mut Tiling) {
    number_row(
        ui,
        "Tile size",
        &mut tiling.tile_mm,
        "mm",
        TILE_STEP,
        MIN_TILE_MM..=MAX_TILE_MM,
        1,
    );
    number_row(
        ui,
        "Angle",
        &mut tiling.angle_deg,
        "\u{b0}",
        1.0,
        0.0..=360.0,
        0,
    );
    describe(
        ui,
        "The picture is laid from the three sides the model faces, and each face reads it \
         from the side it faces most.",
    );
}

fn depth(ui: &mut egui::Ui, window: &mut Window) {
    number_row(
        ui,
        "Depth",
        &mut window.tools.relief.state.amplitude_mm,
        "mm",
        DEPTH_STEP,
        -MAX_DEPTH_MM..=MAX_DEPTH_MM,
        2,
    );
    number_row(
        ui,
        "Precision",
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
        &format!(
            "The relief is cut on a {lattice_mm:.2} mm lattice, and the whole model is \
             meshed again at it. A negative depth sinks the texture in.",
        ),
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
        tone(
            ui,
            "No texture on the plate. It is read from the file a model is opened from, \
             and is gone once it has been pressed in.",
            theme::colors().text_low,
        );
        return;
    };

    let label = match rest.len() {
        0 => "Texture".to_owned(),
        more => format!("{} textures", more + 1),
    };
    stats(ui, &[(label.as_str(), truncated(first))]);
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
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.relief.is_busy() {
        tone(ui, "Pressing...", theme::colors().text_mid);
        progress_bar(ui, None);
        return;
    }

    let blocked = window.tools.relief.blocker(&window.doc.scene);
    if primary_button(ui, icon::RELIEF, "Press it in", blocked.is_none()).clicked() {
        let started = window.tools.relief.start(&window.doc.scene);
        window.machine.status.report("Pressing the relief", started);
    }

    let what = match &window.tools.relief.picture {
        Some(picture) => picture.name.as_str(),
        None => "the texture",
    };
    let scope = format!(
        "Presses {what} into {}. The model is replaced, so its supports and its cavity go \
         with it.",
        window.doc.scene.scope()
    );
    hint(ui, blocked.unwrap_or(&scope));
}
