use serde::{Deserialize, Serialize};

use crate::{Mesh, Scalar, Vec2, Vec3};

/// Which image one face is textured from, and where its corners land on it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    /// Index of the image, in the order the file listed them.
    pub texture: usize,
    pub corners: [Vec2; 3],
}

impl Mapping {
    /// A face of the file's only image, which is what a single-material model has.
    pub fn sole(corners: [Vec2; 3]) -> Self {
        Self {
            texture: 0,
            corners,
        }
    }
}

/// Texture coordinates for a mesh, one pair per corner of each face.
///
/// Kept beside a [`Mesh`] rather than inside it: [`crate::weld`] merges vertices, and two
/// faces meeting at one vertex may name different corners of a texture, so a UV belongs to
/// a corner of a face and not to a vertex. Entry `i` matches `Mesh::faces[i]`, and is
/// `None` for a face the file left unmapped. A model built from materials carries one image
/// per material, so a face names its own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UvMap {
    faces: Vec<Option<Mapping>>,
}

impl UvMap {
    pub fn new(faces: Vec<Option<Mapping>>) -> Self {
        Self { faces }
    }

    /// A map covering every face from one image, for a file that maps all of them.
    pub fn whole(corners: Vec<[Vec2; 3]>) -> Self {
        Self::new(corners.into_iter().map(Mapping::sole).map(Some).collect())
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Faces the map has an entry for, mapped or not, which is every face of the mesh.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// Faces that actually carry a coordinate. Less than [`Self::len`] on a file that
    /// textured only part of its model.
    pub fn mapped(&self) -> usize {
        self.faces.iter().flatten().count()
    }

    /// One past the highest image index any face names, which is how many images the map
    /// needs to be read against.
    pub fn textures(&self) -> usize {
        self.faces
            .iter()
            .flatten()
            .map(|mapping| mapping.texture + 1)
            .max()
            .unwrap_or(0)
    }

    /// What one face is textured from, or `None` for a face the map does not cover.
    pub fn of_face(&self, face: usize) -> Option<Mapping> {
        self.faces.get(face).copied().flatten()
    }

    /// The same map over a mesh [`crate::weld`] has dropped faces from.
    ///
    /// `dropped` is [`crate::Welded::dropped`]: ascending indices into the original face
    /// list. Welding is the first thing done to an imported mesh, and it is the only step
    /// that renumbers faces under a map.
    #[must_use]
    pub fn without(&self, dropped: &[u32]) -> Self {
        let mut gone = dropped.iter().copied().peekable();
        let faces = self
            .faces
            .iter()
            .enumerate()
            .filter(|(face, _)| {
                while gone.peek().is_some_and(|&next| (next as usize) < *face) {
                    gone.next();
                }
                gone.peek() != Some(&(*face as u32))
            })
            .map(|(_, mapping)| *mapping)
            .collect();
        Self::new(faces)
    }

    /// The same map over a mesh whose named faces have had their winding turned round.
    ///
    /// [`crate::orient_outward`] swaps two corners of a face to flip it, and a corner's
    /// coordinate has to follow the corner.
    #[must_use]
    pub fn flipping(&self, flipped: &[u32]) -> Self {
        let mut faces = self.faces.clone();
        for face in flipped {
            if let Some(Some(mapping)) = faces.get_mut(*face as usize) {
                mapping.corners.swap(1, 2);
            }
        }
        Self::new(faces)
    }

    /// Which image `point` is textured from and where it lands on it, taking the point to
    /// lie on `face` of `mesh`.
    ///
    /// This is how a point found by [`crate::closest_point`] gets its UV: the barycentric
    /// weights of the point in the triangle weight the face's three corners.
    pub fn at(&self, mesh: &Mesh, face: usize, point: Vec3) -> Option<(usize, Vec2)> {
        let mapping = self.of_face(face)?;
        let triangle = mesh.triangle(face)?;
        let weights = barycentric(triangle.a, triangle.b, triangle.c, point)?;
        let uv = std::iter::zip(mapping.corners, weights)
            .map(|(corner, weight)| corner * weight)
            .sum();
        Some((mapping.texture, uv))
    }
}

/// The image a [`UvMap`] addresses, as one height per pixel.
///
/// Heights run from 0 to 1 and are what a texture means to a printer: how far the surface
/// moves where the coordinate lands. Rows run from the top of the image, the way every
/// image format stores them, while a texture coordinate's V runs up from the bottom.
#[derive(Debug, Clone, PartialEq)]
pub struct Heightmap {
    width: usize,
    height: usize,
    samples: Vec<Scalar>,
}

impl Heightmap {
    /// `None` unless there is exactly one sample per pixel and both sides are non-zero.
    pub fn new(width: usize, height: usize, samples: Vec<Scalar>) -> Option<Self> {
        (width > 0 && height > 0 && samples.len() == width * height).then_some(Self {
            width,
            height,
            samples,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// The height at `uv`, bilinear between the four nearest pixels.
    ///
    /// Coordinates outside the unit square wrap, which is the default tile style of both
    /// OBJ and 3MF.
    pub fn sample(&self, uv: Vec2) -> Scalar {
        let x = wrap(uv.x) * self.width as Scalar - 0.5;
        let y = wrap(1.0 - uv.y) * self.height as Scalar - 0.5;
        let (left, fx) = (x.floor(), x - x.floor());
        let (top, fy) = (y.floor(), y - y.floor());

        let columns = [
            wrapped(left as isize, self.width),
            wrapped(left as isize + 1, self.width),
        ];
        let rows = [
            wrapped(top as isize, self.height),
            wrapped(top as isize + 1, self.height),
        ];

        let at = |row: usize, column: usize| self.samples[row * self.width + column];
        let upper = at(rows[0], columns[0]) * (1.0 - fx) + at(rows[0], columns[1]) * fx;
        let lower = at(rows[1], columns[0]) * (1.0 - fx) + at(rows[1], columns[1]) * fx;
        upper * (1.0 - fy) + lower * fy
    }
}

/// `value` brought into the unit interval, wrapping rather than clamping.
fn wrap(value: Scalar) -> Scalar {
    let fraction = value - value.floor();
    if fraction.is_finite() { fraction } else { 0.0 }
}

/// `index` brought into `0..len`, wrapping either way.
fn wrapped(index: isize, len: usize) -> usize {
    index.rem_euclid(len.cast_signed()) as usize
}

/// Weights of `point` in the triangle `a b c`, or `None` for a degenerate triangle.
fn barycentric(a: Vec3, b: Vec3, c: Vec3, point: Vec3) -> Option<[Scalar; 3]> {
    let normal = (b - a).cross(c - a);
    let twice_area = normal.length_squared();
    if twice_area <= Scalar::EPSILON {
        return None;
    }
    // A corner's weight is the area of the sub-triangle opposite it over the whole, and
    // `normal.dot` of a cross product is twice that area with its sign.
    let at_a = normal.dot((c - b).cross(point - b)) / twice_area;
    let at_b = normal.dot((a - c).cross(point - c)) / twice_area;
    Some([at_a, at_b, 1.0 - at_a - at_b])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A right triangle in the XY plane, with the unit square's corners as its UVs.
    fn mapped() -> (Mesh, UvMap) {
        let mesh = Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2]],
        );
        let uvs = UvMap::whole(vec![[Vec2::ZERO, Vec2::X, Vec2::Y]]);
        (mesh, uvs)
    }

