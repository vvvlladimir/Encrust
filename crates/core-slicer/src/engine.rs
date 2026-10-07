use core_geometry::{Mesh, Scalar};
use rayon::prelude::*;

use crate::bins::ZBins;
use crate::plane::{self, Crossing};
use crate::stitch::{Stitched, stitch};
use crate::{Layer, SliceError, SliceSettings, Sliced};

/// Converts a mesh into an ordered stack of layers, lowest first.
pub trait SliceEngine {
    fn slice(&self, mesh: &Mesh, settings: &SliceSettings) -> Result<Sliced, SliceError>;

    /// Slices exactly the planes in `heights`, which a caller takes from
    /// [`layer_heights`] in chunks to work a window at a time. No heights is an empty
    /// stack, not an error; see
    /// `docs/decisions/0066-a-stack-is-sliced-a-window-at-a-time.md`.
    fn slice_at(&self, mesh: &Mesh, heights: &[Scalar]) -> Result<Sliced, SliceError>;
}

/// Every plane `mesh` will be sliced at, without slicing any of them.
///
/// A caller that wants the stack a window at a time asks for these first, hands chunks of
/// them to [`SliceEngine::slice_at`], and never holds more than one chunk's contours.
pub fn layer_heights(mesh: &Mesh, settings: &SliceSettings) -> Result<Vec<Scalar>, SliceError> {
    layer_heights_under(mesh, settings, Scalar::INFINITY)
}

/// The same, with nothing planned above `ceiling_mm`.
///
/// A plate carries bodies that only subtract, and a cut reaches past the surface it
/// pierces (ADR 0071), so the baked mesh can stand taller than anything that prints. The
/// caller that merged it is the one that knows how far the material goes.
pub fn layer_heights_under(
    mesh: &Mesh,
    settings: &SliceSettings,
    ceiling_mm: Scalar,
) -> Result<Vec<Scalar>, SliceError> {
    if settings.layer_height <= 0.0 {
        return Err(SliceError::NonPositiveLayerHeight(settings.layer_height));
    }
    let (z_min, z_max) = on_the_plate(mesh, ceiling_mm)?;
    Ok(settings.plane_heights(z_min, z_max))
}

/// Height of the plate, plate millimetres. A stack never starts under it: a layer below it
/// would drive the plate into the vat floor.
const PLATE_MM: Scalar = 0.0;

/// The heights of `mesh` a stack covers: from its bottom, or from the plate where it reaches
/// under it, to its top or `ceiling_mm`, whichever is lower. Whatever stands under the
/// plate or over the ceiling is not cut.
pub(crate) fn on_the_plate(
    mesh: &Mesh,
    ceiling_mm: Scalar,
) -> Result<(Scalar, Scalar), SliceError> {
    let aabb = mesh
        .aabb()
        .filter(|_| !mesh.is_empty())
        .ok_or(SliceError::EmptyMesh)?;
    let top_mm = aabb.maxs.z.min(ceiling_mm);
    if top_mm <= PLATE_MM {
        return Err(SliceError::UnderThePlate { top_mm });
    }
    Ok((aabb.mins.z.max(PLATE_MM), top_mm))
}

/// Slices by intersecting every face with each Z plane and stitching the crossings into
/// closed contours. The geometric rules it relies on are in `docs/design/slicing.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlaneSliceEngine;

impl SliceEngine for PlaneSliceEngine {
    fn slice(&self, mesh: &Mesh, settings: &SliceSettings) -> Result<Sliced, SliceError> {
        if settings.layer_height <= 0.0 {
            return Err(SliceError::NonPositiveLayerHeight(settings.layer_height));
        }
        self.slice_at(mesh, &layer_heights(mesh, settings)?)
    }

    fn slice_at(&self, mesh: &Mesh, heights: &[Scalar]) -> Result<Sliced, SliceError> {
        if heights.is_empty() {
            return Ok(Sliced::default());
        }
        if mesh.is_empty() {
            return Err(SliceError::EmptyMesh);
        }

        // The index covers the planes it will be asked about and no more.
        let low = heights.iter().copied().fold(Scalar::INFINITY, Scalar::min);
        let high = heights
            .iter()
            .copied()
            .fold(Scalar::NEG_INFINITY, Scalar::max);
        let bins = ZBins::build(mesh, low, high, heights.len());
        let stitched: Vec<Stitched> = heights
            .par_iter()
            .map(|&z| slice_plane(mesh, &bins, z))
            .collect();

        Ok(collect(heights.to_vec(), stitched))
    }
}

fn slice_plane(mesh: &Mesh, bins: &ZBins, z: Scalar) -> Stitched {
    let crossings: Vec<Crossing> = bins
        .faces_near(z)
        .iter()
        .filter_map(|&face| plane::crossing(mesh, face as usize, z))
        .collect();
    stitch(crossings)
}

fn collect(heights: Vec<Scalar>, stitched: Vec<Stitched>) -> Sliced {
    let mut sliced = Sliced {
        layers: Vec::with_capacity(heights.len()),
        ..Sliced::default()
    };
    for (z, plane) in heights.into_iter().zip(stitched) {
        // One line a layer, at trace, because a mesh with a hole in it has one on every
        // layer the hole reaches: what a reader needs is the count, and that is in the
        // caller's report.
        tracing::trace!(
            z,
            contours = plane.contours.len(),
            open = plane.open,
            "layer sliced"
        );
        sliced.open_contours += plane.open;
        sliced.degenerate_contours += plane.degenerate;
        sliced.unlinked_segments += plane.unlinked;
        sliced.layers.push(Layer::new(z, plane.contours));
    }
    sliced
}

#[cfg(test)]
mod tests {
    use core_geometry::Vec3;

    use super::*;

    fn wedge() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 10.0),
            ],
            vec![[0, 1, 2]],
        )
    }

    #[test]
    fn layer_count_follows_the_mesh_height() {
        let settings = SliceSettings {
            layer_height: 1.0,
            ..SliceSettings::default()
        };
        let sliced = PlaneSliceEngine
            .slice(&wedge(), &settings)
            .expect("sliceable");
        assert_eq!(sliced.layers.len(), 10);
    }

    #[test]
    fn a_lone_face_yields_no_closed_contour() {
        let settings = SliceSettings {
            layer_height: 1.0,
            ..SliceSettings::default()
        };
        let sliced = PlaneSliceEngine
            .slice(&wedge(), &settings)
            .expect("sliceable");
        // One triangle gives one open segment per plane, which encloses no area.
        assert!(sliced.layers.iter().all(Layer::is_empty));
        assert!(!sliced.is_clean());
    }

    #[test]
    fn empty_mesh_is_rejected() {
        let err = PlaneSliceEngine
            .slice(&Mesh::default(), &SliceSettings::default())
            .expect_err("an empty mesh cannot be sliced");
        assert!(matches!(err, SliceError::EmptyMesh));
    }

    #[test]
    fn zero_layer_height_is_rejected() {
        let settings = SliceSettings {
            layer_height: 0.0,
            ..SliceSettings::default()
        };
        let err = PlaneSliceEngine
            .slice(&wedge(), &settings)
            .expect_err("a zero layer height cannot be sliced");
        assert!(matches!(err, SliceError::NonPositiveLayerHeight(_)));
    }
}
