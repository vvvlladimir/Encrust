use std::collections::HashSet;

use core_geometry::{Scalar, glam::IVec3};
use rayon::prelude::*;

use super::{InfillSettings, Lattice, lattice_of};
use crate::build::TILE_COST_BYTES;
use crate::csg::assemble;
use crate::error::VolumeError;
use crate::grid::TILE;
use crate::sdf::Sdf;

/// The cavity with the lattice standing in it, as one field whose surface is the whole of
/// the void: the way a lattice is cut when the cavity's mesh is not a solid the exact cut
/// can read.
///
/// The field fills the cavity's volume rather than following its surface, so it is priced
/// against `budget_bytes` before a tile of it is filled.
pub(super) fn filled(
    cavity: &Sdf,
    settings: &InfillSettings,
    budget_bytes: usize,
) -> Result<Sdf, VolumeError> {
    let Some(floor_mm) = floor_mm(cavity) else {
        return Ok(cavity.clone());
    };
    let lattice = lattice_of(settings, floor_mm);
    let candidates = candidates(cavity, lattice.as_ref());
    priced(cavity, candidates.len(), budget_bytes)?;

    let grid = cavity.grid();
    Ok(assemble(grid, cavity.band_mm(), candidates, |voxel| {
        cavity
            .value(voxel)
            .max(-lattice.distance(grid.position(voxel)))
    }))
}

/// Refuses a field of `tiles` over `cavity` that `budget_bytes` cannot hold, naming the
/// lattice that would: the tiles fill a volume, so their count goes as its cube.
fn priced(cavity: &Sdf, tiles: usize, budget_bytes: usize) -> Result<(), VolumeError> {
    let needed_bytes = (cavity.tile_count() + tiles) * TILE_COST_BYTES;
    if budget_bytes == 0 || needed_bytes <= budget_bytes {
        return Ok(());
    }
    let voxel_mm = cavity.grid().voxel_mm;
    Err(VolumeError::TooFine {
        needed_bytes,
        budget_bytes,
        voxel_mm,
        fits_at_mm: voxel_mm * (needed_bytes as Scalar / budget_bytes as Scalar).cbrt(),
    })
}

/// The cavity's own tiles, and those inside it the lattice's surface comes near.
///
/// A tile whose centre is further from the lattice than the tile reaches cannot hold a
/// surface the cavity's own did not, so it is left to the runs.
fn candidates(cavity: &Sdf, lattice: &dyn Lattice) -> HashSet<IVec3> {
    let grid = cavity.grid();
    let reach_mm = grid.tile_radius() + cavity.band_mm();
    let inside: Vec<IVec3> = cavity.inside_tiles().collect();
    let mut candidates: HashSet<IVec3> = inside
        .into_par_iter()
        .filter(|tile| lattice.distance(grid.tile_center(*tile)).abs() <= reach_mm)
        .collect();
    candidates.extend(cavity.tile_keys());
    candidates
}

/// The lowest lattice point inside the cavity, in millimetres.
fn floor_mm(cavity: &Sdf) -> Option<Scalar> {
    let grid = cavity.grid();
    let tiles: Vec<IVec3> = cavity.tile_keys().collect();
    let lowest = tiles
        .into_par_iter()
        .filter_map(|tile| {
            let base = tile * TILE;
            (0..TILE).find_map(|z| {
                let mut level = (0..TILE).flat_map(|y| (0..TILE).map(move |x| IVec3::new(x, y, z)));
                level
                    .any(|step| cavity.value(base + step) < 0.0)
                    .then_some(base.z + z)
            })
        })
        .min()?;
    Some(grid.position(IVec3::new(0, 0, lowest)).z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::VoxelGrid;
    use crate::infill::InfillPattern;
    use crate::infill::tests::ball_cavity;

    #[test]
    fn the_floor_of_a_ball_cavity_is_its_lowest_point() {
        let floor = floor_mm(&ball_cavity(10.0)).expect("the ball has a floor");
        assert!(
            (floor + 10.0).abs() <= 0.5,
            "a 10 mm ball's floor is at -10 to within a voxel, got {floor}"
        );
    }

    #[test]
    fn every_pattern_leaves_less_void_than_the_empty_cavity() {
        let cavity = ball_cavity(12.0);
        let empty = cavity.inside_tiles().count() + cavity.tile_count();
        for pattern in InfillPattern::ALL {
            let settings = InfillSettings {
                pattern,
                ..InfillSettings::default()
            };
            let filled = filled(&cavity, &settings, 0).expect("no ceiling");
            assert!(
                filled.inside_tiles().count() < empty,
                "{} leaves the cavity as it was",
                pattern.label()
            );
        }
    }

    #[test]
    fn an_empty_cavity_holds_no_lattice() {
        let empty = assemble(VoxelGrid::new(0.5), 1.5, HashSet::new(), |_| 1.0);
        let filled = filled(&empty, &InfillSettings::default(), 0).expect("no ceiling");
        assert!(filled.is_empty());
    }

    #[test]
    fn a_lattice_field_past_the_budget_names_the_lattice_that_fits() {
        let cavity = ball_cavity(12.0);
        let refused = filled(&cavity, &InfillSettings::default(), 1 << 20);
        let Err(VolumeError::TooFine { fits_at_mm, .. }) = refused else {
            panic!("a megabyte holds no lattice in a 24 mm ball, got {refused:?}");
        };
        assert!(fits_at_mm > cavity.grid().voxel_mm);
    }
}
