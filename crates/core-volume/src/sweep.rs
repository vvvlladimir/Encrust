use std::collections::HashMap;

use core_geometry::{Bvh, ClosestPoint, FastMap, FastSet, Mesh, Scalar, Vec3, glam::IVec3};
use rayon::prelude::*;

use crate::cancel::Cancel;
use crate::grid::{TILE, VoxelGrid, index_in_tile, tile_of};
use crate::scatter::{Faces, Nearest, UNSET};
use crate::sdf::TILE_VALUES;
use crate::sign::Signer;

/// What a tile stores for each voxel, from its signed distance to the mesh, its position
/// and its nearest point; and what a voxel the band never reached holds.
pub(crate) type Record<'a, T> = (&'a (dyn Fn(Scalar, Vec3, &ClosestPoint) -> T + Sync), T);

/// Voxels along one side of the block a tile is swept in: its own, plus the one row of
/// neighbours a step can come from.
const SIDE: i32 = TILE + 2;
const BLOCK_VALUES: usize = (SIDE * SIDE * SIDE) as usize;

/// How much coarser the lattice that carries a nearest face across the wall is.
///
/// The stored band stands a whole wall off the mesh, so something has to cross that wall;
/// crossing it on the stored lattice means paying for the wall's volume at the band's
/// resolution, which is most of a build. Two is eight times less of it, and near enough
/// that the block a stored tile refines still holds a hundred faces to choose between.
/// See `docs/decisions/0082-scatter-the-field-and-carry-it-coarse.md`.
const CARRY: i32 = 2;

/// The coarser carrier a band standing far enough off the mesh can take.
const FAR_CARRY: i32 = 4;

/// How much coarser than `grid` the carrier of a band `band_mm` wide around the surface
/// `iso_mm` off the mesh is: [`FAR_CARRY`] or [`CARRY`], see `docs/design/volume.md`.
pub(crate) fn carry_for(grid: VoxelGrid, band_mm: Scalar, iso_mm: Scalar) -> i32 {
    let trusted_mm = TRUST_VOXELS * FAR_CARRY as Scalar * grid.voxel_mm;
    if trusted_mm < iso_mm.abs() - band_mm {
        FAR_CARRY
    } else {
        CARRY
    }
}

/// What one tile of the sweep carries: the nearest face of each of its voxels.
pub(crate) type Work = Box<[Nearest]>;

/// One tile out of a ring, or nothing when the sweep had not reached anything around it.
type Carried = (IVec3, Option<Work>);

/// The lattice a field of `grid` carries its nearest faces across the wall on, `carry`
/// times coarser.
pub(crate) fn carrier_grid(grid: VoxelGrid, carry: i32) -> VoxelGrid {
    VoxelGrid::new(grid.voxel_mm * carry as Scalar)
}

/// Carries the seed out to the isosurface on the coarse lattice, then refines the tiles
/// the band crosses on the stored one.
///
/// Nothing between the mesh and the band is ever measured at the stored resolution: the
/// carrier hands each stored tile a block of candidate faces, and eight passes over that
/// block pick the nearest of them for every voxel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill<T: Copy + Send + Sync>(
    mesh: &Mesh,
    faces: &Faces,
    bvh: &Bvh,
    signer: &Signer,
    grid: VoxelGrid,
    band_mm: Scalar,
    iso_mm: Scalar,
    rings: Vec<Vec<IVec3>>,
    wanted: &FastMap<IVec3, Scalar>,
    seeds: &FastMap<IVec3, Work>,
    record: Record<'_, T>,
    cancel: Cancel<'_>,
) -> HashMap<IVec3, Box<[T]>> {
    let factor = carry_for(grid, band_mm, iso_mm);
    let coarse = carrier_grid(grid, factor);
    let carrier = carry(faces, bvh, coarse, coarsened(rings, factor), seeds, cancel);

    // A stored tile reads the carrier and nothing else, so there is no order between them.
    // A run given up on fills no further tile and comes back with what it had; the caller
    // is the one that turns that into an error.
    wanted
        .par_iter()
        .filter(|_| !cancel.asked())
        .filter_map(|(tile, centre_mm)| {
            let work = refine(
                faces,
                bvh,
                (grid, factor),
                &carrier,
                *tile,
                (band_mm, iso_mm),
            );
            let values = tile_values(
                (mesh, faces, bvh),
                signer,
                grid,
                (*tile, *centre_mm),
                &work,
                (band_mm, iso_mm),
                record,
            )?;
            Some((*tile, values))
        })
        .collect()
}

