use std::collections::{HashMap, HashSet};

use core_geometry::{Bvh, ClosestPoint, FastMap, Mesh, Scalar, Vec3, glam::IVec3};
use rayon::prelude::*;

use crate::cancel::Cancel;
use crate::grid::{TILE, VoxelGrid, index_in_tile, tile_of};
use crate::scatter::{Faces, Nearest, UNSET};
use crate::sdf::{Quantised, TILE_VALUES, encode};
use crate::sign::Signer;

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

/// What one tile of the sweep carries: the nearest face of each of its voxels.
pub(crate) type Work = Box<[Nearest]>;

/// One tile out of a ring, or nothing when the sweep had not reached anything around it.
type Carried = (IVec3, Option<Work>);

/// The lattice a field of `grid` carries its nearest faces across the wall on.
pub(crate) fn carrier_grid(grid: VoxelGrid) -> VoxelGrid {
    VoxelGrid::new(grid.voxel_mm * CARRY as Scalar)
}

/// Carries the seed out to the isosurface on the coarse lattice, then refines the tiles
/// the band crosses on the stored one.
///
/// Nothing between the mesh and the band is ever measured at the stored resolution: the
/// carrier hands each stored tile a block of candidate faces, and eight passes over that
/// block pick the nearest of them for every voxel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill(
    mesh: &Mesh,
    faces: &Faces,
    bvh: &Bvh,
    signer: &Signer,
    grid: VoxelGrid,
    band_mm: Scalar,
    iso_mm: Scalar,
    rings: Vec<Vec<IVec3>>,
    wanted: &HashSet<IVec3>,
    seeds: &FastMap<IVec3, Work>,
    cancel: Cancel<'_>,
) -> HashMap<IVec3, Box<[Quantised]>> {
    let coarse = carrier_grid(grid);
    let carrier = carry(mesh, faces, bvh, coarse, coarsened(rings), seeds, cancel);

    // A stored tile reads the carrier and nothing else, so there is no order between them.
    // A run given up on fills no further tile and comes back with what it had; the caller
    // is the one that turns that into an error.
    wanted
        .par_iter()
        .filter(|_| !cancel.asked())
        .filter_map(|tile| {
            let work = refine(mesh, faces, bvh, grid, &carrier, *tile, (band_mm, iso_mm));
            let values = tile_values(
                (mesh, faces, bvh),
                signer,
                grid,
                *tile,
                &work,
                band_mm,
                iso_mm,
            )?;
            Some((*tile, values))
        })
        .collect()
}

/// How many tiles the carrier will hold for a shell the walk found in `rings`.
///
/// Known before a voxel is filled, which is what lets the budget of ADR 0063 price it.
pub(crate) fn carrier_tiles(rings: &[Vec<IVec3>]) -> usize {
    let mut seen: HashSet<IVec3> = HashSet::new();
    for ring in rings {
        seen.extend(ring.iter().map(|tile| tile.div_euclid(IVec3::splat(CARRY))));
    }
    seen.len()
}

/// The walk's rings on the carrier's own tiles, each in the earliest ring a tile it covers
/// appeared in.
fn coarsened(rings: Vec<Vec<IVec3>>) -> Vec<Vec<IVec3>> {
    let mut seen: HashSet<IVec3> = HashSet::new();
    rings
        .into_iter()
        .map(|ring| {
            let mut coarse: Vec<IVec3> = ring
                .into_iter()
                .map(|tile| tile.div_euclid(IVec3::splat(CARRY)))
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
/// resident as the carrier itself, which is `CARRY` cubed smaller than it looks.
#[allow(clippy::too_many_arguments)]
fn carry(
    mesh: &Mesh,
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
                (*tile, swept_tile(mesh, faces, bvh, coarse, *tile, &look))
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
        settle(mesh, faces, bvh, coarse, &mut carrier, seeds);
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
    mesh: &Mesh,
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
            (*tile, swept_tile(mesh, faces, bvh, coarse, *tile, &look))
        })
        .collect();

    for (tile, work) in swept {
        if let Some(work) = work {
            carrier.insert(tile, work);
        }
    }
}

/// One carrier tile swept from whatever its neighbours already carry, or `None` when
/// nothing around it carries anything yet.
fn swept_tile<'a>(
    mesh: &Mesh,
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
    Some(swept_block(mesh, faces, bvh, grid, base, near))
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
/// march is needed because the tile arrives already full. The `CARRY` cubed voxels of one
/// carrier cell share those eight points, so the faces are named once per cell rather than
/// once per voxel, and there are only ever two or three distinct ones.
#[allow(clippy::too_many_arguments)]
fn refine(
    mesh: &Mesh,
    faces: &Faces,
    bvh: &Bvh,
    grid: VoxelGrid,
    carrier: &FastMap<IVec3, Work>,
    tile: IVec3,
    band: (Scalar, Scalar),
) -> Work {
    let (band_mm, iso_mm) = band;
    let trust_mm = TRUST_VOXELS * CARRY as Scalar * grid.voxel_mm;
    // Past the band the value is the band's own whatever the distance was, so a voxel
    // there is never worth a traversal however far off the carrier might be.
    let told_mm = band_mm + 2.0 * grid.voxel_mm;
    let matters = |distance: Scalar| (distance - iso_mm.abs()).abs() < told_mm;
    let base = tile * TILE;
    let mut work = vec![UNSET; TILE_VALUES];
    let mut held: Option<(IVec3, &Work)> = None;
    let mut offered: Vec<Nearest> = Vec::with_capacity(8);

    for cell in cells() {
        let low = base.div_euclid(IVec3::splat(CARRY)) + cell;
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

        for step in cell_voxels() {
            let voxel = base + cell * CARRY + step;
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
                searched(mesh, bvh, point)
            } else {
                carried
            };
        }
    }
    work.into_boxed_slice()
}

