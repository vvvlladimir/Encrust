use core_geometry::{Mesh, Transform, Vec3};

use crate::image::Thumbnail;
use crate::marker::{self, Marker, MarkerSpot};
use crate::settings::{Rgb, ThumbnailSettings};
use crate::view::{Basis, View};

/// One mesh on the plate together with where it stands.
pub struct Part<'m> {
    pub mesh: &'m Mesh,
    pub transform: Transform,
}

impl<'m> Part<'m> {
    pub fn new(mesh: &'m Mesh, transform: Transform) -> Self {
        Self { mesh, transform }
    }

    /// A mesh whose vertices are already in plate coordinates.
    pub fn placed(mesh: &'m Mesh) -> Self {
        Self::new(mesh, Transform::default())
    }
}

/// A picture of the plate from one view, and where each marker asked for landed in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub image: Thumbnail,
    /// One per marker, in the order given; none in an image of no pixels.
    pub markers: Vec<MarkerSpot>,
}

/// Direction the light comes from, in view coordinates: over the camera's left shoulder.
const LIGHT: Vec3 = Vec3::new(-0.3, 0.55, -1.0);

/// Share of a face's colour that does not depend on which way it points, so that a face
/// turned away from the light is still read as a surface rather than as a hole.
const AMBIENT: f32 = 0.28;

/// Fraction of the image left empty around the part on each side.
const MARGIN: f32 = 0.06;

/// Renders every part into one shaded image, framed so the whole plate fits. The view fits the
/// parts, not the build volume, as a thumbnail is a centimetre wide.
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
    let (width, height) = (settings.width_px, settings.height_px);
    let mut colours = vec![settings.background; (width as usize) * (height as usize)];
    let mut depths = vec![f32::INFINITY; colours.len()];

    let basis = view.basis();
    let viewed: Vec<Vec<Vec3>> = parts.iter().map(|part| project(part, &basis)).collect();
    let marked: Vec<Vec3> = markers.iter().map(|marker| basis.of(marker.at)).collect();
    let framed = Frame::around(viewed.iter().flatten(), width, height)
        .or_else(|| Frame::around(marked.iter(), width, height));
    let Some(frame) = framed else {
        return Rendered {
            image: Thumbnail::filled(width, height, settings.background),
            markers: Vec::new(),
        };
    };

    for (part, vertices) in parts.iter().zip(&viewed) {
        for face in &part.mesh.faces {
            let Some(corners) = corners(vertices, *face) else {
                continue;
            };
            let colour = shade(&corners, settings.model);
            let screen = corners.map(|corner| frame.to_screen(corner));
            fill(&mut colours, &mut depths, width, height, screen, colour);
        }
    }
    let spots = mark(&mut colours, &depths, &frame, &marked, markers);

    let image = Thumbnail::from_pixels(width, height, colours)
        .unwrap_or_else(|| Thumbnail::filled(width, height, settings.background));
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

/// Every vertex of one part in view coordinates: right, up and depth away from the eye.
fn project(part: &Part<'_>, basis: &Basis) -> Vec<Vec3> {
    let matrix = part.transform.to_matrix();
    part.mesh
        .vertices
        .iter()
        .map(|vertex| basis.of(matrix.transform_point3(*vertex)))
        .collect()
}

fn corners(vertices: &[Vec3], face: [u32; 3]) -> Option<[Vec3; 3]> {
    Some([
        *vertices.get(face[0] as usize)?,
        *vertices.get(face[1] as usize)?,
        *vertices.get(face[2] as usize)?,
    ])
}

/// Flat Lambert shading of one face, two-sided. The normal's sign is dropped so an unrepaired mesh
/// still shows a solid.
fn shade(corners: &[Vec3; 3], model: Rgb) -> Rgb {
    let normal = (corners[1] - corners[0])
        .cross(corners[2] - corners[0])
        .normalize_or_zero();
    let lambert = normal.dot(LIGHT.normalize()).abs();
    let level = AMBIENT + (1.0 - AMBIENT) * lambert;
    model.map(|channel| (f32::from(channel) * level).round().clamp(0.0, 255.0) as u8)
}

/// How view coordinates land on pixels: the part scaled to fill the image, centred.
struct Frame {
    centre: Vec3,
    scale: f32,
    width: f32,
    height: f32,
}

impl Frame {
    fn around<'a>(
        viewed: impl Iterator<Item = &'a Vec3>,
        width_px: u32,
        height_px: u32,
    ) -> Option<Self> {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in viewed {
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

/// Fills one screen-space triangle, keeping whichever face is nearest the eye.
fn fill(
    colours: &mut [Rgb],
    depths: &mut [f32],
    width_px: u32,
    height_px: u32,
    screen: [Vec3; 3],
    colour: Rgb,
) {
    let [a, b, c] = screen;
    let area = edge(a, b, c.x, c.y);
    if area.abs() < f32::EPSILON {
        return;
    }
    let inverse = 1.0 / area;

    let (left, right) = bounds(a.x, b.x, c.x, width_px);
    let (top, bottom) = bounds(a.y, b.y, c.y, height_px);
    for y in top..bottom {
        for x in left..right {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Dividing by the signed area normalises both windings, so a mesh with
            // flipped faces fills the same pixels.
            let wa = edge(b, c, px, py) * inverse;
            let wb = edge(c, a, px, py) * inverse;
            let wc = edge(a, b, px, py) * inverse;
            if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                continue;
            }

            let depth = wa * a.z + wb * b.z + wc * c.z;
            let index = (y * width_px + x) as usize;
            if depth < depths[index] {
                depths[index] = depth;
                colours[index] = colour;
            }
        }
    }
}

fn edge(from: Vec3, to: Vec3, x: f32, y: f32) -> f32 {
    (to.x - from.x) * (y - from.y) - (to.y - from.y) * (x - from.x)
}

/// Half-open pixel range a triangle's extent covers, clamped to the image.
fn bounds(a: f32, b: f32, c: f32, limit_px: u32) -> (u32, u32) {
    let low = a.min(b).min(c).floor().max(0.0) as u32;
    let high = (a.max(b).max(c).ceil().max(0.0) as u32).min(limit_px);
    (low.min(limit_px), high)
}

#[cfg(test)]
mod tests;
