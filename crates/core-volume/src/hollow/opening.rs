//! What of a model is thick enough to hollow: the solid opened by a ball (ADR 0404).

use std::collections::HashMap;

use core_geometry::{Bvh, Mesh, Scalar};

use crate::build::{FieldSettings, build};
use crate::cancel::Cancel;
use crate::csg::intersection;
use crate::error::VolumeError;
use crate::extract::extract;
use crate::grid::VoxelGrid;
use crate::sdf::Sdf;

use super::{BAND_VOXELS, HollowSettings};

/// The field of `surface` opened by a ball as wide as `settings.solid_under_mm`, on a lattice
/// of `voxel_mm`: every part thinner than that gone from it. `None` where nothing is asked.
pub(super) fn opened(
    surface: &Mesh,
    bvh: &Bvh,
    settings: &HollowSettings,
    voxel_mm: Scalar,
    cancel: Cancel<'_>,
) -> Result<Option<Sdf>, VolumeError> {
    let Some(under_mm) = settings.solid_under_mm else {
        return Ok(None);
    };
    if !under_mm.is_finite() || under_mm <= 0.0 {
        return Err(VolumeError::BadSolidUnder(under_mm));
    }
    let radius_mm = under_mm / 2.0;
    let at = |iso_mm| FieldSettings {
        voxel_mm,
        band_voxels: BAND_VOXELS,
        iso_mm,
        sign: settings.sign,
        clip: None,
        budget_bytes: settings.budget_bytes,
    };
    // A band reaches two voxels, so each half is a band of its own around its surface.
    let eroded = extract(&build(surface, bvh, &at(-radius_mm), cancel)?, cancel);
    if cancel.asked() {
        return Err(VolumeError::Cancelled);
    }
    if eroded.faces.is_empty() {
        return Ok(Some(nothing(voxel_mm)));
    }
    let eroded_bvh = Bvh::build(&eroded);
    build(&eroded, &eroded_bvh, &at(radius_mm), cancel).map(Some)
}

/// `field` kept only inside `opened`, where there is one.
pub(super) fn kept_inside(field: Sdf, opened: Option<&Sdf>) -> Result<Sdf, VolumeError> {
    match opened {
        Some(opened) => intersection(&field, opened),
        None => Ok(field),
    }
}

/// A field with nothing inside it, on a lattice of `voxel_mm`.
fn nothing(voxel_mm: Scalar) -> Sdf {
    let band_mm = BAND_VOXELS * voxel_mm;
    Sdf::new(
        VoxelGrid::new(voxel_mm),
        band_mm,
        HashMap::new(),
        HashMap::new(),
    )
}

#[cfg(test)]
mod tests;
