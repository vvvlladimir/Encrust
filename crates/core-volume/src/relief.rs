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
mod tests;
