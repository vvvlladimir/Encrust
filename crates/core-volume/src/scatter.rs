use core_geometry::{
    ClosestPoint, FastMap, Mesh, Scalar, Triangle, Vec3, glam::IVec3, point_triangle,
};
use rayon::prelude::*;

use crate::grid::{TILE, VoxelGrid, index_in_tile, tile_of, within};
use crate::sdf::TILE_VALUES;

/// How far past the mesh the exact seed reaches, in voxels.
///
/// The seed has to be a shell no path can cross unseen, since it is what gives the sweep
/// its side: one and a half voxels is past the half-diagonal of the cube the surface cuts.
pub(crate) const SEED_VOXELS: Scalar = 1.5;

/// The face nearest a voxel.
///
/// Neither the distance nor the side is carried with it. The distance is a
/// `point_triangle` away, and a shell of a hundred million voxels cannot afford four more
/// bytes of each; the side cannot be carried at all, because both sides of a face are
/// upstream of some voxel and the sweep has no way to tell one from the other. It is
/// worked out from the face where the values are written, and nowhere else.
pub(crate) type Nearest = u32;

/// A voxel no face has been found for yet.
pub(crate) const UNSET: Nearest = u32::MAX;

/// The mesh's faces as triangles, so that reading one is a line of cache rather than
/// three lookups into a vertex list the size of the model.
///
/// The sweep asks for a face's corners billions of times over, in an order that follows
/// the lattice and not the mesh; indexing the vertices there is most of a build.
pub(crate) struct Faces(Vec<Triangle>);

