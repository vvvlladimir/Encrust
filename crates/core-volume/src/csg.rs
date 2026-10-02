use std::collections::{HashMap, HashSet};

use core_geometry::{Scalar, glam::IVec3};
use rayon::prelude::*;

use crate::error::VolumeError;
use crate::grid::{TILE, VoxelGrid, index_in_tile};
use crate::sdf::{Quantised, Sdf, TILE_VALUES, encode, gap_runs};

/// Everything either field holds.
pub fn union(left: &Sdf, right: &Sdf) -> Result<Sdf, VolumeError> {
    combine(left, right, Scalar::min)
}

/// Only what both fields hold.
pub fn intersection(left: &Sdf, right: &Sdf) -> Result<Sdf, VolumeError> {
    combine(left, right, Scalar::max)
}

/// `left` with `right` taken out of it.
pub fn difference(left: &Sdf, right: &Sdf) -> Result<Sdf, VolumeError> {
    combine(left, right, |near, far| near.max(-far))
}

/// Moves the surface by `distance_mm`: outward when positive, inward when negative.
///
/// The band cannot be moved further than it reaches, so the result's band is narrower by
/// what was asked of it.
pub fn offset(field: &Sdf, distance_mm: Scalar) -> Result<Sdf, VolumeError> {
    let band_mm = shrunk_band(field, distance_mm.abs())?;
    Ok(rebuild(field, band_mm, |value| value - distance_mm))
}

/// Hollows the field out, leaving a wall `thickness_mm` thick inside its surface.
pub fn shell(field: &Sdf, thickness_mm: Scalar) -> Result<Sdf, VolumeError> {
    let band_mm = shrunk_band(field, thickness_mm)?;
    Ok(rebuild(field, band_mm, |value| {
        value.max(-(value + thickness_mm))
    }))
}

/// Applies `op` to two fields on the same lattice, tile by tile.
fn combine(
    left: &Sdf,
    right: &Sdf,
    op: impl Fn(Scalar, Scalar) -> Scalar + Sync,
) -> Result<Sdf, VolumeError> {
    let grid = left.grid();
    if grid != right.grid() {
        return Err(VolumeError::GridMismatch {
            left_voxel_mm: grid.voxel_mm,
            right_voxel_mm: right.grid().voxel_mm,
        });
    }

    let band_mm = left.band_mm().min(right.band_mm());
    let mut candidates: HashSet<IVec3> = left.tile_keys().collect();
    candidates.extend(right.tile_keys());

    Ok(assemble(grid, band_mm, candidates, |voxel| {
        op(left.value(voxel), right.value(voxel))
    }))
}

/// Applies `op` to every value of one field, keeping its lattice and its tiles.
fn rebuild(field: &Sdf, band_mm: Scalar, op: impl Fn(Scalar) -> Scalar + Sync) -> Sdf {
    assemble(
        field.grid(),
        band_mm,
        field.tile_keys().collect(),
        |voxel| op(field.value(voxel)),
    )
}

/// Builds a field out of a function of the lattice, over the tiles worth asking about.
///
/// Tiles the resulting surface misses are dropped and answered by the runs instead, which
/// is what keeps a field derived from another one as sparse as the one it came from.
pub(crate) fn assemble(
    grid: VoxelGrid,
    band_mm: Scalar,
    candidates: HashSet<IVec3>,
    at: impl Fn(IVec3) -> Scalar + Sync,
) -> Sdf {
    let clamped = |voxel: IVec3| at(voxel).clamp(-band_mm, band_mm);
    let tiles: HashMap<IVec3, Box<[Quantised]>> = candidates
        .into_par_iter()
        .filter_map(|tile| fill_tile(tile, band_mm, &clamped).map(|values| (tile, values)))
        .collect();

    let solid = gap_runs(tiles.keys().copied(), |tile| clamped(tile * TILE) < 0.0);
    Sdf::new(grid, band_mm, tiles, solid)
}

/// Fills one tile, or answers `None` when the surface misses it.
fn fill_tile(
    tile: IVec3,
    band_mm: Scalar,
    at: &(impl Fn(IVec3) -> Scalar + Sync),
) -> Option<Box<[Quantised]>> {
    let mut values = vec![Quantised::MAX; TILE_VALUES];
    let mut crosses = false;
    let base = tile * TILE;

    for z in 0..TILE {
        for y in 0..TILE {
            for x in 0..TILE {
                let voxel = base + IVec3::new(x, y, z);
                let value = at(voxel);
                crosses |= value.abs() < band_mm;
                values[index_in_tile(voxel)] = encode(value, band_mm);
            }
        }
    }

    crosses.then(|| values.into_boxed_slice())
}

/// What the band is left with after the surface has been moved by `asked_mm`.
fn shrunk_band(field: &Sdf, asked_mm: Scalar) -> Result<Scalar, VolumeError> {
    let band_mm = field.band_mm() - asked_mm;
    if band_mm <= 0.0 {
        return Err(VolumeError::OutsideBand {
            asked_mm,
            band_mm: field.band_mm(),
        });
    }
    Ok(band_mm)
}
