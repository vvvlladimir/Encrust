use std::collections::HashMap;

use core_geometry::{FastMap, Mesh, Scalar, Vec3, glam::IVec3};
use rayon::prelude::*;

use crate::grid::TILE;
use crate::sdf::Sdf;

const CORNERS: usize = 8;
const EDGES: usize = 12;

/// Lattice points along one side of the block a tile needs: its own, plus the one row of
/// neighbours the cells on its far side reach into.
const BLOCK: i32 = TILE + 1;

/// One tile's own surface: the vertices it contributes, which cells stand on each of them,
/// and the quads that join them.
struct TilePart {
    vertices: Vec<Vec3>,
    /// The vertex each of this tile's cells stands on, by [`cell_key`]. Several cells share
    /// one where they were clustered.
    named: Vec<(u64, u32)>,
    /// Four cells around one lattice edge the surface crosses, wound out of the solid.
    quads: Vec<[u64; 4]>,
}

/// Where a cell's vertex stands and which way the surface faces there.
#[derive(Clone, Copy)]
struct Cut {
    at: Vec3,
    normal: Vec3,
}

/// How far a cell's vertex may be pulled from where the field put it, in voxels.
///
/// Half a voxel is what the field itself promises (ADR 0082) and what marching cubes
/// already placed the surface within, so clustering inside this changes nothing that was
/// ever true. It is also what bounds the facet: a patch of `s` across on a curve of radius
/// `R` stands `s²/8R` off its own chord, so a cavity's flat stretches cluster far and its
/// tight ones barely at all.
const CLUSTER_SLACK_VOXELS: Scalar = 0.5;

/// Cells along one side of the largest cluster.
///
/// A tile is walked as its own eight halves, so half a tile is the biggest facet there
/// can be. That is also as large as one wants: the quads joining facets are still the fine
/// lattice's, and a facet much wider than they are leaves them describing very little.
const CLUSTER_MAX_SIDE: i32 = TILE / 2;

/// How much the surface may turn inside one cluster, as a cosine.
///
/// The slack alone would fold the two sides of a thin wall onto each other, since both lie
/// near the same plane. Nothing that turns more than a right angle is one facet.
const CLUSTER_MIN_COSINE: Scalar = 0.0;

/// Surface nets over the band, one tile at a time, merged a layer of tiles at a time.
///
/// One vertex a cell rather than one a cut edge, and one quad for each lattice edge the
/// surface crosses: half again fewer triangles than marching cubes for the same surface,
/// and every vertex is named by its own cell, so tiles meet with no weld to do. Tiles are
/// merged in ascending order of their Z and the table is cut back between layers, so
/// neither the whole field's parts nor a table over the whole surface is ever resident;
/// see `docs/decisions/0084-a-cavity-is-clustered-surface-nets.md` and ADR 0064,
/// whose merge this keeps.
pub fn extract(field: &Sdf) -> Mesh {
    let mut layers: HashMap<i32, Vec<IVec3>> = HashMap::new();
    for tile in field.tile_keys() {
        layers.entry(tile.z).or_default().push(tile);
    }
    let mut levels: Vec<i32> = layers.keys().copied().collect();
    levels.sort_unstable();

    let mut mesh = Mesh::default();
    let mut shared: FastMap<u64, u32> = FastMap::default();
    // Where each triangle was written and which way round, so that a second copy of it
    // can take the first back out.
    let mut written: FastMap<[u32; 3], (usize, bool)> = FastMap::default();
    for level in levels {
        let Some(mut tiles) = layers.remove(&level) else {
            continue;
        };
        // A quad reaches back to cells of lower X, Y and Z, so tiles are merged in that
        // order and every cell a quad names is already in the table. It is also what makes
        // vertex numbering the same twice, which a hash map's order is not (ADR 0066).
        tiles.sort_unstable_by_key(|tile| (tile.x, tile.y));
        let parts: Vec<TilePart> = tiles
            .par_iter()
            .map(|tile| tile_mesh(field, *tile))
            .collect();
        for part in &parts {
            merge(&mut mesh, &mut shared, &mut written, part);
        }
        drop(parts);

        // Only the last row of cells of this layer can still be reached back into, and
        // only a triangle standing on it can be written a second time.
        let floor = (level + 1) * TILE - 1;
        shared.retain(|key, _| key_z(*key) >= floor);
        let kept: std::collections::HashSet<u32> = shared.values().copied().collect();
        written.retain(|face, _| face.iter().all(|corner| kept.contains(corner)));
    }

    mesh.faces.retain(|face| face[0] != face[1]);
    mesh
}