/// Every carrier cell of a tile, as an offset in carrier voxels from its lowest.
fn cells() -> impl Iterator<Item = IVec3> {
    let side = TILE / CARRY;
    (0..side)
        .flat_map(move |z| (0..side).flat_map(move |y| (0..side).map(move |x| IVec3::new(x, y, z))))
}

/// Every voxel of one carrier cell, as an offset from its lowest.
fn cell_voxels() -> impl Iterator<Item = IVec3> {
    (0..CARRY)
        .flat_map(|z| (0..CARRY).flat_map(move |y| (0..CARRY).map(move |x| IVec3::new(x, y, z))))
}

/// Marches one block over and reads its own tile back out of it, asking the hierarchy for
/// whatever the march never reached.
fn swept_block(
    mesh: &Mesh,
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
            searched(mesh, bvh, grid.position(base + local))
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
fn searched(mesh: &Mesh, bvh: &Bvh, point: Vec3) -> Nearest {
    bvh.closest(mesh, point)
        .map_or(UNSET, |found| found.face as Nearest)
}

/// One tile's stored values, or `None` when the band misses every voxel of it.
#[allow(clippy::too_many_arguments)]
fn tile_values(
    surface: (&Mesh, &Faces, &Bvh),
    signer: &Signer,
    grid: VoxelGrid,
    tile: IVec3,
    work: &[Nearest],
    band_mm: Scalar,
    iso_mm: Scalar,
) -> Option<Box<[Quantised]>> {
    let base = tile * TILE;
    let mut values = vec![Quantised::MAX; TILE_VALUES];
    let mut found: Vec<(usize, Vec3, ClosestPoint)> = Vec::with_capacity(64);
    let mut crosses = false;

    for corner in 0..8 {
        let block = base + IVec3::new(corner & 1, (corner >> 1) & 1, (corner >> 2) & 1) * SIDED;
        crosses |= block_values(
            surface,
            signer,
            grid,
            block,
            work,
            (band_mm, iso_mm),
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

/// Writes one block of a tile and says whether the band crossed it.
#[allow(clippy::too_many_arguments)]
fn block_values(
    (mesh, faces, bvh): (&Mesh, &Faces, &Bvh),
    signer: &Signer,
    grid: VoxelGrid,
    block: IVec3,
    work: &[Nearest],
    band: (Scalar, Scalar),
    found: &mut Vec<(usize, Vec3, ClosestPoint)>,
    values: &mut [Quantised],
) -> bool {
    let (band_mm, iso_mm) = band;
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
    let one_sided = (nearest > reach_mm).then(|| {
        let exact = bvh.closest(mesh, *first);
        signer.is_inside(mesh, *first, exact.as_ref().unwrap_or(on_first))
    });

    let mut crosses = false;
    for (slot, point, on_face) in found.iter() {
        let inside = one_sided.unwrap_or_else(|| signer.is_inside(mesh, *point, on_face));
        let value = if inside {
            -on_face.distance
        } else {
            on_face.distance
        } - iso_mm;
        crosses |= value.abs() < band_mm;
        values[*slot] = encode(value, band_mm);
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
    use crate::sdf::step_mm;
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
            IVec3::new(2, 2, 2),
            &underside,
            band_mm,
            iso_mm,
        )
        .expect("the 3 mm offset crosses the tile");

        let lowest = Scalar::from(values[0]) * step_mm(band_mm);
        assert!(
            lowest > 0.0,
            "3.2 mm up is over the plate and outside its 3 mm offset, got {lowest}"
        );
    }
}
