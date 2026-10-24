use super::values::{at, tile_voxels};
use super::{BLOCK_VALUES, Carried, SIDE, Work, cell_voxels, cells, gather, swept_block};
use crate::cancel::Cancel;
use crate::grid::{TILE, VoxelGrid, index_in_tile, tile_of};
use crate::scatter::{Faces, Nearest, UNSET};
use crate::sdf::TILE_VALUES;
use core_geometry::{Bvh, FastMap, Scalar, glam::IVec3};
use rayon::prelude::*;

/// How many times the shell is swept over.
///
/// The rings carry a face outward, but a tile only ever sees the faces already in its own
/// block, so the nearest face of a deep voxel can be stranded in a tile of the same ring
/// that had not been swept yet. A second pass, with every tile now readable, is what lets
/// it across; a third moves nothing on a ball or a bust.
pub(super) const PASSES: usize = 2;

/// One more sweep of every tile of the shell, this time against the whole of it.
pub(super) fn settle(
    faces: &Faces,
    bvh: &Bvh,
    coarse: VoxelGrid,
    carrier: &mut FastMap<IVec3, Work>,
    seeds: &FastMap<IVec3, Work>,
    cancel: Cancel<'_>,
) {
    let tiles: Vec<IVec3> = carrier.keys().copied().collect();
    let swept: Vec<Carried> = tiles
        .par_iter()
        .map(|tile| {
            let look = |at: IVec3| {
                carrier
                    .get(&at)
                    .or_else(|| seeds.get(&at))
                    .map(|work| &work[..])
            };
            let swept = resettled(faces, bvh, coarse, *tile, &look);
            cancel.advance(1);
            (*tile, swept)
        })
        .collect();

    for (tile, work) in swept {
        if let Some(work) = work {
            carrier.insert(tile, work);
        }
    }
}

/// One carrier tile swept again, or `None` when nothing around it could move a voxel.
pub(super) fn resettled<'a>(
    faces: &Faces,
    bvh: &Bvh,
    grid: VoxelGrid,
    tile: IVec3,
    look: &impl Fn(IVec3) -> Option<&'a [Nearest]>,
) -> Option<Work> {
    let base = tile * TILE;
    let mut near = vec![UNSET; BLOCK_VALUES];
    gather(&mut near, base, look, tile);
    offers_nearer(faces, grid, base, &near).then(|| swept_block(faces, bvh, grid, base, near))
}

/// Whether the halo of a block offers a voxel on the border of its tile a nearer face
/// than the one it holds: the only way a face can enter the tile, so without one a sweep
/// over it moves nothing.
pub(super) fn offers_nearer(faces: &Faces, grid: VoxelGrid, base: IVec3, near: &[Nearest]) -> bool {
    let in_halo = |local: IVec3| local.min_element() == 0 || local.max_element() == SIDE - 1;
    let on_border = |local: IVec3| local.min_element() == 1 || local.max_element() == TILE;

    for local in tile_voxels().map(|voxel| voxel + IVec3::ONE) {
        if !on_border(local) {
            continue;
        }
        let own = near[at(local)];
        let point = grid.position(base + local - IVec3::ONE);
        let mut held: Option<Scalar> = None;
        for offset in 0..27 {
            let side = IVec3::new(offset % 3 - 1, (offset / 3) % 3 - 1, offset / 9 - 1);
            let beside = local + side;
            if !in_halo(beside) {
                continue;
            }
            let carried = near[at(beside)];
            if carried == UNSET || carried == own {
                continue;
            }
            let held =
                *held.get_or_insert_with(|| faces.distance(own, point).unwrap_or(Scalar::INFINITY));
            if faces
                .distance(carried, point)
                .is_some_and(|distance| distance < held)
            {
                return true;
            }
        }
    }
    false
}

/// One carrier tile swept from whatever its neighbours already carry, or `None` when
/// nothing around it carries anything yet.
pub(super) fn swept_tile<'a>(
    faces: &Faces,
    bvh: &Bvh,
    grid: VoxelGrid,
    tile: IVec3,
    look: &impl Fn(IVec3) -> Option<&'a [Nearest]>,
) -> Option<Work> {
    let base = tile * TILE;
    let mut near = vec![UNSET; BLOCK_VALUES];
    gather(&mut near, base, look, tile);
    if near.iter().all(|carried| *carried == UNSET) {
        return None;
    }
    Some(swept_block(faces, bvh, grid, base, near))
}

/// How far from the mesh the carrier is believed, in its own voxels.
///
/// The eight carrier points around a voxel hold the faces nearest *them*, and the nearest
/// face of the voxel itself can be a carrier step to one side of all of them. That miss
/// costs about `step² / 2d` in distance, which is nothing a wall away from the mesh and
/// everything at it — so under this the hierarchy is asked instead. See
/// `docs/decisions/0082-scatter-the-field-and-carry-it-coarse.md`.
pub(super) const TRUST_VOXELS: Scalar = 3.0;

