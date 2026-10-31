//! The picture a recent file is shown with on the start page: the plate as it stood when the
//! file was opened or saved, rendered once off the drawing thread and kept as a PNG in a
//! folder beside `preferences.json`; see `docs/decisions/0234`. A project carries the
//! same picture inside it (ADR 0261). It is kept as shade and coverage, tinted when shown (0385).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use core_geometry::{Mesh, Transform};
use core_thumbnail::{Part, Rgb, ThumbnailSettings, render};

use crate::recent::Recent;
use crate::scene::Scene;
use crate::ui::theme;

/// The picture's size in pixels: the start page tile's 3:2, sharp on a 2x screen.
pub const SIZE_PX: [usize; 2] = [480, 320];

/// Rendered at twice the size it is kept at and halved, which smooths its edges.
const SUPERSAMPLE: usize = 2;

/// Rendered white on black: a lit face is never darker than the renderer's ambient share, so
/// a black sample is one no face covers.
const RENDERED: (Rgb, Rgb) = ([0; 3], [255; 3]);

/// What a picture is drawn from: each mesh and where it stands.
pub type Parts = Vec<(Arc<Mesh>, Transform)>;

/// The visible models on the plate shown, the ones a project's picture is drawn from.
pub fn of_plate(scene: &Scene) -> Parts {
    scene
        .here()
        .filter(|object| object.visible)
        .map(|object| (Arc::clone(&object.mesh), object.transform))
        .collect()
}

/// Renders `parts` for `path` on a thread of its own, then drops the pictures of files
/// `recent` no longer lists. A failure is silent: the tile falls back to its glyph.
pub fn keep(recent: &Recent, path: &Path, parts: Parts) {
    if parts.is_empty() {
        return;
    }
    keep_with(recent, path, move || encoded(&parts));
}

/// Keeps `png`, a picture already drawn, for `path`, as [`keep`] does.
pub fn keep_png(recent: &Recent, path: &Path, png: Vec<u8>) {
    keep_with(recent, path, move || Ok(png));
}

fn keep_with(
    recent: &Recent,
    path: &Path,
    png: impl FnOnce() -> image::ImageResult<Vec<u8>> + Send + 'static,
) {
    let Some(dir) = dir() else {
        return;
    };
    let target = dir.join(file_name(path));
    let listed: Vec<PathBuf> = recent
        .files()
        .iter()
        .map(|file| dir.join(file_name(&file.path)))
        .collect();
    crate::job::spawn(move || {
        if png().and_then(|png| write(&target, &png)).is_ok() {
            prune(&dir, &listed);
        }
    });
}

/// `parts` as the PNG a picture is kept as, or `None` for a plate with nothing on it.
pub fn png(parts: &Parts) -> image::ImageResult<Option<Vec<u8>>> {
    if parts.is_empty() {
        return Ok(None);
    }
    encoded(parts).map(Some)
}

/// `parts` rendered in this frame's colours at the size a picture is kept at.
pub fn rendered(parts: &Parts) -> egui::ColorImage {
    tinted(SIZE_PX, &shades(parts), colors())
}

/// The tile's background and the shade a lit face is drawn towards.
fn colors() -> (Rgb, Rgb) {
    let rgb = |color: egui::Color32| [color.r(), color.g(), color.b()];
    (rgb(theme::colors().sunken), rgb(theme::scene().object))
}

/// The picture kept for `path` in this frame's colours, or `None` when there is none to
/// show. One kept before pictures were tinted has no alpha and is shown as it was drawn.
pub fn load(path: &Path) -> Option<egui::ColorImage> {
    let bytes = std::fs::read(dir()?.join(file_name(path))).ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    let size = [image.width() as usize, image.height() as usize];
    if !image.color().has_alpha() {
        return Some(egui::ColorImage::from_rgb(size, image.to_rgb8().as_raw()));
    }
    Some(tinted(size, image.to_luma_alpha8().as_raw(), colors()))
}

/// Shade and coverage bytes `size` large drawn as `(background, model)` would draw them:
/// each pixel the background blended toward the model shaded by its level.
fn tinted(size: [usize; 2], shades: &[u8], (background, model): (Rgb, Rgb)) -> egui::ColorImage {
    let mut bytes = Vec::with_capacity(shades.len() / 2 * 3);
    for [level, cover] in shades.as_chunks::<2>().0 {
        let (level, cover) = (u32::from(*level), u32::from(*cover));
        bytes.extend(background.iter().zip(model).map(|(back, front)| {
            let lit = u32::from(front) * level / 255;
            ((u32::from(*back) * (255 - cover) + lit * cover + 127) / 255) as u8
        }));
    }
    egui::ColorImage::from_rgb(size, &bytes)
}

/// A browser keeps no paths, so it keeps no pictures either.
#[cfg(not(target_arch = "wasm32"))]
fn dir() -> Option<PathBuf> {
    Some(printer_profiles::user_dir()?.with_file_name("recent"))
}

#[cfg(target_arch = "wasm32")]
fn dir() -> Option<PathBuf> {
    None
}

