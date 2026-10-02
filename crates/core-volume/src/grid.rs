use core_geometry::{Scalar, Vec3, glam::IVec3};

/// Voxels along one side of a tile.
///
/// Eight is 512 values, two kilobytes of `f32`: small enough that a tile the band only
/// clips costs little, large enough that the map holding them stays short.
pub const TILE: i32 = 8;

/// How coarse the voxel lattice is. Voxel `(0, 0, 0)` is the origin of the space the mesh
/// is in, so two fields of the same spacing share a lattice whatever they are fields of,
/// and can be combined without resampling either.
///
/// A voxel is a lattice point, not a box: `position` gives the point a value belongs to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelGrid {
    /// Lattice spacing in millimetres.
    pub voxel_mm: Scalar,
}

impl VoxelGrid {
    pub fn new(voxel_mm: Scalar) -> Self {
        Self { voxel_mm }
    }

    pub fn position(&self, voxel: IVec3) -> Vec3 {
        voxel.as_vec3() * self.voxel_mm
    }

    /// Lattice point at or below `point` on every axis.
    ///
    /// The nudge is a ten-thousandth of a voxel, which puts a point that landed a rounding
    /// error below its own lattice point back on it without moving any real boundary.
    pub fn voxel(&self, point: Vec3) -> IVec3 {
        let local = point / self.voxel_mm + Vec3::splat(1e-4);
        IVec3::new(
            local.x.floor() as i32,
            local.y.floor() as i32,
            local.z.floor() as i32,
        )
    }

    /// Half the diagonal of one tile, which is how far a tile's furthest voxel can be from
    /// its centre.
    pub fn tile_radius(&self) -> Scalar {
        (TILE - 1) as Scalar * self.voxel_mm * Scalar::sqrt(3.0) / 2.0
    }

    pub fn tile_center(&self, tile: IVec3) -> Vec3 {
        self.position(tile * TILE) + Vec3::splat((TILE - 1) as Scalar * self.voxel_mm / 2.0)
    }
}

/// Tile a voxel belongs to. Negative coordinates round down, so tiles tile space evenly.
pub fn tile_of(voxel: IVec3) -> IVec3 {
    IVec3::new(
        voxel.x.div_euclid(TILE),
        voxel.y.div_euclid(TILE),
        voxel.z.div_euclid(TILE),
    )
}

/// Whether `tile` lies inside the inclusive box from `low` to `high`.
pub fn within(tile: IVec3, low: IVec3, high: IVec3) -> bool {
    tile.cmpge(low).all() && tile.cmple(high).all()
}

/// Index of `voxel` inside its own tile's 512 values, X fastest.
pub fn index_in_tile(voxel: IVec3) -> usize {
    let local = IVec3::new(
        voxel.x.rem_euclid(TILE),
        voxel.y.rem_euclid(TILE),
        voxel.z.rem_euclid(TILE),
    );
    ((local.z * TILE + local.y) * TILE + local.x) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_lands_back_on_its_own_voxel() {
        let grid = VoxelGrid::new(0.1);
        let voxel = IVec3::new(3, -7, 11);
        assert_eq!(grid.voxel(grid.position(voxel)), voxel);
    }

    #[test]
    fn tiles_below_the_origin_round_down() {
        assert_eq!(tile_of(IVec3::new(-1, 0, 7)), IVec3::new(-1, 0, 0));
        assert_eq!(tile_of(IVec3::new(-8, 8, 15)), IVec3::new(-1, 1, 1));
    }

    #[test]
    fn every_voxel_of_a_tile_has_its_own_index() {
        let mut seen = vec![false; 512];
        for z in 0..TILE {
            for y in 0..TILE {
                for x in 0..TILE {
                    let index = index_in_tile(IVec3::new(x - TILE, y, z + TILE));
                    assert!(!seen[index], "index {index} came up twice");
                    seen[index] = true;
                }
            }
        }
        assert!(seen.into_iter().all(|hit| hit));
    }
}
