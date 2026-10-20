use std::borrow::Cow;
use std::sync::Arc;

use core_geometry::{
    Bvh, ClosestPoint, Heightmap, Mesh, Scalar, UvMap, Vec2, Vec3, collapse_flat_faces,
};
use serde::{Deserialize, Serialize};

use crate::build::DEFAULT_BUDGET_BYTES;
use crate::cancel::Cancel;
use crate::error::VolumeError;
use crate::extract::extract_clustered;
use crate::hollow::lattice_mm;

mod basis;
mod displace;
mod mask;
mod refine;
mod smooth;
mod snap;

pub use basis::ReliefBasis;
pub use displace::{press_painted, press_painted_tiled};
pub use mask::AngleMask;
use mask::Held;

/// How far the texture may move the surface, how its grey is read, which faces it leaves
/// alone, what it is cut on, and what the field may cost while it is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReliefSettings {
    /// How far the surface moves where the texture is white, in millimetres. Positive
    /// raises the relief out of the model, negative sinks it in.
    pub amplitude_mm: Scalar,
    /// How fine the lattice is, from 0 to 1, as everywhere else in this crate.
    pub precision: Scalar,
    pub budget_bytes: usize,
    /// Reads the texture's black as high and its white as low.
    #[serde(default)]
    pub invert: bool,
    /// Reads mid grey as the surface itself, so white raises it and black sinks it, each
    /// by half the amplitude, and the model keeps about its size.
    #[serde(default)]
    pub symmetric: bool,
    /// How far the texture is blurred before it is read, in pixels of the texture.
    #[serde(default)]
    pub smoothing_px: Scalar,
    #[serde(default)]
    pub mask: AngleMask,
}

impl Default for ReliefSettings {
    fn default() -> Self {
        Self {
            amplitude_mm: 0.5,
            precision: 0.5,
            budget_bytes: DEFAULT_BUDGET_BYTES,
            invert: false,
            symmetric: false,
            smoothing_px: 0.0,
            mask: AngleMask::default(),
        }
    }
}

impl ReliefSettings {
    /// The lattice the relief is cut on, given the model's surface area.
    ///
    /// The amplitude stands in for a wall: it is the smallest thing the field has to
    /// carry, so it is what decides how fine the lattice has to be.
    pub fn voxel_mm(&self, area_mm2: Scalar) -> Scalar {
        lattice_mm(self.amplitude_mm.abs(), self.precision, area_mm2)
    }

    fn check(&self) -> Result<(), VolumeError> {
        if !self.amplitude_mm.is_finite() || self.amplitude_mm == 0.0 {
            return Err(VolumeError::BadAmplitude(self.amplitude_mm));
        }
        if !(self.smoothing_px.is_finite() && self.smoothing_px >= 0.0) {
            return Err(VolumeError::BadSmoothing(self.smoothing_px));
        }
        self.mask.check()
    }

    /// How far, in amplitudes, a height read off the prepared texture moves the surface.
    fn lifted(&self, height: Scalar) -> Scalar {
        if self.symmetric { height - 0.5 } else { height }
    }

    /// The middle of the range the surface moves outward over, and how far either side of
    /// it it reaches, millimetres.
    fn range_mm(&self) -> (Scalar, Scalar) {
        let ends = [self.lifted(0.0), self.lifted(1.0)].map(|lift| self.amplitude_mm * lift);
        let (least_mm, most_mm) = (ends[0].min(ends[1]), ends[0].max(ends[1]));
        (
            Scalar::midpoint(least_mm, most_mm),
            (most_mm - least_mm) / 2.0,
        )
    }
}

/// `image` as a relief pressed with `settings` reads it: smoothed, then turned over where
/// `invert` asks. The window draws this, so what it shows is what is pressed.
pub fn prepared<'a>(image: &'a Heightmap, settings: &ReliefSettings) -> Cow<'a, Heightmap> {
    if settings.smoothing_px <= 0.0 && !settings.invert {
        return Cow::Borrowed(image);
    }
    let mut image = smooth::smoothed(image, settings.smoothing_px);
    if settings.invert {
        for height in image.samples_mut() {
            *height = 1.0 - *height;
        }
    }
    Cow::Owned(image)
}