/// How many tiles the carrier will hold for a shell the walk found in `rings`.
///
/// Known before a voxel is filled, which is what lets the budget of ADR 0063 price it.
pub(crate) fn carrier_tiles(rings: &[Vec<IVec3>], carry: i32) -> usize {
    let mut seen: FastSet<IVec3> = FastSet::default();
    for ring in rings {
        seen.extend(ring.iter().map(|tile| tile.div_euclid(IVec3::splat(carry))));
    }
    seen.len()
}

/// The walk's rings on the carrier's own tiles, each in the earliest ring a tile it covers
/// appeared in.
fn coarsened(rings: Vec<Vec<IVec3>>, carry: i32) -> Vec<Vec<IVec3>> {
    let mut seen: FastSet<IVec3> = FastSet::default();
    rings
        .into_iter()
        .map(|ring| {
            let mut coarse: Vec<IVec3> = ring
                .into_iter()
                .map(|tile| tile.div_euclid(IVec3::splat(carry)))
                .filter(|tile| seen.insert(*tile))
                .collect();
            // Vertex numbering downstream follows the order tiles come out in, and a hash
            // set's order is not the same twice (ADR 0066).
            coarse.sort_unstable_by_key(|tile| (tile.z, tile.y, tile.x));
            coarse
        })
        .collect()
}

/// The coarse field of nearest faces over the whole shell, ring by ring, so that a tile is
/// only swept once the tiles between it and the surface are.
///
/// Only the two rings a ring can read from are kept as halo, so the shell is only ever
/// resident as the carrier itself, which is the carry factor cubed smaller than it looks.
fn carry(
    faces: &Faces,
    bvh: &Bvh,
    coarse: VoxelGrid,
    rings: Vec<Vec<IVec3>>,
    seeds: &FastMap<IVec3, Work>,
    cancel: Cancel<'_>,
) -> FastMap<IVec3, Work> {
    let mut carrier: FastMap<IVec3, Work> = FastMap::default();
    let mut previous: FastMap<IVec3, Work> = FastMap::default();
    let mut current: FastMap<IVec3, Work> = FastMap::default();
    let mut deferred: Vec<IVec3> = Vec::new();

    for mut ring in rings {
        if cancel.asked() {
            return carrier;
        }
        ring.append(&mut deferred);
        let swept: Vec<Carried> = ring
            .par_iter()
            .map(|tile| {
                let look = |at: IVec3| {
                    current
                        .get(&at)
                        .or_else(|| previous.get(&at))
                        .or_else(|| seeds.get(&at))
                        .map(|work| &work[..])
                };
                (*tile, swept_tile(faces, bvh, coarse, *tile, &look))
            })
            .collect();

        previous = std::mem::take(&mut current);
        for (tile, work) in swept {
            let Some(work) = work else {
                deferred.push(tile);
                continue;
            };
            carrier.insert(tile, work.clone());
            current.insert(tile, work);
        }
    }

    for _ in 1..PASSES {
        if cancel.asked() {
            return carrier;
        }
        settle(faces, bvh, coarse, &mut carrier, seeds);
    }
    carrier
}

/// How many times the shell is swept over.
///
/// The rings carry a face outward, but a tile only ever sees the faces already in its own
/// block, so the nearest face of a deep voxel can be stranded in a tile of the same ring
/// that had not been swept yet. A second pass, with every tile now readable, is what lets
/// it across; a third moves nothing on a ball or a bust.
const PASSES: usize = 2;

/// One more sweep of every tile of the shell, this time against the whole of it.
fn settle(
    faces: &Faces,
    bvh: &Bvh,
    coarse: VoxelGrid,
    carrier: &mut FastMap<IVec3, Work>,
    seeds: &FastMap<IVec3, Work>,
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
            (*tile, resettled(faces, bvh, coarse, *tile, &look))
        })
        .collect();

    for (tile, work) in swept {
        if let Some(work) = work {
            carrier.insert(tile, work);
        }
    }
}

/// One carrier tile swept again, or `None` when nothing around it could move a voxel.
fn resettled<'a>(
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
fn offers_nearer(faces: &Faces, grid: VoxelGrid, base: IVec3, near: &[Nearest]) -> bool {
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
fn swept_tile<'a>(
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
const TRUST_VOXELS: Scalar = 3.0;

/// One stored tile's nearest faces, refined from the carrier at its own resolution.
///
/// A voxel takes the nearest of the faces the eight carrier points around it hold, and no
/// march is needed because the tile arrives already full. The `carry` cubed voxels of one
/// carrier cell share those eight points, so the faces are named once per cell rather than
/// once per voxel, and there are only ever two or three distinct ones.
fn refine(
    faces: &Faces,
    bvh: &Bvh,
    (grid, carry): (VoxelGrid, i32),
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
            work[index_in_tile(voxel)] = if nearest < trust_mm && matters(nearest) {
                // Only a face nearer than the one carried can replace it, which bounds
                // the search from its first node.
                bvh.closest_within(point, nearest)
                    .map_or(carried, |found| found.face as Nearest)
            } else {
                carried
            };
        }
    }
    work.into_boxed_slice()
}