    #[test]
    fn a_corner_maps_to_its_own_coordinate() {
        let (mesh, uvs) = mapped();
        assert_eq!(uvs.at(&mesh, 0, Vec3::ZERO), Some((0, Vec2::ZERO)));
        assert_eq!(
            uvs.at(&mesh, 0, Vec3::new(2.0, 0.0, 0.0)),
            Some((0, Vec2::new(1.0, 0.0)))
        );
    }

    #[test]
    fn the_middle_of_an_edge_maps_to_the_middle_of_its_two_corners() {
        let (mesh, uvs) = mapped();
        let (_, middle) = uvs.at(&mesh, 0, Vec3::new(1.0, 1.0, 0.0)).expect("on face");
        assert!(
            (middle - Vec2::splat(0.5)).length() < 1e-6,
            "got {middle}, expected the midpoint of (1,0) and (0,1)"
        );
    }

    #[test]
    fn a_face_the_map_does_not_cover_has_no_coordinate() {
        let (mesh, _) = mapped();
        assert!(UvMap::default().at(&mesh, 0, Vec3::ZERO).is_none());
    }

    #[test]
    fn a_welded_away_face_takes_its_coordinates_with_it() {
        let corners = |n: Scalar| [Vec2::splat(n); 3];
        let uvs = UvMap::whole(vec![corners(0.0), corners(1.0), corners(2.0)]);
        let kept = uvs.without(&[1]);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept.of_face(1), Some(Mapping::sole(corners(2.0))));
    }

