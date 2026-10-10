use std::sync::{Arc, OnceLock};

use core_geometry::Vec3;
use egui::ColorImage;

use crate::plate::BuildPlate;
use crate::render::vertex::LabelVertex;
use crate::ui::theme;

/// The word lying in front of the plate.
const WORD: &str = "Front";

/// The depth of the band in front of the plate the word is centred in, millimetres.
const BAND_MM: f32 = 12.0;

/// Point size the word is laid out at. Only the resolution of the atlas glyphs depends on
/// it; the word is scaled to the band afterwards.
const LAYOUT_PT: f32 = 48.0;

/// How much of the band's depth the word's line height fills.
const FILL: f32 = 0.7;

/// How far the word stands over the plate, millimetres: enough that no rounding puts it
/// level with the plate's own lines.
const RELIEF_MM: f32 = 0.02;

/// The word as the font laid it out, in line heights about its own middle, and the slice
/// of the font atlas it samples.
struct Word {
    /// Corner position and texture coordinate, three to a triangle.
    corners: Vec<([f32; 2], [f32; 2])>,
    atlas: Arc<ColorImage>,
}

static LAID_OUT: OnceLock<Word> = OnceLock::new();

/// Lays the word out and snapshots the font atlas behind it, once for the life of the
/// window. Taking the atlas as it stands is safe because egui only ever appends to it, so
/// the glyphs this snapshot holds keep the coordinates the layout gave them.
pub fn prime(painter: &egui::Painter) {
    LAID_OUT.get_or_init(|| lay_out(painter));
}

fn lay_out(painter: &egui::Painter) -> Word {
    let font = egui::FontId::new(LAYOUT_PT, egui::FontFamily::Name(theme::MEDIUM.into()));
    let galley = painter.layout_no_wrap(WORD.to_owned(), font, egui::Color32::PLACEHOLDER);
    let (size, atlas) = painter
        .ctx()
        .fonts(|fonts| (fonts.font_image_size(), Arc::new(fonts.image())));
    let [width_px, height_px] = size;

    // Points to line heights, about the middle of the word: what is left depends on the
    // band it is put in, not on the size it happened to be laid out at.
    let scale = 1.0 / galley.rect.height().max(f32::EPSILON);
    let middle = galley.rect.center();

    let mut corners = Vec::new();
    for placed in &galley.rows {
        let mesh = &placed.row.visuals.mesh;
        for index in &mesh.indices {
            let Some(vertex) = mesh.vertices.get(*index as usize) else {
                continue;
            };
            let at = placed.pos + vertex.pos.to_vec2() - middle.to_vec2();
            corners.push((
                [at.x * scale, at.y * scale],
                [
                    vertex.uv.x / width_px as f32,
                    vertex.uv.y / height_px as f32,
                ],
            ));
        }
    }
    Word { corners, atlas }
}

/// The word lying in front of `plate`, and the atlas its triangles sample.
///
/// Geometry rather than an overlay: it lies on the plate's plane, so it keeps the plate's
/// perspective and stays put when the camera moves.
pub fn front(plate: &BuildPlate) -> (Vec<LabelVertex>, Option<Arc<ColorImage>>) {
    let Some(word) = LAID_OUT.get() else {
        return (Vec::new(), None);
    };
    let colour = theme::scene().label;
    let scale = BAND_MM * FILL;
    let middle = Vec3::new(plate.x_mm / 2.0, -BAND_MM / 2.0, RELIEF_MM);

    let vertices = word
        .corners
        .iter()
        .map(|([x, y], uv)| {
            // The font lays out downwards and the plate measures away from the viewer, so
            // the word's own `y` runs against the plate's.
            let at = middle + Vec3::new(x * scale, -y * scale, 0.0);
            LabelVertex::new(at, *uv, colour)
        })
        .collect();
    (vertices, Some(Arc::clone(&word.atlas)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_drawn_before_the_word_has_been_laid_out() {
        // The atlas needs a context, which a headless test has none of; what matters is
        // that the frame asking for the word first gets an empty one rather than a panic.
        if LAID_OUT.get().is_none() {
            let (vertices, atlas) = front(&BuildPlate::default());
            assert!(vertices.is_empty() && atlas.is_none());
        }
    }
}