/// A model with its texture pressed in, and what that cost.
#[derive(Debug, Clone, PartialEq)]
pub struct Relief {
    pub mesh: Mesh,
    /// The lattice the relief came out on, in millimetres.
    pub voxel_mm: Scalar,
    /// Whether the budget made that lattice coarser than precision asked for, which is
    /// the one thing that silently costs the relief its detail.
    pub coarsened: bool,
}

/// Presses a texture into `mesh`, as geometry rather than as exposure.
///
/// Every lattice point of the field is moved by the height its own nearest surface point
/// reads out of the texture, and the surface is meshed again where that leaves it, so the
/// relief needs no subdivision of the model it is pressed into; see
/// `docs/design/volume.md`.
pub fn press(
    mesh: &Mesh,
    bvh: &Bvh,
    uvs: &UvMap,
    heights: &[Heightmap],
    settings: &ReliefSettings,
) -> Result<Relief, VolumeError> {
    press_reusing(mesh, bvh, uvs, heights, settings, None, Cancel::never())
        .map(|(relief, _)| relief)
}

/// Refuses a map that is not `mesh`'s, or that names an image `heights` does not hold.
fn mapped(mesh: &Mesh, uvs: &UvMap, heights: &[Heightmap]) -> Result<(), VolumeError> {
    // A map covering part of the mesh is honest — what it does not cover does not move —
    // but one indexed by a different face list is not the mesh's map at all.
    if uvs.len() != mesh.faces.len() || uvs.mapped() == 0 {
        return Err(VolumeError::UnmappedMesh {
            faces: mesh.faces.len(),
            mapped: uvs.mapped(),
        });
    }
    // A model textured by material carries one image per material, and a face names its
    // own: a map naming an image that was not handed over has nothing to read.
    if uvs.textures() > heights.len() {
        return Err(VolumeError::MissingTexture {
            wanted: uvs.textures(),
            given: heights.len(),
        });
    }
    Ok(())
}

/// Refuses a mesh with no faces to press into, or settings no press can be made with.
fn pressable(mesh: &Mesh, settings: &ReliefSettings) -> Result<(), VolumeError> {
    if mesh.aabb().is_none() {
        return Err(VolumeError::EmptyMesh);
    }
    settings.check()
}

/// [`press`] over `basis` where it fits this model and these settings, and the basis the
/// press stood on, for the caller to keep for another press of the same mesh. `cancel` is
/// asked between the stages of the press and inside its field.
pub fn press_reusing(
    mesh: &Mesh,
    bvh: &Bvh,
    uvs: &UvMap,
    heights: &[Heightmap],
    settings: &ReliefSettings,
    basis: Option<Arc<ReliefBasis>>,
    cancel: Cancel<'_>,
) -> Result<(Relief, Arc<ReliefBasis>), VolumeError> {
    pressable(mesh, settings)?;
    mapped(mesh, uvs, heights)?;

    let heights: Vec<_> = heights
        .iter()
        .map(|image| prepared(image, settings))
        .collect();
    let sample = |face: usize, point: Vec3| {
        let (texture, uv) = uvs.at(mesh, face, point)?;
        Some(heights.get(texture)?.sample(uv))
    };
    displaced(mesh, bvh, settings, &sample, (basis, cancel))
}

/// How near their shared plane cells must stand to be clustered into one facet, in
/// voxels: a flat face the relief left alone still clusters, the relief itself does not.
const RELIEF_SLACK_VOXELS: Scalar = 0.05;

/// How a picture is laid over a model that has no map of its own: as square tiles of
/// `tile_mm`, turned by `angle_deg` on each of the three planes it is projected along.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tiling {
    pub tile_mm: Scalar,
    pub angle_deg: Scalar,
}

impl Tiling {
    fn check(self) -> Result<(), VolumeError> {
        if !self.tile_mm.is_finite() || self.tile_mm <= 0.0 {
            return Err(VolumeError::BadTile(self.tile_mm));
        }
        Ok(())
    }
}

impl Default for Tiling {
    fn default() -> Self {
        Self {
            tile_mm: 12.0,
            angle_deg: 0.0,
        }
    }
}

/// How sharply the three projections hand over to one another as the surface turns: the
/// normal's components to this power. Four keeps a face read from one plane until it is
/// well past forty-five degrees.
const BLEND_POWER: i32 = 4;