    #[test]
    fn a_face_names_the_image_of_its_own_material() {
        let corners = [Vec2::ZERO; 3];
        let uvs = UvMap::new(vec![
            Some(Mapping::sole(corners)),
            Some(Mapping {
                texture: 2,
                corners,
            }),
            None,
        ]);
        assert_eq!(
            uvs.textures(),
            3,
            "the highest index names how many are needed"
        );
        assert_eq!(uvs.of_face(1).map(|mapping| mapping.texture), Some(2));
    }

    #[test]
    fn a_flipped_face_flips_its_last_two_corners() {
        let uvs = UvMap::whole(vec![[Vec2::ZERO, Vec2::X, Vec2::Y]]);
        assert_eq!(
            uvs.flipping(&[0]).of_face(0),
            Some(Mapping::sole([Vec2::ZERO, Vec2::Y, Vec2::X]))
        );
    }

    #[test]
    fn a_face_the_file_left_unmapped_has_no_coordinate() {
        let (mesh, _) = mapped();
        let holed = UvMap::new(vec![None]);
        assert_eq!(holed.len(), 1, "the face is still counted");
        assert_eq!(holed.mapped(), 0);
        assert!(holed.at(&mesh, 0, Vec3::ZERO).is_none());
    }

    #[test]
    fn a_degenerate_face_has_no_coordinate() {
        let mesh = Mesh::new(vec![Vec3::ZERO; 3], vec![[0, 1, 2]]);
        let uvs = UvMap::whole(vec![[Vec2::ZERO; 3]]);
        assert!(uvs.at(&mesh, 0, Vec3::ZERO).is_none());
    }
}

#[cfg(test)]
mod heightmap_tests {
    use super::*;

    /// Black on the left of the image, white on the right, one row.
    fn split() -> Heightmap {
        Heightmap::new(2, 1, vec![0.0, 1.0]).expect("two samples over two pixels")
    }

    #[test]
    fn a_map_needs_one_sample_per_pixel() {
        assert!(Heightmap::new(2, 2, vec![0.0; 3]).is_none());
        assert!(Heightmap::new(0, 1, Vec::new()).is_none());
    }

    /// A sample is a weighted sum of stored values, so it is exact only to a rounding.
    fn close(left: Scalar, right: Scalar) -> bool {
        (left - right).abs() < 1e-6
    }

    #[test]
    fn a_pixel_centre_samples_its_own_value() {
        let map = split();
        assert!(close(map.sample(Vec2::new(0.25, 0.5)), 0.0));
        assert!(close(map.sample(Vec2::new(0.75, 0.5)), 1.0));
    }

    #[test]
    fn between_two_pixels_the_sample_is_between_their_values() {
        let map = split();
        let middle = map.sample(Vec2::new(0.5, 0.5));
        assert!(
            (middle - 0.5).abs() < 1e-6,
            "got {middle}, expected the midpoint of 0 and 1"
        );
    }

    #[test]
    fn a_coordinate_outside_the_square_wraps() {
        let map = split();
        assert!(close(
            map.sample(Vec2::new(1.25, 0.5)),
            map.sample(Vec2::new(0.25, 0.5))
        ));
        assert!(close(
            map.sample(Vec2::new(-0.25, 0.5)),
            map.sample(Vec2::new(0.75, 0.5))
        ));
    }

    #[test]
    fn v_runs_up_from_the_bottom_of_the_image() {
        let map = Heightmap::new(1, 2, vec![1.0, 0.0]).expect("two samples over two pixels");
        assert!(
            close(map.sample(Vec2::new(0.5, 0.75)), 1.0),
            "the top row of the image is V near one"
        );
        assert!(close(map.sample(Vec2::new(0.5, 0.25)), 0.0));
    }
}
