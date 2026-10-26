use std::collections::HashMap;

use core_geometry::{Bvh, ClosestPoint, FastMap, FastSet, Mesh, Scalar, Vec3, glam::IVec3};
use rayon::prelude::*;

use crate::cancel::{Cancel, Step};
use crate::grid::{TILE, VoxelGrid, index_in_tile};
use crate::scatter::{Faces, Nearest, UNSET};
use crate::sdf::TILE_VALUES;
use crate::sign::Signer;

mod settle;
mod values;
use settle::{PASSES, SHARED_AREA_VOXELS, TRUST_VOXELS, refine, settle, swept_tile};
use values::{at, tile_values, tile_voxels};

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
    let shared = faces.mean_area_mm2() >= SHARED_AREA_VOXELS * grid.voxel_mm * grid.voxel_mm;
    cancel.begin(Step::Refining, wanted.len());
    wanted
        .par_iter()
        .filter(|_| !cancel.asked())
        .filter_map(|(tile, centre_mm)| {
            cancel.advance(1);
            let work = refine(
                faces,
                bvh,
                (grid, factor, shared),
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

    // Each settling pass goes over every tile the rings swept, so it counts as much again.
    cancel.begin(
        Step::Carrying,
        rings.iter().map(Vec::len).sum::<usize>() * PASSES,
    );
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
                let swept = swept_tile(faces, bvh, coarse, *tile, &look);
                cancel.advance(1);
                (*tile, swept)
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
        settle(faces, bvh, coarse, &mut carrier, seeds, cancel);
    }
    carrier
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

    #[test]
    fn a_field_signed_by_winding_agrees_with_the_winding_number_at_every_voxel() {
        // Two cubes pushed into each other: walls cross inside the solid, and a side
        // carried between voxels must never step through one.
        let shift = Vec3::new(2.5, 1.5, 2.0);
        let mut crossing = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(5.0));
        crossing.append(&core_geometry::cuboid(shift, shift + Vec3::splat(5.0)));
        // A wall thinner than a voxel, whose far side a carried face may hand over.
        let thin = core_geometry::cuboid(Vec3::ZERO, Vec3::new(5.0, 5.0, 0.1));

        for mesh in [crossing, thin] {
            assert_signs_match_winding(&mesh);
        }
    }

    fn assert_signs_match_winding(mesh: &Mesh) {
        let settings = crate::FieldSettings {
            voxel_mm: 0.25,
            sign: SignMode::Winding,
            ..crate::FieldSettings::default()
        };
        let field = crate::build(mesh, &Bvh::build(mesh), &settings, Cancel::never())
            .expect("the body has faces");
        let winding = core_geometry::Winding::build(mesh);

        let grid = field.grid();
        for tile in field.tile_keys() {
            for local in tile_voxels() {
                let voxel = tile * TILE + local;
                let value = field.value(voxel);
                if value.abs() < step_mm(field.band_mm()) {
                    continue;
                }
                let inside = winding.is_inside(mesh, grid.position(voxel));
                assert_eq!(
                    value < 0.0,
                    inside,
                    "voxel {voxel} holds {value} mm, the winding number says inside {inside}"
                );
            }
        }
    }
}