/// Appends one tile's vertices, then its quads as triangles against whatever cell each
/// corner names.
fn merge(
    mesh: &mut Mesh,
    shared: &mut FastMap<u64, u32>,
    written: &mut FastMap<[u32; 3], (usize, bool)>,
    part: &TilePart,
) {
    let first = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&part.vertices);
    for (cell, local) in &part.named {
        shared.insert(*cell, first + local);
    }

    for quad in &part.quads {
        let Some(corners) = quad
            .iter()
            .map(|cell| shared.get(cell).copied())
            .collect::<Option<Vec<u32>>>()
        else {
            continue;
        };
        for triple in [[0, 1, 2], [0, 2, 3]] {
            let face = triple.map(|corner| corners[corner]);
            // A quad whose corners land twice in one cell is a triangle with no area.
            if face[0] == face[1] || face[1] == face[2] || face[0] == face[2] {
                continue;
            }
            // Clustering can fold two quads onto one triangle. Two copies wound the same
            // way are one piece of surface written twice; wound against each other they
            // enclose nothing at all. Either way only edges used four times come of
            // keeping both, and a surface with those cannot be sliced.
            let mut named = face;
            named.sort_unstable();
            let forward = (face[0] < face[1]) == (face[1] < face[2]);
            match written.get(&named).copied() {
                // Already taken back out, or already standing the way this one does.
                Some((CANCELLED, _)) => {}
                Some((_, kept)) if kept == forward => {}
                Some((at, _)) => {
                    mesh.faces[at] = CANCELLED_FACE;
                    written.insert(named, (CANCELLED, forward));
                }
                None => {
                    written.insert(named, (mesh.faces.len(), forward));
                    mesh.faces.push(face);
                }
            }
        }
    }
}

/// A triangle that was written and then taken back out by its own opposite.
const CANCELLED: usize = usize::MAX;
const CANCELLED_FACE: [u32; 3] = [0, 0, 0];

/// The cell whose lowest corner is `voxel`, as one number.
///
/// Twenty bits an axis is a lattice of half a million voxels a side, which is 26 m at the
/// finest spacing hollowing ever asks for.
fn cell_key(voxel: IVec3) -> u64 {
    let place = |value: i32| (i64::from(value) + BIAS) as u64 & FIELD;
    (place(voxel.x) << 40) | (place(voxel.y) << 20) | place(voxel.z)
}

/// The Z of the cell a key names.
fn key_z(key: u64) -> i32 {
    ((key & FIELD).cast_signed() - BIAS) as i32
}

/// Bits one axis of a cell key is given, and what a coordinate is shifted by to fill them
/// unsigned.
const FIELD: u64 = (1 << 20) - 1;
const BIAS: i64 = 1 << 19;