/// One stored tile's nearest faces, refined from the carrier at its own resolution.
///
/// A voxel takes the nearest of the faces the eight carrier points around it hold, and no
/// march is needed because the tile arrives already full. The `carry` cubed voxels of one
/// carrier cell share those eight points, so the faces are named once per cell rather than
/// once per voxel, and there are only ever two or three distinct ones.
pub(super) fn refine(
    faces: &Faces,
    bvh: &Bvh,
    (grid, carry, shared): (VoxelGrid, i32, bool),
    carrier: &FastMap<IVec3, Work>,
    tile: IVec3,
    band: (Scalar, Scalar),
) -> Work {
    let (band_mm, iso_mm) = band;
    let trust_mm = TRUST_VOXELS * carry as Scalar * grid.voxel_mm;
    // Past the band the value is the band's own whatever the distance was, so a voxel
    // there is never worth a traversal however far off the carrier might be.
    let told_mm = band_mm + 2.0 * grid.voxel_mm;
    let matters = |distance: Scalar| (distance - iso_mm.abs()).abs() < told_mm;
    let base = tile * TILE;
    let mut work = vec![UNSET; TILE_VALUES];
    let mut held: Option<(IVec3, &Work)> = None;
    let mut offered: Vec<Nearest> = Vec::with_capacity(8);
    let mut doubted: Vec<Doubted> = Vec::with_capacity((carry * carry * carry) as usize);
    let mut nearby: Vec<u32> = Vec::new();

    for cell in cells(carry) {
        let low = base.div_euclid(IVec3::splat(carry)) + cell;
        offered.clear();
        for corner in 0..8 {
            let at = low + IVec3::new(corner & 1, (corner >> 1) & 1, (corner >> 2) & 1);
            let source = tile_of(at);
            if held.is_none_or(|(tile, _)| tile != source) {
                held = carrier.get(&source).map(|work| (source, work));
            }
            if let Some((_, held)) = held {
                let carried = held[index_in_tile(at)];
                if carried != UNSET && !offered.contains(&carried) {
                    offered.push(carried);
                }
            }
        }

        doubted.clear();
        for step in cell_voxels(carry) {
            let voxel = base + cell * carry + step;
            let point = grid.position(voxel);
            let (mut nearest, mut carried) = (Scalar::INFINITY, UNSET);
            for face in &offered {
                let Some(distance) = faces.distance(*face, point) else {
                    continue;
                };
                if distance < nearest {
                    nearest = distance;
                    carried = *face;
                }
            }
            work[index_in_tile(voxel)] = carried;
            if nearest < trust_mm && matters(nearest) {
                doubted.push(Doubted {
                    voxel,
                    nearest,
                    carried,
                });
            }
        }
        if shared {
            settle_doubted(faces, bvh, grid, &doubted, &mut nearby, &mut work);
        } else {
            settle_each(bvh, grid, &doubted, &mut work);
        }
    }
    work.into_boxed_slice()
}

/// Most faces one walk for a whole carrier cell may bring back before each voxel is
/// better off with a walk of its own; tuned on a thin wall and on a relief.
pub(super) const SHARED_FACES: usize = 96;

/// Smallest mean face, in voxels of area, for which one walk is shared by a carrier cell:
/// a finer mesh brings back more faces than the voxels' own walks would open.
pub(super) const SHARED_AREA_VOXELS: Scalar = 2.0;

/// A voxel too near the mesh for the face its carrier points offered to be trusted.
pub(super) struct Doubted {
    pub(super) voxel: IVec3,
    /// How far the offered face is, which no nearer face can be beyond, mm.
    pub(super) nearest: Scalar,
    pub(super) carried: Nearest,
}

/// The true nearest face of each doubted voxel of one carrier cell, from one walk of the
/// hierarchy for the whole cell rather than one per voxel.
///
/// Every face nearer a voxel than its offered one lies within that distance of the voxel,
/// so within it plus the voxel's own distance from the cell's middle; the walk takes every
/// leaf that near, and the voxels choose among its faces.
pub(super) fn settle_doubted(
    faces: &Faces,
    bvh: &Bvh,
    grid: VoxelGrid,
    doubted: &[Doubted],
    nearby: &mut Vec<u32>,
    work: &mut [Nearest],
) {
    let Some(first) = doubted.first() else {
        return;
    };
    let (low, high) = doubted
        .iter()
        .fold((first.voxel, first.voxel), |(low, high), at| {
            (low.min(at.voxel), high.max(at.voxel))
        });
    let middle = (grid.position(low) + grid.position(high)) * 0.5;
    let spread = (grid.position(high) - grid.position(low)).length() * 0.5;
    let reach = doubted.iter().map(|at| at.nearest).fold(0.0, Scalar::max) + spread;
    bvh.faces_within(middle, reach, nearby);
    if nearby.len() > SHARED_FACES {
        settle_each(bvh, grid, doubted, work);
        return;
    }

    for at in doubted {
        let point = grid.position(at.voxel);
        // Only a face nearer than the one carried can replace it, measured squared as the
        // hierarchy's own search does.
        let mut best = (at.nearest * at.nearest, at.carried);
        for face in nearby.iter() {
            if let Some(found) = faces.closest(*face, point) {
                let squared = (found.point - point).length_squared();
                if squared < best.0 {
                    best = (squared, *face);
                }
            }
        }
        work[index_in_tile(at.voxel)] = best.1;
    }
}

/// The true nearest face of each doubted voxel, from a walk of the hierarchy of its own.
pub(super) fn settle_each(bvh: &Bvh, grid: VoxelGrid, doubted: &[Doubted], work: &mut [Nearest]) {
    for at in doubted {
        // Only a face nearer than the one carried can replace it, which bounds the search
        // from its first node.
        work[index_in_tile(at.voxel)] = bvh
            .closest_within(grid.position(at.voxel), at.nearest)
            .map_or(at.carried, |found| found.face as Nearest);
    }
}