/// Every carrier cell of a tile, as an offset in carrier voxels from its lowest.
fn cells(carry: i32) -> impl Iterator<Item = IVec3> {
    let side = TILE / carry;
    (0..side)
        .flat_map(move |z| (0..side).flat_map(move |y| (0..side).map(move |x| IVec3::new(x, y, z))))
}

/// Every voxel of one carrier cell, as an offset from its lowest.
fn cell_voxels(carry: i32) -> impl Iterator<Item = IVec3> {
    (0..carry).flat_map(move |z| {
        (0..carry).flat_map(move |y| (0..carry).map(move |x| IVec3::new(x, y, z)))
    })
}

/// Marches one block over and reads its own tile back out of it, asking the hierarchy for
/// whatever the march never reached.
fn swept_block(
    faces: &Faces,
    bvh: &Bvh,
    grid: VoxelGrid,
    base: IVec3,
    mut near: Vec<Nearest>,
) -> Work {
    let mut best = vec![Scalar::INFINITY; BLOCK_VALUES];
    measure(faces, grid, base, &near, &mut best);

    for octant in 0..8 {
        let step = IVec3::new(
            1 - 2 * (octant & 1),
            1 - 2 * ((octant >> 1) & 1),
            1 - 2 * ((octant >> 2) & 1),
        );
        march(faces, grid, base, step, &mut near, &mut best);
    }

    let mut work = vec![UNSET; TILE_VALUES];
    for local in tile_voxels() {
        let carried = near[at(local + IVec3::ONE)];
        work[index_in_tile(base + local)] = if carried == UNSET {
            searched(bvh, grid.position(base + local))
        } else {
            carried
        };
    }
    work.into_boxed_slice()
}

/// Copies what the twenty-seven tiles around a block already carry into it.
fn gather<'a>(
    near: &mut [Nearest],
    base: IVec3,
    look: &impl Fn(IVec3) -> Option<&'a [Nearest]>,
    tile: IVec3,
) {
    for offset in 0..27 {
        let side = IVec3::new(offset % 3 - 1, (offset / 3) % 3 - 1, offset / 9 - 1);
        let Some(source) = look(tile + side) else {
            continue;
        };
        let (low, high) = block_span(side);
        for z in low.z..=high.z {
            for y in low.y..=high.y {
                for x in low.x..=high.x {
                    let local = IVec3::new(x, y, z);
                    near[at(local)] = source[index_in_tile(base + local - IVec3::ONE)];
                }
            }
        }
    }
}

/// Which corner of a block the neighbour at `side` supplies, both ends inclusive.
fn block_span(side: IVec3) -> (IVec3, IVec3) {
    let along = |side: i32| match side {
        -1 => (0, 0),
        0 => (1, TILE),
        _ => (SIDE - 1, SIDE - 1),
    };
    let (x, y, z) = (along(side.x), along(side.y), along(side.z));
    (IVec3::new(x.0, y.0, z.0), IVec3::new(x.1, y.1, z.1))
}

/// The distance from every block voxel to the face it came in carrying.
fn measure(faces: &Faces, grid: VoxelGrid, base: IVec3, near: &[Nearest], best: &mut [Scalar]) {
    for z in 0..SIDE {
        for y in 0..SIDE {
            for x in 0..SIDE {
                let local = IVec3::new(x, y, z);
                let slot = at(local);
                if near[slot] == UNSET {
                    continue;
                }
                let point = grid.position(base + local - IVec3::ONE);
                best[slot] = faces
                    .distance(near[slot], point)
                    .unwrap_or(Scalar::INFINITY);
            }
        }
    }
}