/// The surface the cells whose lowest corner is inside `tile` contribute, as a part of its
/// own.
fn tile_mesh(field: &Sdf, tile: IVec3) -> TilePart {
    let grid = field.grid();
    let base = tile * TILE;

    // One lookup per lattice point rather than one per cell corner, which is eight times
    // fewer for the same numbers.
    let mut block = vec![0.0 as Scalar; (BLOCK * BLOCK * BLOCK) as usize];
    for z in 0..BLOCK {
        for y in 0..BLOCK {
            for x in 0..BLOCK {
                block[((z * BLOCK + y) * BLOCK + x) as usize] =
                    field.value(base + IVec3::new(x, y, z));
            }
        }
    }
    let at = |offset: IVec3| block[((offset.z * BLOCK + offset.y) * BLOCK + offset.x) as usize];

    let mut part = TilePart {
        vertices: Vec::new(),
        named: Vec::new(),
        quads: Vec::new(),
    };
    let mut cuts: Vec<Option<Cut>> = vec![None; (TILE * TILE * TILE) as usize];
    for z in 0..TILE {
        for y in 0..TILE {
            for x in 0..TILE {
                let low = IVec3::new(x, y, z);
                let values: [Scalar; CORNERS] =
                    std::array::from_fn(|corner| at(low + CORNER_OFFSETS[corner]));
                cuts[in_tile(low)] = cell_cut(&values, grid.position(base + low), grid.voxel_mm);
                quads_at(&values, base + low, &mut part.quads);
            }
        }
    }

    let slack_mm = CLUSTER_SLACK_VOXELS * grid.voxel_mm;
    for corner in CORNER_OFFSETS {
        let low = corner * CLUSTER_MAX_SIDE;
        cluster(&cuts, low, CLUSTER_MAX_SIDE, base, slack_mm, &mut part);
    }
    part
}

/// Where a cell sits in one tile's own cells.
fn in_tile(local: IVec3) -> usize {
    ((local.z * TILE + local.y) * TILE + local.x) as usize
}

/// Gives one vertex to every cell of the block at `low` that can share it, and recurses
/// into the eight halves of the block where they cannot.
///
/// A cell's vertex is only ever pulled `slack_mm` from where the field put it, so a flat
/// stretch of cavity comes out as few large facets and a tight one keeps its cells. The
/// blocks are the tile's own halves, quarters and eighths, which never cross into another
/// tile — a tile's lowest cell is a multiple of eight — so no tile has to agree with any
/// other about where a facet ends. See
/// `docs/decisions/0084-a-cavity-is-clustered-surface-nets.md`.
fn cluster(
    cuts: &[Option<Cut>],
    low: IVec3,
    side: i32,
    base: IVec3,
    slack_mm: Scalar,
    part: &mut TilePart,
) {
    let mut members: Vec<(IVec3, Cut)> = Vec::new();
    for z in low.z..low.z + side {
        for y in low.y..low.y + side {
            for x in low.x..low.x + side {
                let at = IVec3::new(x, y, z);
                if let Some(cut) = cuts[in_tile(at)] {
                    members.push((at, cut));
                }
            }
        }
    }
    if members.is_empty() {
        return;
    }

    if side == 1 || one_facet(&members, slack_mm) {
        let index = part.vertices.len() as u32;
        part.vertices.push(facet(&members, slack_mm));
        for (at, _) in &members {
            part.named.push((cell_key(base + *at), index));
        }
        return;
    }

    let half = side / 2;
    for corner in CORNER_OFFSETS {
        cluster(cuts, low + corner * half, half, base, slack_mm, part);
    }
}

/// Where the one vertex of a cluster stands.
///
/// The mean of its cells would sit inside the surface, because the chord of a curve is:
/// clustering a whole cavity that way shrinks it, and a mould, whose cavity *is* its
/// printed outside, came out 3% light. So the mean is pushed back out along the facet's
/// normal by the sagitta the cells' own normals imply — they fan by `spread / radius`, so
/// the radius is what their spread over the cells' own says, and the rise is
/// `radius (1 - |mean normal|)`. On a flat stretch the normals do not fan and nothing
/// moves.
fn facet(members: &[(IVec3, Cut)], slack_mm: Scalar) -> Vec3 {
    let count = members.len() as Scalar;
    let middle = members.iter().map(|(_, cut)| cut.at).sum::<Vec3>() / count;
    let mean = members.iter().map(|(_, cut)| cut.normal).sum::<Vec3>() / count;
    let Some(normal) = mean.try_normalize() else {
        return middle;
    };

    let across = members
        .iter()
        .map(|(_, cut)| (cut.at - middle).length())
        .sum::<Scalar>()
        / count;
    let fan = members
        .iter()
        .map(|(_, cut)| (cut.normal - mean).length())
        .sum::<Scalar>()
        / count;
    if fan <= Scalar::EPSILON {
        return middle;
    }

    let rise = (across / fan) * (1.0 - mean.length());
    middle + normal * rise.clamp(0.0, slack_mm)
}

