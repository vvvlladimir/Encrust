use super::{Record, SIDE};
use crate::grid::{TILE, VoxelGrid, index_in_tile};
use crate::scatter::{Faces, Nearest};
use crate::sdf::TILE_VALUES;
use crate::sign::Signer;
use core_geometry::{Bvh, ClosestPoint, Mesh, Scalar, Vec3, glam::IVec3};

/// One tile's stored values, or `None` when the band misses every voxel of it.
///
/// `centre_mm` is the signed distance at the tile's centre. No surface is nearer the
/// centre than that, so past the tile's own radius it settles the side of every voxel.
#[allow(clippy::too_many_arguments)]
pub(super) fn tile_values<T: Copy>(
    surface: (&Mesh, &Faces, &Bvh),
    signer: &Signer,
    grid: VoxelGrid,
    (tile, centre_mm): (IVec3, Scalar),
    work: &[Nearest],
    (band_mm, iso_mm): (Scalar, Scalar),
    record: Record<'_, T>,
) -> Option<Box<[T]>> {
    let base = tile * TILE;
    let settled = (centre_mm.abs() > grid.tile_radius()).then_some(centre_mm < 0.0);
    let mut values = vec![record.1; TILE_VALUES];
    let mut found: Vec<(usize, Vec3, ClosestPoint)> = Vec::with_capacity(64);
    let mut crosses = false;

    for corner in 0..8 {
        let block = base + IVec3::new(corner & 1, (corner >> 1) & 1, (corner >> 2) & 1) * SIDED;
        crosses |= block_values(
            surface,
            signer,
            grid,
            (block, settled),
            work,
            (band_mm, iso_mm, record.0),
            &mut found,
            &mut values,
        );
    }

    crosses.then(|| values.into_boxed_slice())
}

/// Voxels along one side of the block a side of the surface is decided over.
///
/// A block no edge of which the surface crosses is all one side, so one question settles
/// all sixty-four. That is what keeps the winding number affordable on a mesh that is not
/// closed.
pub(super) const SIDED: i32 = 4;

/// What a lattice value is written as, from its signed distance, its point and the nearest
/// point of the mesh.
pub(super) type Written<'a, T> = &'a dyn Fn(Scalar, Vec3, &ClosestPoint) -> T;

/// Writes one block of a tile and says whether the band crossed it.
#[allow(clippy::too_many_arguments)]
pub(super) fn block_values<T>(
    (mesh, faces, bvh): (&Mesh, &Faces, &Bvh),
    signer: &Signer,
    grid: VoxelGrid,
    (block, settled): (IVec3, Option<bool>),
    work: &[Nearest],
    band: (Scalar, Scalar, Written<'_, T>),
    found: &mut Vec<(usize, Vec3, ClosestPoint)>,
    values: &mut [T],
) -> bool {
    let (band_mm, iso_mm, record) = band;
    // The surface crossing a lattice edge leaves one of its ends within half a voxel of
    // itself, so nothing nearer than a voxel means nothing crossed and one side for all.
    let reach_mm = grid.voxel_mm;
    found.clear();
    let mut nearest = Scalar::INFINITY;

    for z in 0..SIDED {
        for y in 0..SIDED {
            for x in 0..SIDED {
                let voxel = block + IVec3::new(x, y, z);
                let slot = index_in_tile(voxel);
                let point = grid.position(voxel);
                let Some(on_face) = faces.closest(work[slot], point) else {
                    continue;
                };
                nearest = nearest.min(on_face.distance);
                found.push((slot, point, on_face));
            }
        }
    }

    let Some((_, first, on_first)) = found.first() else {
        return false;
    };
    // The side is asked of the true nearest point, not of the carried face: a face from
    // the far side of a thin wall is near enough for a distance and wrong for a side.
    let one_sided = settled.or_else(|| {
        (nearest > reach_mm).then(|| {
            let exact = bvh.closest(*first);
            signer.is_inside(mesh, *first, exact.as_ref().unwrap_or(on_first))
        })
    });

    let mut crosses = false;
    for (slot, point, on_face) in found.iter() {
        let inside = one_sided.unwrap_or_else(|| signer.is_inside(mesh, *point, on_face));
        let signed = if inside {
            -on_face.distance
        } else {
            on_face.distance
        };
        crosses |= (signed - iso_mm).abs() < band_mm;
        values[*slot] = record(signed, *point, on_face);
    }
    crosses
}

/// Every voxel of a tile, as an offset from its lowest corner.
pub(super) fn tile_voxels() -> impl Iterator<Item = IVec3> {
    (0..TILE).flat_map(|z| (0..TILE).flat_map(move |y| (0..TILE).map(move |x| IVec3::new(x, y, z))))
}

/// Where a block voxel sits in the block's own values.
pub(super) fn at(local: IVec3) -> usize {
    ((local.z * SIDE + local.y) * SIDE + local.x) as usize
}