/// Presses `image` into `mesh` as tiles projected along X, Y and Z, each weighted by how
/// squarely the surface faces it, so a model needs no map; see `docs/design/volume.md`.
pub fn press_tiled(
    mesh: &Mesh,
    bvh: &Bvh,
    image: &Heightmap,
    tiling: Tiling,
    settings: &ReliefSettings,
) -> Result<Relief, VolumeError> {
    press_tiled_reusing(mesh, bvh, image, tiling, settings, None, Cancel::never())
        .map(|(relief, _)| relief)
}

/// [`press_tiled`] over `basis` where it fits this model and these settings, and the
/// basis the press stood on, for the caller to keep beside this very mesh. `cancel` is
/// asked as in [`press_reusing`].
pub fn press_tiled_reusing(
    mesh: &Mesh,
    bvh: &Bvh,
    image: &Heightmap,
    tiling: Tiling,
    settings: &ReliefSettings,
    basis: Option<Arc<ReliefBasis>>,
    cancel: Cancel<'_>,
) -> Result<(Relief, Arc<ReliefBasis>), VolumeError> {
    pressable(mesh, settings)?;
    tiling.check()?;
    let image = prepared(image, settings);
    let tiled = tiled(&image, tiling);
    let sample = |face: usize, point: Vec3| {
        tiled(
            point,
            mesh.triangle(face)?.normal_unnormalized().try_normalize()?,
        )
    };
    displaced(mesh, bvh, settings, &sample, (basis, cancel))
}

/// The height `image`, laid as `tiling` lays it, reads at a point of a surface facing a
/// unit normal: its three planes weighted by how squarely the surface faces each.
fn tiled(image: &Heightmap, tiling: Tiling) -> impl Fn(Vec3, Vec3) -> Option<Scalar> + Sync + '_ {
    let (sin, cos) = tiling.angle_deg.to_radians().sin_cos();
    let on_plane = move |a: Scalar, b: Scalar| {
        let turned = Vec2::new(a * cos - b * sin, a * sin + b * cos);
        image.sample(turned / tiling.tile_mm)
    };
    move |point: Vec3, normal: Vec3| {
        let weights = normal.abs().powf(BLEND_POWER as Scalar);
        let total = weights.x + weights.y + weights.z;
        let height = weights.x * on_plane(point.y, point.z)
            + weights.y * on_plane(point.x, point.z)
            + weights.z * on_plane(point.x, point.y);
        (total > 0.0).then(|| height / total)
    }
}

/// The surface of `mesh` moved by the height `sample` reads at its nearest point, over
/// `reused` where it fits and a basis built afresh where it does not.
fn displaced(
    mesh: &Mesh,
    bvh: &Bvh,
    settings: &ReliefSettings,
    sample: &(dyn Fn(usize, Vec3) -> Option<Scalar> + Sync),
    (reused, cancel): (Option<Arc<ReliefBasis>>, Cancel<'_>),
) -> Result<(Relief, Arc<ReliefBasis>), VolumeError> {
    let basis = match reused {
        Some(basis) if basis.fits(mesh, settings) => basis,
        _ => Arc::new(ReliefBasis::build(mesh, bvh, settings, cancel)?),
    };
    let (surface, hierarchy) = basis.surface(mesh, bvh);
    let press = Press {
        mesh: surface,
        bvh: hierarchy,
        model: basis.is_united().then_some(bvh),
        held: Held::of(surface, &settings.mask),
        sample,
        settings,
    };
    let relief = Relief {
        mesh: press.pressed(&basis, cancel)?,
        voxel_mm: basis.voxel_mm(),
        coarsened: basis.coarsened(),
    };
    Ok((relief, basis))
}

/// What one relief run reads its heights from and moves the surface by.
struct Press<'a> {
    /// The surface moved: the model, or its shells unioned.
    mesh: &'a Mesh,
    bvh: &'a Bvh,
    /// The model the texture is read off, when that is not the surface moved.
    model: Option<&'a Bvh>,
    held: Option<Held>,
    sample: &'a (dyn Fn(usize, Vec3) -> Option<Scalar> + Sync),
    settings: &'a ReliefSettings,
}