/// Whether one vertex can stand for every cell of a block: near enough the plane they all
/// share, and all facing the same way.
fn one_facet(members: &[(IVec3, Cut)], slack_mm: Scalar) -> bool {
    let count = members.len() as Scalar;
    let middle = members.iter().map(|(_, cut)| cut.at).sum::<Vec3>() / count;
    let Some(normal) =
        (members.iter().map(|(_, cut)| cut.normal).sum::<Vec3>() / count).try_normalize()
    else {
        return false;
    };

    members.iter().all(|(_, cut)| {
        cut.normal.dot(normal) > CLUSTER_MIN_COSINE
            && (cut.at - middle).dot(normal).abs() <= slack_mm
    })
}

/// Where one cell's vertex stands and which way the surface faces there, or `None` when
/// the surface misses the cell.
///
/// The mean of the surface's crossings of the cell's own edges — naive surface nets. A
/// cavity has no sharp feature to preserve, so nothing is solved for here; see
/// `docs/design/volume.md`. The normal is the gradient of the cell's trilinear field,
/// which the clustering needs and which costs eight adds.
fn cell_cut(values: &[Scalar; CORNERS], corner: Vec3, voxel_mm: Scalar) -> Option<Cut> {
    let mut total = Vec3::ZERO;
    let mut cuts = 0;
    for (from, to) in edge_ends() {
        let (near, far) = (values[from], values[to]);
        if (near < 0.0) == (far < 0.0) {
            continue;
        }
        let span = near - far;
        let along = if span.abs() > Scalar::EPSILON {
            (near / span).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let start = CORNER_OFFSETS[from].as_vec3() * voxel_mm;
        let end = CORNER_OFFSETS[to].as_vec3() * voxel_mm;
        total += start + (end - start) * along;
        cuts += 1;
    }

    let mut gradient = Vec3::ZERO;
    for (index, value) in values.iter().enumerate() {
        gradient += (CORNER_OFFSETS[index].as_vec3() * 2.0 - Vec3::ONE) * *value;
    }

    (cuts > 0).then(|| Cut {
        at: corner + total / cuts as Scalar,
        normal: gradient.normalize_or_zero(),
    })
}

/// The quads for the three lattice edges leaving `voxel`, wherever the surface crosses one.
///
/// A crossed edge is shared by exactly four cells, and those four vertices are the quad.
/// Only the edges leaving this cell's own lowest corner are emitted, so every edge of the
/// lattice is claimed once, by the cell that owns it.
fn quads_at(values: &[Scalar; CORNERS], voxel: IVec3, quads: &mut Vec<[u64; 4]>) {
    for axis in 0..3 {
        let far = 1 << axis;
        let (near, beyond) = (values[0], values[far]);
        if (near < 0.0) == (beyond < 0.0) {
            continue;
        }

        // The two axes the four cells around this edge are spread over.
        let (across, up) = ((axis + 1) % 3, (axis + 2) % 3);
        let step = |along: usize| {
            let mut back = IVec3::ZERO;
            back[along] = -1;
            back
        };
        let ring = [IVec3::ZERO, step(across), step(across) + step(up), step(up)];

        // Out of the solid: the corner at the near end being inside puts the ring one way
        // round, and the axes being taken in cyclic order gives the other the rest.
        let wound: [IVec3; 4] = if near < 0.0 {
            [ring[0], ring[1], ring[2], ring[3]]
        } else {
            [ring[0], ring[3], ring[2], ring[1]]
        };
        quads.push(wound.map(|offset| cell_key(voxel + offset)));
    }
}

/// Where each corner of a cell sits, as the bits of its own index name it.
const CORNER_OFFSETS: [IVec3; CORNERS] = [
    IVec3::new(0, 0, 0),
    IVec3::new(1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(1, 1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(1, 0, 1),
    IVec3::new(0, 1, 1),
    IVec3::new(1, 1, 1),
];

/// The twelve edges, each joining the two corners that differ on one axis.
fn edge_ends() -> [(usize, usize); EDGES] {
    let mut ends = [(0, 0); EDGES];
    let mut at = 0;
    for near in 0..CORNERS {
        for far in (near + 1)..CORNERS {
            if (near ^ far).is_power_of_two() {
                ends[at] = (near, far);
                at += 1;
            }
        }
    }
    ends
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Corner values for a cell with only the corner at its lowest point inside.
    fn one_corner_inside() -> [Scalar; CORNERS] {
        let mut values = [1.0; CORNERS];
        values[0] = -1.0;
        values
    }

    #[test]
    fn a_cell_the_surface_misses_has_no_vertex() {
        assert!(cell_cut(&[1.0; CORNERS], Vec3::ZERO, 1.0).is_none());
        assert!(cell_cut(&[-1.0; CORNERS], Vec3::ZERO, 1.0).is_none());
    }

    /// Three edges leave the inside corner, each cut at its middle, so the vertex sits a
    /// sixth of the cell along each axis away from that corner.
    #[test]
    fn a_cell_with_one_corner_inside_puts_its_vertex_near_that_corner() {
        let cut = cell_cut(&one_corner_inside(), Vec3::ZERO, 1.0).expect("the surface crosses it");
        let vertex = cut.at;
        assert!(
            (vertex - Vec3::splat(1.0 / 6.0)).length() < 1e-5,
            "three cuts at 0.5 on the axes average to a sixth on each, got {vertex}"
        );
    }

    /// The three edges leaving the lowest corner are cut, so this cell claims three quads;
    /// a cell the surface misses claims none.
    #[test]
    fn a_cell_claims_one_quad_for_each_of_its_own_edges_the_surface_cuts() {
        let mut quads = Vec::new();
        quads_at(&one_corner_inside(), IVec3::ZERO, &mut quads);
        assert_eq!(quads.len(), 3);

        quads.clear();
        quads_at(&[1.0; CORNERS], IVec3::ZERO, &mut quads);
        assert!(quads.is_empty());
    }

    /// A run of cells lying on one plane is one facet; the two sides of a wall thinner
    /// than the slack are not, however near that plane they both lie.
    #[test]
    fn a_flat_run_is_one_facet_and_two_opposed_faces_are_not() {
        let flat: Vec<(IVec3, Cut)> = (0..4)
            .map(|step| {
                (
                    IVec3::new(step, 0, 0),
                    Cut {
                        at: Vec3::new(step as Scalar, 0.0, 0.0),
                        normal: Vec3::Z,
                    },
                )
            })
            .collect();
        assert!(one_facet(&flat, 0.05));

        let mut opposed = flat.clone();
        opposed[3].1.normal = -Vec3::Z;
        assert!(!one_facet(&opposed, 0.05));
    }

    /// A cell standing further off the shared plane than the slack keeps its own vertex.
    #[test]
    fn a_cell_off_the_plane_by_more_than_the_slack_is_not_clustered() {
        let mut bent: Vec<(IVec3, Cut)> = (0..4)
            .map(|step| {
                (
                    IVec3::new(step, 0, 0),
                    Cut {
                        at: Vec3::new(step as Scalar, 0.0, 0.0),
                        normal: Vec3::Z,
                    },
                )
            })
            .collect();
        bent[0].1.at.z = 1.0;
        assert!(!one_facet(&bent, 0.05));
    }

    /// Every quad names four different cells, or the surface would come out with a
    /// triangle of no area in it.
    #[test]
    fn a_quad_names_four_different_cells() {
        let mut quads = Vec::new();
        quads_at(&one_corner_inside(), IVec3::new(3, -4, 5), &mut quads);
        for quad in quads {
            let mut seen = quad.to_vec();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), 4, "a quad named the same cell twice");
        }
    }
}