/// The FNV-1a hash of the path, which names one file stably across runs and builds.
fn file_name(path: &Path) -> String {
    let hash = path
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("{hash:016x}.png")
}

/// `parts` rendered as a grey and alpha PNG `SIZE_PX` large.
fn encoded(parts: &Parts) -> image::ImageResult<Vec<u8>> {
    let mut png = Vec::new();
    image::write_buffer_with_format(
        &mut std::io::Cursor::new(&mut png),
        &shades(parts),
        SIZE_PX[0] as u32,
        SIZE_PX[1] as u32,
        image::ExtendedColorType::La8,
        image::ImageFormat::Png,
    )?;
    Ok(png)
}

/// Written beside its place and renamed into it, so a reader never sees half a file.
fn write(target: &Path, png: &[u8]) -> image::ImageResult<()> {
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let partial = target.with_extension("part");
    std::fs::write(&partial, png)?;
    std::fs::rename(partial, target)?;
    Ok(())
}

/// `parts` rendered large and halved, as shade and coverage bytes `SIZE_PX` large.
fn shades(parts: &Parts) -> Vec<u8> {
    let (background, model) = RENDERED;
    let placed: Vec<Part<'_>> = parts
        .iter()
        .map(|(mesh, transform)| Part::new(mesh, *transform))
        .collect();
    let settings = ThumbnailSettings {
        width_px: (SIZE_PX[0] * SUPERSAMPLE) as u32,
        height_px: (SIZE_PX[1] * SUPERSAMPLE) as u32,
        background,
        model,
    };
    let large = render(&placed, &settings);
    halved(large.pixels(), SIZE_PX)
}

/// Removes every picture in `dir` that is not one of `listed`.
fn prune(dir: &Path, listed: &[PathBuf]) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "png") && !listed.contains(&path) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Each block of `SUPERSAMPLE` pixels square of a white on black render halved into one: the
/// mean shade of the samples a face covers, and the share of them it covers.
fn halved(pixels: &[Rgb], size: [usize; 2]) -> Vec<u8> {
    let wide = size[0] * SUPERSAMPLE;
    let count = (SUPERSAMPLE * SUPERSAMPLE) as u32;
    let mut bytes = Vec::with_capacity(size[0] * size[1] * 2);
    for y in 0..size[1] {
        for x in 0..size[0] {
            let (mut level, mut covered) = (0u32, 0u32);
            for dy in 0..SUPERSAMPLE {
                let row = (y * SUPERSAMPLE + dy) * wide + x * SUPERSAMPLE;
                for pixel in &pixels[row..row + SUPERSAMPLE] {
                    level += u32::from(pixel[0]);
                    covered += u32::from(pixel[0] > 0);
                }
            }
            bytes.push((level / covered.max(1)) as u8);
            bytes.push((covered * 255 / count) as u8);
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_names_its_picture_the_same_every_time() {
        let name = file_name(Path::new("/plates/cube.stl"));
        assert_eq!(name, file_name(Path::new("/plates/cube.stl")));
        assert_ne!(name, file_name(Path::new("/plates/cube.obj")));
    }

    #[test]
    fn a_written_picture_reads_back_at_its_size() {
        let dir = std::env::temp_dir().join(format!("encrust-recent-{}", std::process::id()));
        let target = dir.join("cube.png");
        let triangle = Mesh::new(
            vec![
                core_geometry::Vec3::new(0.0, 0.0, 0.0),
                core_geometry::Vec3::new(1.0, 0.0, 0.0),
                core_geometry::Vec3::new(0.0, 1.0, 1.0),
            ],
            vec![[0, 1, 2]],
        );
        let parts: Parts = vec![(Arc::new(triangle), Transform::default())];
        let png = encoded(&parts).expect("a picture in memory encodes");
        write(&target, &png).expect("the temporary folder is writable");
        let image = image::open(&target).expect("the picture was just written");
        assert_eq!([image.width(), image.height()], [480, 320]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn one_picture_takes_the_background_and_model_colour_it_is_shown_in() {
        let cube = core_geometry::cuboid(core_geometry::Vec3::ZERO, core_geometry::Vec3::ONE);
        let parts: Parts = vec![(Arc::new(cube), Transform::default())];
        let shades = shades(&parts);
        let middle = SIZE_PX[1] / 2 * SIZE_PX[0] + SIZE_PX[0] / 2;
        for (background, model) in [
            ([12, 14, 17], [134, 171, 187]),
            ([226, 230, 235], [47, 126, 163]),
        ] {
            let image = tinted(SIZE_PX, &shades, (background, model));
            let [r, g, b, _] = image.pixels[0].to_array();
            assert_eq!([r, g, b], background, "the corner is the tile's background");
            let [r, g, b, _] = image.pixels[middle].to_array();
            let level = f32::from(shades[middle * 2]) / 255.0;
            let lit = model.map(|channel| (f32::from(channel) * level) as u8);
            assert!(
                [r, g, b]
                    .iter()
                    .zip(lit)
                    .all(|(got, want)| got.abs_diff(want) <= 1),
                "the middle is the model at its shade {lit:?}, got {:?}",
                [r, g, b]
            );
        }
    }
}
