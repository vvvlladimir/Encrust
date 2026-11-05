mod occlusion;
mod raster;
mod shade;
#[cfg(test)]
mod tests;

use std::borrow::Cow;

use core_geometry::{Mesh, SmoothNormals, Transform, Vec3, smooth_normals, transform_mesh};

use crate::image::Thumbnail;
use crate::settings::ThumbnailSettings;
use raster::{Corner, Surfaces};

/// What a part is, which picks its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    #[default]
    Model,
    Support,
}

/// One mesh on the plate together with where it stands.
pub struct Part<'m> {
    pub mesh: &'m Mesh,
    pub transform: Transform,
    pub look: Look,
}

impl<'m> Part<'m> {
    pub fn new(mesh: &'m Mesh, transform: Transform) -> Self {
        Self {
            mesh,
            transform,
            look: Look::Model,
        }
    }

    /// A mesh whose vertices are already in plate coordinates.
    pub fn placed(mesh: &'m Mesh) -> Self {
        Self::new(mesh, Transform::default())
    }

    /// Supports, which stand in plate coordinates.
    pub fn support(mesh: &'m Mesh) -> Self {
        Self {
            look: Look::Support,
            ..Self::placed(mesh)
        }
    }
}

/// Where the plate is looked at from, in plate coordinates with Z up. The front right corner, a
/// little above the part: it shows a silhouette, a top and a side.
const EYE: Vec3 = Vec3::new(1.0, -1.0, 0.75);

/// Degrees two faces may turn apart and still be shaded as one surface: a ball's facets
/// blend, a box keeps its edges.
const CREASE_DEG: f32 = 40.0;

/// Fraction of the image left empty around the part on each side.
const MARGIN: f32 = 0.06;

/// Renders every part into one shaded image framed to the parts, not the build volume, as a
/// thumbnail is a centimetre wide; see `docs/design/thumbnail.md`.
pub fn render(parts: &[Part<'_>], settings: &ThumbnailSettings) -> Thumbnail {
    let view = View::new();
    let viewed: Vec<Viewed<'_>> = parts.iter().map(|part| Viewed::of(part, &view)).collect();
    let Some(frame) = Frame::around(
        viewed.iter().map(|part| part.vertices.as_slice()),
        settings.width_px,
        settings.height_px,
    ) else {
        return empty(settings);
    };

    let mut surfaces = Surfaces::new(settings.width_px, settings.height_px);
    for part in &viewed {
        for face in 0..part.mesh.faces.len() {
            if let Some(corners) = part.corners(face, &view, &frame) {
                surfaces.fill(&corners, part.look);
            }
        }
    }
    shade::image(&surfaces, 1.0 / frame.scale, frame.span_mm, settings)
        .unwrap_or_else(|| empty(settings))
}

/// A render nothing landed on, so a record shows its own background alone.
fn empty(settings: &ThumbnailSettings) -> Thumbnail {
    let count = (settings.width_px as usize) * (settings.height_px as usize);
    Thumbnail::rendered(
        settings.width_px,
        settings.height_px,
        vec![settings.background; count],
        vec![false; count],
    )
    .unwrap_or_else(|| {
        Thumbnail::filled(settings.width_px, settings.height_px, settings.background)
    })
}

/// The view's axes in plate coordinates: right, up, and forward away from the eye.
struct View {
    right: Vec3,
    up: Vec3,
    forward: Vec3,
}

impl View {
    fn new() -> Self {
        let forward = -EYE.normalize();
        let right = forward.cross(Vec3::Z).normalize();
        let up = right.cross(forward);
        Self { right, up, forward }
    }

    /// A plate point or direction in view coordinates; the third is depth.
    fn of(&self, vector: Vec3) -> Vec3 {
        Vec3::new(
            vector.dot(self.right),
            vector.dot(self.up),
            vector.dot(self.forward),
        )
    }
}

/// One part placed on the plate, its vertices in view coordinates and its smoothed normals.
struct Viewed<'m> {
    look: Look,
    mesh: Cow<'m, Mesh>,
    vertices: Vec<Vec3>,
    normals: SmoothNormals,
}

impl<'m> Viewed<'m> {
    fn of(part: &Part<'m>, view: &View) -> Self {
        let mesh = if part.transform == Transform::default() {
            Cow::Borrowed(part.mesh)
        } else {
            Cow::Owned(transform_mesh(part.mesh, part.transform))
        };
        let vertices = mesh
            .vertices
            .iter()
            .map(|vertex| view.of(*vertex))
            .collect();
        let normals = smooth_normals(&mesh, CREASE_DEG);
        Self {
            look: part.look,
            mesh,
            vertices,
            normals,
        }
    }

    /// Face `face` on screen, each corner with its normal in view coordinates; a corner of a
    /// face the smoothing skipped takes the face's own.
    fn corners(&self, face: usize, view: &View, frame: &Frame) -> Option<[Corner; 3]> {
        let indices = self.mesh.faces.get(face)?;
        let points = [
            *self.vertices.get(indices[0] as usize)?,
            *self.vertices.get(indices[1] as usize)?,
            *self.vertices.get(indices[2] as usize)?,
        ];
        let flat = (points[1] - points[0])
            .cross(points[2] - points[0])
            .normalize_or_zero();
        let corners = std::array::from_fn(|slot| {
            let normal = self
                .normals
                .corners
                .get(face * 3 + slot)
                .and_then(|&index| self.normals.normals.get(index as usize))
                .map_or(flat, |normal| view.of(*normal));
            Corner {
                screen: frame.to_screen(points[slot]),
                normal,
            }
        });
        Some(corners)
    }
}

/// How view coordinates land on pixels: the part scaled to fill the image, centred.
struct Frame {
    centre: Vec3,
    /// Pixels per millimetre.
    scale: f32,
    /// The parts' largest extent along any view axis, depth included.
    span_mm: f32,
    width: f32,
    height: f32,
}

impl Frame {
    fn around<'v>(
        viewed: impl Iterator<Item = &'v [Vec3]>,
        width_px: u32,
        height_px: u32,
    ) -> Option<Self> {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in viewed.flatten() {
            min = min.min(*vertex);
            max = max.max(*vertex);
        }
        if width_px == 0 || height_px == 0 || min.x > max.x {
            return None;
        }

        let (width, height) = (width_px as f32, height_px as f32);
        let span = max - min;
        // A part seen exactly edge on spans nothing in one direction; the other still
        // decides the scale, and a part that spans nothing at all is drawn as a point.
        let fit = |available: f32, span: f32| {
            (span > f32::EPSILON).then(|| available * (1.0 - 2.0 * MARGIN) / span)
        };
        let scale = match (fit(width, span.x), fit(height, span.y)) {
            (Some(x), Some(y)) => x.min(y),
            (Some(x), None) => x,
            (None, Some(y)) => y,
            (None, None) => 1.0,
        };

        Some(Self {
            centre: (min + max) * 0.5,
            scale,
            span_mm: span.max_element(),
            width,
            height,
        })
    }

    /// Pixel coordinates and depth of a view-space point. Y grows downwards in an image.
    fn to_screen(&self, point: Vec3) -> Vec3 {
        Vec3::new(
            self.width * 0.5 + (point.x - self.centre.x) * self.scale,
            self.height * 0.5 - (point.y - self.centre.y) * self.scale,
            point.z,
        )
    }
}
