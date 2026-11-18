mod occlusion;
mod raster;
mod shade;
#[cfg(test)]
mod tests;

use std::borrow::Cow;

use core_geometry::{Mesh, SmoothNormals, Transform, Vec3, smooth_normals, transform_mesh};

use crate::image::{MARGIN, Thumbnail};
use crate::marker::{self, Marker, MarkerSpot};
use crate::settings::{Rgb, ThumbnailSettings};
use crate::view::{Basis, View};
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

/// A picture of the plate from one view, and where each marker asked for landed in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub image: Thumbnail,
    /// One per marker, in the order given; none in an image of no pixels.
    pub markers: Vec<MarkerSpot>,
}

/// Degrees two faces may turn apart and still be shaded as one surface: a ball's facets
/// blend, a box keeps its edges.
const CREASE_DEG: f32 = 40.0;

/// Renders every part into one shaded image framed to the parts, not the build volume, as a
/// thumbnail is a centimetre wide; see `docs/design/thumbnail.md`.
pub fn render(parts: &[Part<'_>], settings: &ThumbnailSettings) -> Thumbnail {
    render_view(parts, View::Iso, &[], settings).image
}

/// Renders every part from `view` with `markers` drawn over it, framed so the parts fit: a
/// marker off the picture is pinned to its edge. How each is drawn: `docs/design/thumbnail.md`.
pub fn render_view(
    parts: &[Part<'_>],
    view: View,
    markers: &[Marker],
    settings: &ThumbnailSettings,
) -> Rendered {
    let basis = view.basis();
    let viewed: Vec<Viewed<'_>> = parts.iter().map(|part| Viewed::of(part, &basis)).collect();
    let marked: Vec<Vec3> = markers.iter().map(|marker| basis.of(marker.at)).collect();
    let (width_px, height_px) = (settings.width_px, settings.height_px);
    let framed = Frame::around(
        viewed.iter().map(|part| part.vertices.as_slice()),
        width_px,
        height_px,
    )
    .or_else(|| Frame::around(std::iter::once(marked.as_slice()), width_px, height_px));
    let Some(frame) = framed else {
        return Rendered {
            image: empty(settings),
            markers: Vec::new(),
        };
    };

    let mut surfaces = Surfaces::new(width_px, height_px);
    for part in &viewed {
        for face in 0..part.mesh.faces.len() {
            if let Some(corners) = part.corners(face, &basis, &frame) {
                surfaces.fill(&corners, part.look);
            }
        }
    }
    let mut image = shade::image(&surfaces, 1.0 / frame.scale, frame.span_mm, settings)
        .unwrap_or_else(|| empty(settings));
    let spots = mark(
        image.pixels_mut(),
        &surfaces.depths,
        &frame,
        &marked,
        markers,
    );
    Rendered {
        image,
        markers: spots,
    }
}

/// Draws every marker over the shaded picture and says where each landed. `marked` holds their
/// points in view coordinates, in the order of `markers`.
fn mark(
    colours: &mut [Rgb],
    depths: &[f32],
    frame: &Frame,
    marked: &[Vec3],
    markers: &[Marker],
) -> Vec<MarkerSpot> {
    let size = (frame.width as u32, frame.height as u32);
    let radius = marker::radius_px(size.0, size.1);
    let spots: Vec<MarkerSpot> = marked
        .iter()
        .map(|point| {
            let screen = frame.to_screen(*point);
            let off = |at: f32, across: f32| !(0.0..across).contains(&at);
            if off(screen.x, frame.width) || off(screen.y, frame.height) {
                let pin = |at: f32, across: f32| at.clamp(radius, (across - radius).max(radius));
                return MarkerSpot {
                    x_px: pin(screen.x, frame.width),
                    y_px: pin(screen.y, frame.height),
                    hidden: false,
                    pinned: true,
                };
            }
            MarkerSpot {
                x_px: screen.x,
                y_px: screen.y,
                hidden: marker::hidden(depths, size, screen, frame.scale),
                pinned: false,
            }
        })
        .collect();
    // The hidden ones first, so one behind the model never covers one in plain sight.
    let mut order: Vec<usize> = (0..spots.len()).collect();
    order.sort_by_key(|&index| !spots[index].hidden);
    for index in order {
        marker::draw(colours, size, spots[index], markers[index].colour);
    }
    spots
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

/// One part placed on the plate, its vertices in view coordinates and its smoothed normals.
struct Viewed<'m> {
    look: Look,
    mesh: Cow<'m, Mesh>,
    vertices: Vec<Vec3>,
    normals: SmoothNormals,
}

impl<'m> Viewed<'m> {
    fn of(part: &Part<'m>, view: &Basis) -> Self {
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
    fn corners(&self, face: usize, view: &Basis, frame: &Frame) -> Option<[Corner; 3]> {
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
            (span > f32::EPSILON).then(|| available * (1.0 - 2.0 * MARGIN as f32) / span)
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