impl Faces {
    pub(crate) fn of(mesh: &Mesh) -> Self {
        let flat = Triangle::new(Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
        Self(
            (0..mesh.faces.len())
                .into_par_iter()
                .map(|face| mesh.triangle(face).unwrap_or(flat))
                .collect(),
        )
    }

    pub(crate) fn get(&self, face: usize) -> Option<&Triangle> {
        self.0.get(face)
    }

    /// How far `point` is from the face `carried` names, or `None` when it names none.
    pub(crate) fn distance(&self, carried: Nearest, point: Vec3) -> Option<Scalar> {
        Some(self.closest(carried, point)?.distance)
    }

    /// The nearest point of the face `carried` names, or `None` when it names none.
    pub(crate) fn closest(&self, carried: Nearest, point: Vec3) -> Option<ClosestPoint> {
        if carried == UNSET {
            return None;
        }
        let triangle = self.0.get(carried as usize)?;
        let on_face = point_triangle(point, triangle);
        Some(ClosestPoint {
            point: on_face,
            distance: (on_face - point).length(),
            face: carried as usize,
        })
    }
}

/// The face nearest every voxel within [`SEED_VOXELS`] of the mesh, tile by tile.
///
/// Scattered from the faces rather than gathered per voxel: a face is measured against the
/// voxels its own bounds reach and no others, so the work follows the surface rather than
/// the volume around it. See `docs/design/volume.md`.
pub(crate) fn seeds(
    mesh: &Mesh,
    faces: &Faces,
    grid: VoxelGrid,
    keep: Option<(IVec3, IVec3)>,
) -> FastMap<IVec3, Box<[Nearest]>> {
    let reach_mm = SEED_VOXELS * grid.voxel_mm;
    let mut pairs = touched(mesh, grid, reach_mm, keep);
    pairs.par_sort_unstable_by_key(|(tile, _)| (tile.z, tile.y, tile.x));

    let groups: Vec<&[(IVec3, u32)]> = pairs.chunk_by(|left, right| left.0 == right.0).collect();
    groups
        .into_par_iter()
        .map(|group| (group[0].0, seed_tile(faces, grid, reach_mm, group)))
        .collect()
}

/// Every (tile, face) pair a face's bounds grown by `reach_mm` come to.
fn touched(
    mesh: &Mesh,
    grid: VoxelGrid,
    reach_mm: Scalar,
    keep: Option<(IVec3, IVec3)>,
) -> Vec<(IVec3, u32)> {
    let reach = Vec3::splat(reach_mm);
    (0..mesh.faces.len())
        .into_par_iter()
        .fold(Vec::new, |mut pairs, face| {
            let Some(triangle) = mesh.triangle(face) else {
                return pairs;
            };
            let low = triangle.a.min(triangle.b).min(triangle.c) - reach;
            let high = triangle.a.max(triangle.b).max(triangle.c) + reach;
            let (first, last) = (tile_of(grid.voxel(low)), tile_of(grid.voxel(high)));

            for z in first.z..=last.z {
                for y in first.y..=last.y {
                    for x in first.x..=last.x {
                        let tile = IVec3::new(x, y, z);
                        if keep.is_none_or(|(low, high)| within(tile, low, high)) {
                            pairs.push((tile, face as u32));
                        }
                    }
                }
            }
            pairs
        })
        .reduce(Vec::new, |mut pairs, mut other| {
            pairs.append(&mut other);
            pairs
        })
}

/// One tile's seed, from the faces whose bounds reach into it.
fn seed_tile(
    faces: &Faces,
    grid: VoxelGrid,
    reach_mm: Scalar,
    group: &[(IVec3, u32)],
) -> Box<[Nearest]> {
    let base = group[0].0 * TILE;
    let mut nearest = vec![UNSET; TILE_VALUES];
    let mut best = vec![reach_mm * reach_mm; TILE_VALUES];

    for (_, face) in group {
        let Some(triangle) = faces.get(*face as usize) else {
            continue;
        };
        let reach = Vec3::splat(reach_mm);
        let low = grid
            .voxel(triangle.a.min(triangle.b).min(triangle.c) - reach)
            .max(base);
        let high = grid
            .voxel(triangle.a.max(triangle.b).max(triangle.c) + reach)
            .min(base + IVec3::splat(TILE - 1));

        for z in low.z..=high.z {
            for y in low.y..=high.y {
                for x in low.x..=high.x {
                    let voxel = IVec3::new(x, y, z);
                    let point = grid.position(voxel);
                    let found = point_triangle(point, triangle);
                    let squared = (found - point).length_squared();
                    let slot = index_in_tile(voxel);
                    // Past the reach the list is not complete, so the winner there would
                    // not be the nearest face and the sweep would carry it outward.
                    if squared < best[slot] {
                        best[slot] = squared;
                        nearest[slot] = *face;
                    }
                }
            }
        }
    }

    nearest.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Bvh;

    /// A unit cube at the origin, wound outward.
    fn cube() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(4.0, 0.0, 0.0),
                Vec3::new(4.0, 4.0, 0.0),
                Vec3::new(0.0, 4.0, 0.0),
                Vec3::new(0.0, 0.0, 4.0),
                Vec3::new(4.0, 0.0, 4.0),
                Vec3::new(4.0, 4.0, 4.0),
                Vec3::new(0.0, 4.0, 4.0),
            ],
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

    /// Every seeded voxel carries the face a full search would have given it, and the
    /// distance to that face is inside the seed's own reach.
    #[test]
    fn a_seeded_voxel_carries_its_true_nearest_face() {
        let mesh = cube();
        let bvh = Bvh::build(&mesh);
        let grid = VoxelGrid::new(0.25);
        let reach_mm = SEED_VOXELS * grid.voxel_mm;
        let seeded = seeds(&mesh, &Faces::of(&mesh), grid, None);

        let mut checked = 0;
        for (tile, values) in &seeded {
            for z in 0..TILE {
                for y in 0..TILE {
                    for x in 0..TILE {
                        let voxel = tile * TILE + IVec3::new(x, y, z);
                        let slot = index_in_tile(voxel);
                        if values[slot] == UNSET {
                            continue;
                        }
                        let point = grid.position(voxel);
                        let exact = bvh.closest(&mesh, point).expect("the cube has faces");
                        let triangle = mesh
                            .triangle(values[slot] as usize)
                            .expect("the seed named a face of the mesh");
                        let carried = (point_triangle(point, &triangle) - point).length();
                        assert!(
                            carried <= exact.distance + 1e-4,
                            "seed at {voxel} is {carried} mm off, the nearest face is {} mm",
                            exact.distance
                        );
                        assert!(
                            carried <= reach_mm + 1e-4,
                            "seed at {voxel} is past its reach"
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 1_000, "only {checked} voxels were seeded");
    }
}
