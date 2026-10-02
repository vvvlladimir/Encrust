use core_geometry::{Mesh, Transform, Vec3};

use crate::image::Thumbnail;
use crate::settings::{Rgb, ThumbnailSettings};

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

/// Where the plate is looked at from, in plate coordinates with Z up.
///
/// The front right corner, a little above the part: the angle every slicer's thumbnail
/// uses, because it shows a silhouette, a top and a side at once.
const EYE: Vec3 = Vec3::new(1.0, -1.0, 0.75);

/// Direction the light comes from, in view coordinates: over the camera's left shoulder.
const LIGHT: Vec3 = Vec3::new(-0.3, 0.55, -1.0);

/// Share of a face's colour that does not depend on which way it points, so that a face
/// turned away from the light is still read as a surface rather than as a hole.
const AMBIENT: f32 = 0.28;

/// Fraction of the image left empty around the part on each side.
const MARGIN: f32 = 0.06;

/// Renders every part into one shaded image, framed so the whole plate fits.
///
/// The view fits what it is given rather than the build volume: a thumbnail is looked at
/// a centimetre wide, and a part drawn to scale inside a 200 mm plate is a speck.
pub fn render(parts: &[Part<'_>], settings: &ThumbnailSettings) -> Thumbnail {
    let (width, height) = (settings.width_px, settings.height_px);
    let mut colours = vec![settings.background; (width as usize) * (height as usize)];
    let mut depths = vec![f32::INFINITY; colours.len()];

    let viewed: Vec<Vec<Vec3>> = parts.iter().map(project).collect();
    let Some(frame) = Frame::around(&viewed, width, height) else {
        return Thumbnail::filled(width, height, settings.background);
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

    Thumbnail::from_pixels(width, height, colours)
        .unwrap_or_else(|| Thumbnail::filled(width, height, settings.background))
}

/// Every vertex of one part in view coordinates: right, up and depth away from the eye.
fn project(part: &Part<'_>) -> Vec<Vec3> {
    let forward = -EYE.normalize();
    let right = forward.cross(Vec3::Z).normalize();
    let up = right.cross(forward);
    let matrix = part.transform.to_matrix();

    part.mesh
        .vertices
        .iter()
        .map(|vertex| {
            let placed = matrix.transform_point3(*vertex);
            Vec3::new(placed.dot(right), placed.dot(up), placed.dot(forward))
        })
        .collect()
}

fn corners(vertices: &[Vec3], face: [u32; 3]) -> Option<[Vec3; 3]> {
    Some([
        *vertices.get(face[0] as usize)?,
        *vertices.get(face[1] as usize)?,
        *vertices.get(face[2] as usize)?,
    ])
}

/// Flat Lambert shading of one face, two-sided.
///
/// The sign of the normal is dropped rather than trusted: a repaired mesh points its
/// faces outwards, but a thumbnail of an unrepaired one should still show a solid.
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
    fn around(viewed: &[Vec<Vec3>], width_px: u32, height_px: u32) -> Option<Self> {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in viewed.iter().flatten() {
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
mod tests {
    use super::*;

    /// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
    fn unit_cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    fn settings() -> ThumbnailSettings {
        ThumbnailSettings {
            width_px: 64,
            height_px: 64,
            ..ThumbnailSettings::default()
        }
    }

    #[test]
    fn an_empty_plate_renders_to_the_background() {
        let settings = settings();
        let image = render(&[], &settings);
        assert!(
            image
                .pixels()
                .iter()
                .all(|pixel| *pixel == settings.background)
        );
    }

    #[test]
    fn a_cube_covers_the_middle_and_leaves_the_corners_empty() {
        let mesh = unit_cube();
        let settings = settings();
        let image = render(&[Part::placed(&mesh)], &settings);

        let middle = image.pixels()[(32 * 64 + 32) as usize];
        assert_ne!(middle, settings.background, "the part covers the centre");
        for corner in [0usize, 63, 63 * 64, 64 * 64 - 1] {
            assert_eq!(
                image.pixels()[corner],
                settings.background,
                "the margin keeps the part off the border"
            );
        }
    }

    #[test]
    fn three_faces_of_a_cube_are_shaded_differently() {
        let mesh = unit_cube();
        let image = render(&[Part::placed(&mesh)], &settings());

        let mut shades: Vec<Rgb> = image.pixels().to_vec();
        shades.sort_unstable();
        shades.dedup();
        assert!(
            shades.len() >= 4,
            "background plus one shade per visible face, got {}",
            shades.len()
        );
    }

    #[test]
    fn the_view_frames_the_part_wherever_it_stands() {
        let mesh = unit_cube();
        let settings = settings();
        let here = render(&[Part::placed(&mesh)], &settings);
        let there = render(
            &[Part::new(
                &mesh,
                Transform::from_translation(Vec3::new(40.0, -25.0, 3.0)),
            )],
            &settings,
        );

        assert_eq!(
            here, there,
            "the frame follows the part, so moving it on the plate changes nothing"
        );
    }

    #[test]
    fn a_nearer_part_hides_the_one_behind_it() {
        let mesh = unit_cube();
        let settings = ThumbnailSettings {
            model: [255, 255, 255],
            ..settings()
        };
        // The second cube sits directly behind the first along the view direction, so the
        // image must look exactly like the front one alone.
        let behind = Transform::from_translation(EYE.normalize() * -4.0);
        let alone = render(&[Part::placed(&mesh)], &settings);
        let stacked = render(&[Part::placed(&mesh), Part::new(&mesh, behind)], &settings);

        let lit = |image: &Thumbnail| {
            image
                .pixels()
                .iter()
                .filter(|pixel| **pixel != settings.background)
                .count()
        };
        assert_eq!(
            lit(&alone),
            lit(&stacked),
            "the far cube is hidden behind the near one"
        );
    }
}
