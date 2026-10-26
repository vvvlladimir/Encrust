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
    let (nearest, placed) = nearest_in_block(faces, grid, block, work, found);

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

    let linked = one_sided.is_none() && signer.is_costly();
    let sides = linked.then(|| sides(mesh, signer, reach_mm, found, &placed));

    let mut crosses = false;
    for (index, (slot, point, on_face)) in found.iter().enumerate() {
        let inside = one_sided
            .or_else(|| sides.map(|sides| sides[index]))
            .unwrap_or_else(|| signer.is_inside(mesh, *point, on_face));
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

const BLOCK_VOXELS: usize = (SIDED * SIDED * SIDED) as usize;
const UNPLACED: u8 = u8::MAX;

/// Fills `found` with the block's voxels a carried face reaches, and says the nearest
/// distance and where in `found` each voxel of the block went.
fn nearest_in_block(
    faces: &Faces,
    grid: VoxelGrid,
    block: IVec3,
    work: &[Nearest],
    found: &mut Vec<(usize, Vec3, ClosestPoint)>,
) -> (Scalar, [u8; BLOCK_VOXELS]) {
    found.clear();
    let mut nearest = Scalar::INFINITY;
    let mut placed = [UNPLACED; BLOCK_VOXELS];
    for local in cube_voxels(SIDED) {
        let voxel = block + local;
        let slot = index_in_tile(voxel);
        let point = grid.position(voxel);
        let Some(on_face) = faces.closest(work[slot], point) else {
            continue;
        };
        nearest = nearest.min(on_face.distance);
        placed[index_in(SIDED, local)] = found.len() as u8;
        found.push((slot, point, on_face));
    }
    (nearest, placed)
}

/// The side of each of `found`, asked once per group of voxels no surface can part: two
/// neighbours `spacing_mm` apart share a side when either stands further than that off it.
fn sides(
    mesh: &Mesh,
    signer: &Signer,
    spacing_mm: Scalar,
    found: &[(usize, Vec3, ClosestPoint)],
    placed: &[u8; BLOCK_VOXELS],
) -> [bool; BLOCK_VOXELS] {
    let mut parent: [u8; BLOCK_VOXELS] = std::array::from_fn(|index| index as u8);
    for local in cube_voxels(SIDED) {
        let here = placed[index_in(SIDED, local)];
        if here == UNPLACED {
            continue;
        }
        for step in [IVec3::X, IVec3::Y, IVec3::Z] {
            let next = local + step;
            if next.max_element() >= SIDED {
                continue;
            }
            let there = placed[index_in(SIDED, next)];
            let apart = |at: u8| found[at as usize].2.distance > spacing_mm;
            if there != UNPLACED && (apart(here) || apart(there)) {
                let (one, other) = (root(&mut parent, here), root(&mut parent, there));
                parent[one as usize] = other;
            }
        }
    }

    // The side is asked where the group stands furthest off the mesh, where it is surest.
    let mut asked: [u8; BLOCK_VOXELS] = std::array::from_fn(|index| index as u8);
    for index in 0..found.len() as u8 {
        let group = root(&mut parent, index) as usize;
        if found[index as usize].2.distance > found[asked[group] as usize].2.distance {
            asked[group] = index;
        }
    }
    let mut answer = [None; BLOCK_VOXELS];
    let mut inside = [false; BLOCK_VOXELS];
    for (index, side) in inside.iter_mut().enumerate().take(found.len()) {
        let group = root(&mut parent, index as u8) as usize;
        *side = *answer[group].get_or_insert_with(|| {
            let (_, point, on_face) = &found[asked[group] as usize];
            signer.is_inside(mesh, *point, on_face)
        });
    }
    inside
}

fn root(parent: &mut [u8; BLOCK_VOXELS], mut index: u8) -> u8 {
    while parent[index as usize] != index {
        let up = parent[parent[index as usize] as usize];
        parent[index as usize] = up;
        index = up;
    }
    index
}

/// Every voxel of a tile, as an offset from its lowest corner.
pub(super) fn tile_voxels() -> impl Iterator<Item = IVec3> {
    cube_voxels(TILE)
}

/// Where a block voxel sits in the block's own values.
pub(super) fn at(local: IVec3) -> usize {
    index_in(SIDE, local)
}

fn cube_voxels(side: i32) -> impl Iterator<Item = IVec3> {
    (0..side)
        .flat_map(move |z| (0..side).flat_map(move |y| (0..side).map(move |x| IVec3::new(x, y, z))))
}

fn index_in(side: i32, local: IVec3) -> usize {
    ((local.z * side + local.y) * side + local.x) as usize
}