impl Press<'_> {
    /// The pressed surface over `basis`, meshed and taken onto the surface the texture
    /// describes.
    fn pressed(&self, basis: &ReliefBasis, cancel: Cancel<'_>) -> Result<Mesh, VolumeError> {
        let field = self.field_on(basis, cancel);
        let mut pressed = extract_clustered(&field, RELIEF_SLACK_VOXELS, cancel);
        // Each stage given up on hands on what it had, which is no relief at all.
        if cancel.asked() {
            return Err(VolumeError::Cancelled);
        }
        self.snap(&mut pressed, &field);
        // Snapping can bring two vertices onto one point where a feature narrows to one.
        collapse_flat_faces(&mut pressed);
        Ok(pressed)
    }

    /// The signed distance `value` at `point`, whose nearest point of the model is
    /// `found`, once the texture has moved the surface where the mask lets it; see
    /// `docs/design/volume.md`.
    fn moved(&self, value: Scalar, point: Vec3, found: &ClosestPoint) -> Scalar {
        let held = self
            .held
            .as_ref()
            .and_then(|held| Some((held, held.nearest(point)?)));
        if held.is_some_and(|(_, at)| Held::is_held(&at, found)) {
            return value;
        }
        let moved = self.height(found).map_or(value, |height| {
            value - self.settings.amplitude_mm * self.settings.lifted(height)
        });
        match held {
            // Only a point outside the model can be wrapped round an edge into it.
            Some((held, at)) if value > 0.0 => held.clipped(moved, point, &at),
            _ => moved,
        }
    }

    /// The texture's height at `found`, read off the model's own face there.
    fn height(&self, found: &ClosestPoint) -> Option<Scalar> {
        let face = match self.model {
            Some(bvh) => bvh.closest(found.point)?.face,
            None => found.face,
        };
        (self.sample)(face, found.point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Aabb, Mapping, Vec2, Vec3};

    const NEVER: Cancel<'static> = Cancel::never();

    /// A closed cube of `side` millimetres standing on the origin, with every face's UVs
    /// taken from where its corners stand in X and Y.
    fn cube(side: Scalar) -> (Mesh, UvMap) {
        let corners = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(side, 0.0, 0.0),
            Vec3::new(side, side, 0.0),
            Vec3::new(0.0, side, 0.0),
            Vec3::new(0.0, 0.0, side),
            Vec3::new(side, 0.0, side),
            Vec3::new(side, side, side),
            Vec3::new(0.0, side, side),
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

        let mesh = Mesh::new(corners.to_vec(), faces);
        let planar = |vertex: Vec3| Vec2::new(vertex.x / side, vertex.y / side);
        let uvs = UvMap::whole(
            mesh.faces
                .iter()
                .map(|face| face.map(|index| planar(corners[index as usize])))
                .collect(),
        );
        (mesh, uvs)
    }

    fn flat(height: Scalar) -> Heightmap {
        Heightmap::new(1, 1, vec![height]).expect("one sample over one pixel")
    }

    /// Black over the left half of the unit square, white over the right, with a pixel
    /// either side of each boundary so that both halves have a plateau of their own.
    fn half() -> Heightmap {
        Heightmap::new(4, 1, vec![0.0, 0.0, 1.0, 1.0]).expect("four samples over four pixels")
    }

    fn pressed_cube(heights: &Heightmap, amplitude_mm: Scalar) -> Mesh {
        let (mesh, uvs) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        press(
            &mesh,
            &bvh,
            &uvs,
            std::slice::from_ref(heights),
            &ReliefSettings {
                amplitude_mm,
                precision: 0.5,
                ..ReliefSettings::default()
            },
        )
        .expect("a mapped cube can be pressed")
        .mesh
    }

    fn bounds(mesh: &Mesh) -> Aabb {
        mesh.aabb().expect("the pressed mesh has faces")
    }

    /// What the lattice itself can promise: marching the cells places the surface within
    /// half a voxel, and `ReliefSettings` cuts a 0.4 mm amplitude at 0.13 mm.
    const TOLERANCE_MM: Scalar = 0.1;

    #[test]
    fn a_white_picture_tiled_over_a_cube_moves_every_side_out() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let settings = ReliefSettings {
            amplitude_mm: 0.4,
            ..ReliefSettings::default()
        };
        let pressed = press_tiled(&mesh, &bvh, &flat(1.0), Tiling::default(), &settings)
            .expect("any closed mesh can be tiled");
        let grown = bounds(&pressed.mesh);
        // Every side reads white from the plane it faces, so all six move out 0.4 mm.
        assert!(
            (grown.maxs.x - 10.4).abs() < TOLERANCE_MM && (grown.mins.y + 0.4).abs() < TOLERANCE_MM,
            "got {grown:?}"
        );
    }

    #[test]
    fn a_tile_of_no_size_is_refused() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let tiling = Tiling {
            tile_mm: 0.0,
            angle_deg: 0.0,
        };
        let refused = press_tiled(&mesh, &bvh, &flat(1.0), tiling, &ReliefSettings::default());
        assert!(matches!(refused, Err(VolumeError::BadTile(_))));
    }

    #[test]
    fn a_white_texture_moves_the_whole_surface_out() {
        let grown = bounds(&pressed_cube(&flat(1.0), 0.4));
        assert!(
            (grown.maxs.z - 10.4).abs() < TOLERANCE_MM && (grown.mins.z + 0.4).abs() < TOLERANCE_MM,
            "got {grown:?}, expected a 10 mm cube grown 0.4 mm on every side"
        );
    }

    #[test]
    fn a_negative_amplitude_sinks_the_surface_in() {
        let shrunk = bounds(&pressed_cube(&flat(1.0), -0.4));
        assert!(
            (shrunk.maxs.z - 9.6).abs() < TOLERANCE_MM,
            "got {shrunk:?}, expected a 10 mm cube taken 0.4 mm in on every side"
        );
    }

    #[test]
    fn a_black_texture_leaves_the_surface_where_it_was() {
        let same = bounds(&pressed_cube(&flat(0.0), 0.4));
        assert!(
            (same.maxs.z - 10.0).abs() < TOLERANCE_MM && same.mins.z.abs() < TOLERANCE_MM,
            "got {same:?}, expected the 10 mm cube unmoved"
        );
    }

    #[test]
    fn half_a_texture_moves_and_half_stays() {
        let mesh = pressed_cube(&half(), 0.4);
        // Inside a plateau of the texture, away from both the boundary between the halves
        // and the wrap at the edges of the square.
        let top = |from: Scalar, to: Scalar| {
            mesh.vertices
                .iter()
                .filter(|vertex| (from..to).contains(&vertex.x))
                .map(|vertex| vertex.z)
                .fold(Scalar::MIN, Scalar::max)
        };
        let white = top(6.5, 8.5);
        let black = top(1.5, 3.5);
        assert!(
            (white - 10.4).abs() < TOLERANCE_MM && (black - 10.0).abs() < TOLERANCE_MM,
            "white half topped out at {white} and black at {black}, expected 10.4 and 10.0"
        );
    }

    #[test]
    fn a_pressed_solid_is_still_a_solid() {
        let pressed = pressed_cube(&flat(1.0), 0.4);
        assert!(
            core_geometry::diagnose(&pressed).is_closed(),
            "a relief comes out of the field as one closed surface"
        );
    }

    #[test]
    fn a_face_the_map_does_not_cover_stays_where_it_was() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let holed = UvMap::new(vec![None; mesh.faces.len()])
            .without(&[])
            .flipping(&[]);
        let Err(error) = press(
            &mesh,
            &bvh,
            &holed,
            &[flat(1.0)],
            &ReliefSettings::default(),
        ) else {
            panic!("a map that covers no face has nothing to press");
        };
        assert!(matches!(error, VolumeError::UnmappedMesh { mapped: 0, .. }));

        // One face mapped is enough to run, and the rest of the cube keeps its size.
        let mut corners = vec![None; mesh.faces.len()];
        corners[2] = Some(Mapping::sole([Vec2::ZERO; 3]));
        let pressed = press(
            &mesh,
            &bvh,
            &UvMap::new(corners),
            &[flat(1.0)],
            &ReliefSettings {
                amplitude_mm: 0.4,
                ..ReliefSettings::default()
            },
        )
        .expect("one mapped face is a map")
        .mesh;
        let bounds = bounds(&pressed);
        assert!(
            (bounds.mins.z).abs() < TOLERANCE_MM,
            "the unmapped underside stayed on the plate, got {}",
            bounds.mins.z
        );
    }

    #[test]
    fn each_face_is_pressed_from_its_own_image() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        // The lid off the first image, which is white; everything else off the second,
        // which is black.
        let faces = (0..mesh.faces.len())
            .map(|face| {
                Some(Mapping {
                    texture: usize::from(!(2..4).contains(&face)),
                    corners: [Vec2::ZERO; 3],
                })
            })
            .collect();

        let pressed = press(
            &mesh,
            &bvh,
            &UvMap::new(faces),
            &[flat(1.0), flat(0.0)],
            &ReliefSettings {
                amplitude_mm: 0.4,
                ..ReliefSettings::default()
            },
        )
        .expect("two images are two images")
        .mesh;

        let grown = bounds(&pressed);
        assert!(
            (grown.maxs.z - 10.4).abs() < TOLERANCE_MM && grown.mins.z.abs() < TOLERANCE_MM,
            "got {grown:?}, expected only the lid to have moved"
        );
    }

    #[test]
    fn a_map_naming_an_image_that_was_not_given_is_refused() {
        let (mesh, uvs) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let second = UvMap::new(
            (0..mesh.faces.len())
                .map(|face| {
                    uvs.of_face(face).map(|mapping| Mapping {
                        texture: 1,
                        ..mapping
                    })
                })
                .collect(),
        );
        assert!(matches!(
            press(
                &mesh,
                &bvh,
                &second,
                &[flat(1.0)],
                &ReliefSettings::default()
            ),
            Err(VolumeError::MissingTexture {
                wanted: 2,
                given: 1
            })
        ));
    }

    #[test]
    fn an_amplitude_of_nothing_is_refused() {
        let (mesh, uvs) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let settings = ReliefSettings {
            amplitude_mm: 0.0,
            ..ReliefSettings::default()
        };
        assert!(matches!(
            press(&mesh, &bvh, &uvs, &[flat(1.0)], &settings),
            Err(VolumeError::BadAmplitude(0.0))
        ));
    }

    #[test]
    fn a_map_that_does_not_cover_the_mesh_is_refused() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let short = UvMap::whole(vec![[Vec2::ZERO; 3]]);
        assert!(matches!(
            press(
                &mesh,
                &bvh,
                &short,
                &[flat(1.0)],
                &ReliefSettings::default()
            ),
            Err(VolumeError::UnmappedMesh {
                faces: 12,
                mapped: 1
            })
        ));
    }

    #[test]
    fn an_empty_mesh_has_nothing_to_press_into() {
        let mesh = Mesh::default();
        let bvh = Bvh::build(&mesh);
        assert!(matches!(
            press(
                &mesh,
                &bvh,
                &UvMap::default(),
                &[flat(1.0)],
                &ReliefSettings::default()
            ),
            Err(VolumeError::EmptyMesh)
        ));
    }

    fn tiled_cube(heights: &Heightmap, settings: ReliefSettings) -> Aabb {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let pressed = press_tiled(&mesh, &bvh, heights, Tiling::default(), &settings)
            .expect("any closed mesh can be tiled");
        bounds(&pressed.mesh)
    }

    fn raised(settings: ReliefSettings) -> ReliefSettings {
        ReliefSettings {
            amplitude_mm: 0.4,
            ..settings
        }
    }

    #[test]
    fn an_inverted_texture_raises_what_was_black() {
        let grown = tiled_cube(
            &flat(0.0),
            raised(ReliefSettings {
                invert: true,
                ..ReliefSettings::default()
            }),
        );
        assert!(
            (grown.maxs.z - 10.4).abs() < TOLERANCE_MM,
            "got {grown:?}, expected black read as white to grow the cube 0.4 mm"
        );
    }

    #[test]
    fn a_symmetric_relief_leaves_mid_grey_where_it_was_and_moves_black_in() {
        let symmetric = raised(ReliefSettings {
            symmetric: true,
            ..ReliefSettings::default()
        });
        let grey = tiled_cube(&flat(0.5), symmetric);
        let black = tiled_cube(&flat(0.0), symmetric);
        assert!(
            (grey.maxs.z - 10.0).abs() < TOLERANCE_MM && (black.maxs.z - 9.8).abs() < TOLERANCE_MM,
            "grey topped out at {} and black at {}, expected 10.0 and half of 0.4 mm in",
            grey.maxs.z,
            black.maxs.z
        );
    }

    /// The cube with its lid widened to 14 mm, so that every side leans out over the
    /// 10 mm bottom by about 11 degrees.
    fn leaning_out() -> Mesh {
        let (cube, _) = cube(10.0);
        let vertices = cube
            .vertices
            .iter()
            .map(|corner| {
                if corner.z > 0.0 {
                    Vec3::new(corner.x * 1.4 - 2.0, corner.y * 1.4 - 2.0, corner.z)
                } else {
                    *corner
                }
            })
            .collect();
        Mesh::new(vertices, cube.faces)
    }

    #[test]
    fn a_held_bottom_stays_flat_where_leaning_sides_meet_it() {
        let mesh = leaning_out();
        let bvh = Bvh::build(&mesh);
        let settings = raised(ReliefSettings {
            mask: AngleMask {
                bottom_deg: 5.0,
                ..AngleMask::default()
            },
            ..ReliefSettings::default()
        });
        let pressed = press_tiled(&mesh, &bvh, &flat(1.0), Tiling::default(), &settings)
            .expect("any closed mesh can be tiled");
        let grown = bounds(&pressed.mesh);
        // A side leaning out faces partly down, so without the clip at the held face's
        // plane its relief hangs up to 0.4 × sin 11° ≈ 0.08 mm below the bottom edge.
        assert!(
            grown.mins.z.abs() < TOLERANCE_MM / 2.0 && grown.maxs.x > 12.2,
            "got {grown:?}, expected the bottom at 0 and the sides moved out"
        );
    }

    #[test]
    fn a_held_top_is_measured_from_the_up_it_is_given() {
        let grown = tiled_cube(
            &flat(1.0),
            raised(ReliefSettings {
                mask: AngleMask {
                    up: Vec3::X,
                    top_deg: 5.0,
                    ..AngleMask::default()
                },
                ..ReliefSettings::default()
            }),
        );
        assert!(
            (grown.maxs.x - 10.0).abs() < TOLERANCE_MM
                && (grown.maxs.z - 10.4).abs() < TOLERANCE_MM,
            "got {grown:?}, expected the +X face held and the lid moved"
        );
    }

    #[test]
    fn a_mask_past_level_or_with_no_up_is_refused() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let with = |mask: AngleMask| {
            let settings = ReliefSettings {
                mask,
                ..ReliefSettings::default()
            };
            press_tiled(&mesh, &bvh, &flat(1.0), Tiling::default(), &settings)
        };
        assert!(matches!(
            with(AngleMask {
                bottom_deg: 95.0,
                ..AngleMask::default()
            }),
            Err(VolumeError::BadMaskAngle(_))
        ));
        assert!(matches!(
            with(AngleMask {
                top_deg: Scalar::NAN,
                ..AngleMask::default()
            }),
            Err(VolumeError::BadMaskAngle(_))
        ));
        assert!(matches!(
            with(AngleMask {
                up: Vec3::ZERO,
                ..AngleMask::default()
            }),
            Err(VolumeError::BadMaskUp(_))
        ));
    }

    #[test]
    fn a_negative_smoothing_is_refused() {
        let (mesh, uvs) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let settings = ReliefSettings {
            smoothing_px: -1.0,
            ..ReliefSettings::default()
        };
        assert!(matches!(
            press(&mesh, &bvh, &uvs, &[flat(1.0)], &settings),
            Err(VolumeError::BadSmoothing(_))
        ));
    }

    #[test]
    fn a_prepared_texture_is_smoothed_then_turned_over() {
        let settings = ReliefSettings {
            invert: true,
            smoothing_px: 4.0,
            ..ReliefSettings::default()
        };
        // Smoothed far past its four pixels, the half-black half-white strip is all
        // mid grey, which turned over is mid grey again.
        let strip = half();
        let read = prepared(&strip, &settings);
        assert!(
            read.samples()
                .iter()
                .all(|height| (height - 0.5).abs() < 0.05),
            "got {:?}",
            read.samples()
        );
        assert!(matches!(
            prepared(&strip, &ReliefSettings::default()),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn a_face_buried_in_another_shell_takes_no_relief() {
        // Two 10 mm cubes pushed half into each other, so that each has a side inside
        // the other: the solid is the 15 × 10 × 10 box they span.
        let mut mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(10.0));
        mesh.append(&core_geometry::cuboid(
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::new(15.0, 10.0, 10.0),
        ));
        let bvh = Bvh::build(&mesh);
        let settings = raised(ReliefSettings {
            symmetric: true,
            ..ReliefSettings::default()
        });
        let pressed = press_tiled(&mesh, &bvh, &flat(0.0), Tiling::default(), &settings)
            .expect("two closed cubes can be tiled");
        let depth = |p: Vec3| {
            (p - Vec3::ZERO)
                .min(Vec3::new(15.0, 10.0, 10.0) - p)
                .min_element()
        };
        let deepest = pressed
            .mesh
            .vertices
            .iter()
            .copied()
            .map(depth)
            .fold(Scalar::MIN, Scalar::max);
        // Black read symmetrically sinks the box's own sides by half the 0.4 mm amplitude.
        assert!(
            (deepest - 0.2).abs() < TOLERANCE_MM,
            "the deepest vertex stands {deepest} mm inside, expected only the sides sunk 0.2 mm"
        );
    }

    #[test]
    fn a_kept_basis_presses_another_picture_into_the_same_model_as_a_fresh_one_would() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let settings = raised(ReliefSettings::default());
        let tiled = |image: &Heightmap, basis| {
            press_tiled_reusing(
                &mesh,
                &bvh,
                image,
                Tiling::default(),
                &settings,
                basis,
                NEVER,
            )
            .expect("any closed mesh can be tiled")
        };
        let (_, basis) = tiled(&flat(1.0), None);
        let (reused, again) = tiled(&half(), Some(Arc::clone(&basis)));
        assert!(
            Arc::ptr_eq(&basis, &again),
            "the same model and lattice keep the basis"
        );
        let (fresh, _) = tiled(&half(), None);
        assert_eq!(reused.mesh, fresh.mesh);
    }

    #[test]
    fn a_basis_kept_for_another_depth_or_model_is_built_again() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let (_, basis) = press_tiled_reusing(
            &mesh,
            &bvh,
            &flat(1.0),
            Tiling::default(),
            &raised(ReliefSettings::default()),
            None,
            NEVER,
        )
        .expect("any closed mesh can be tiled");

        let deeper = ReliefSettings {
            amplitude_mm: 0.6,
            ..ReliefSettings::default()
        };
        let (_, rebuilt) = press_tiled_reusing(
            &mesh,
            &bvh,
            &flat(1.0),
            Tiling::default(),
            &deeper,
            Some(Arc::clone(&basis)),
            NEVER,
        )
        .expect("any closed mesh can be tiled");
        assert!(!Arc::ptr_eq(&basis, &rebuilt));

        let (larger, _) = cube(12.0);
        let (_, other) = press_tiled_reusing(
            &larger,
            &Bvh::build(&larger),
            &flat(1.0),
            Tiling::default(),
            &raised(ReliefSettings::default()),
            Some(Arc::clone(&basis)),
            NEVER,
        )
        .expect("any closed mesh can be tiled");
        assert!(!Arc::ptr_eq(&basis, &other));
    }

    #[test]
    fn a_press_given_up_on_is_cancelled_and_keeps_no_basis() {
        let (mesh, uvs) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let settings = raised(ReliefSettings::default());
        let stopped = || true;
        let pressed = press_reusing(
            &mesh,
            &bvh,
            &uvs,
            &[flat(1.0)],
            &settings,
            None,
            Cancel::when(&stopped),
        );
        assert!(matches!(pressed, Err(VolumeError::Cancelled)));

        let tiled = press_tiled_reusing(
            &mesh,
            &bvh,
            &flat(1.0),
            Tiling::default(),
            &settings,
            None,
            Cancel::when(&stopped),
        );
        assert!(matches!(tiled, Err(VolumeError::Cancelled)));
    }
}