/// One of the eight passes: every voxel takes whichever face carried by the seven voxels
/// upstream of it stands nearest to it, which is Zhao's sweep carrying a nearest face
/// rather than a value. Seven and not three, or a face waiting in a corner of the block
/// could never step into it.
fn march(
    faces: &Faces,
    grid: VoxelGrid,
    base: IVec3,
    step: IVec3,
    near: &mut [Nearest],
    best: &mut [Scalar],
) {
    let along = |sign: i32, index: i32| if sign > 0 { 1 + index } else { TILE - index };

    for third in 0..TILE {
        for second in 0..TILE {
            for first in 0..TILE {
                let local = IVec3::new(
                    along(step.x, first),
                    along(step.y, second),
                    along(step.z, third),
                );
                let slot = at(local);
                let point = grid.position(base + local - IVec3::ONE);
                for corner in 1..8 {
                    let mut back = IVec3::ZERO;
                    for axis in 0..3 {
                        if corner & (1 << axis) != 0 {
                            back[axis] = -step[axis];
                        }
                    }
                    let carried = near[at(local + back)];
                    // A neighbour standing on the face this voxel already carries has
                    // nothing to say, and past the first pass most of them do.
                    if carried == near[slot] {
                        continue;
                    }
                    let Some(distance) = faces.distance(carried, point) else {
                        continue;
                    };
                    if distance < best[slot] {
                        best[slot] = distance;
                        near[slot] = carried;
                    }
                }
            }
        }
    }
}

/// The nearest face of a voxel the sweep never reached, found the slow way.
///
/// A voxel with neither a carrier nor a swept neighbour is the sweep's own blind spot: it
/// costs a traversal rather than a wrong answer, and `hollow-lab` is where it shows.
fn searched(bvh: &Bvh, point: Vec3) -> Nearest {
    bvh.closest(point)
        .map_or(UNSET, |found| found.face as Nearest)
}

/// One tile's stored values, or `None` when the band misses every voxel of it.
///
/// `centre_mm` is the signed distance at the tile's centre. No surface is nearer the
/// centre than that, so past the tile's own radius it settles the side of every voxel.
#[allow(clippy::too_many_arguments)]
fn tile_values<T: Copy>(
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
const SIDED: i32 = 4;

/// What a lattice value is written as, from its signed distance, its point and the nearest
/// point of the mesh.
type Written<'a, T> = &'a dyn Fn(Scalar, Vec3, &ClosestPoint) -> T;

/// Writes one block of a tile and says whether the band crossed it.
#[allow(clippy::too_many_arguments)]
fn block_values<T>(
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
fn tile_voxels() -> impl Iterator<Item = IVec3> {
    (0..TILE).flat_map(|z| (0..TILE).flat_map(move |y| (0..TILE).map(move |x| IVec3::new(x, y, z))))
}

/// Where a block voxel sits in the block's own values.
fn at(local: IVec3) -> usize {
    ((local.z * SIDE + local.y) * SIDE + local.x) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdf::{Quantised, encode, step_mm};
    use crate::sign::SignMode;

    /// A plate ten millimetres square and a tenth thick, wound outward; its first two
    /// faces are its underside.
    fn plate() -> Mesh {
        let (s, t) = (10.0, 0.1);
        let corners = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(s, 0.0, 0.0),
            Vec3::new(s, s, 0.0),
            Vec3::new(0.0, s, 0.0),
            Vec3::new(0.0, 0.0, t),
            Vec3::new(s, 0.0, t),
            Vec3::new(s, s, t),
            Vec3::new(0.0, s, t),
        ];
        Mesh::new(
            corners.to_vec(),
            vec![
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
            ],
        )
    }

    #[test]
    fn a_block_carried_the_far_side_of_a_thin_wall_is_still_signed_by_the_near_side() {
        let mesh = plate();
        let faces = Faces::of(&mesh);
        let bvh = Bvh::build(&mesh);
        let signer = Signer::new(&mesh, SignMode::Pseudonormal);
        let grid = VoxelGrid::new(0.2);
        let (band_mm, iso_mm) = (0.4, 3.0);
        // The tile from 3.2 to 4.6 mm over the plate, every voxel of it handed the
        // underside: near enough in distance, and facing the other way.
        let underside = vec![0; TILE_VALUES].into_boxed_slice();

        let values = tile_values(
            (&mesh, &faces, &bvh),
            &signer,
            grid,
            (IVec3::new(2, 2, 2), 0.0),
            &underside,
            (band_mm, iso_mm),
            (
                &|signed, _, _| encode(signed - iso_mm, band_mm),
                Quantised::MAX,
            ),
        )
        .expect("the 3 mm offset crosses the tile");

        let lowest = Scalar::from(values[0]) * step_mm(band_mm);
        assert!(
            lowest > 0.0,
            "3.2 mm up is over the plate and outside its 3 mm offset, got {lowest}"
        );
    }
}
