use crate::hash::FastMap;
use crate::topology::{edge_groups, edge_uses};
use crate::triangulate::triangulate;
use crate::{Mesh, Scalar, Vec2, Vec3};

/// Smallest cross product treated as a direction, mm². Below it the loop is a line and
/// has no plane to be triangulated in.
const MIN_NORMAL: Scalar = 1e-12;

/// What closing the boundary loops of a mesh added to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Filled {
    pub loops_filled: usize,
    pub faces_added: usize,
    /// Loops left open: a boundary that branches at a vertex has no one way round it.
    pub loops_left: usize,
}

impl Filled {
    /// Nothing was open, so the mesh is as it came.
    pub fn is_empty(&self) -> bool {
        self.loops_filled == 0 && self.loops_left == 0
    }
}

/// Closes every boundary loop of a mesh with a patch of triangles wound like the surface
/// around them.
///
/// Run [`crate::weld`] first: on an unwelded mesh every edge is a boundary. A loop with no
/// plane at all is left open and counted; see `docs/design/mesh-repair.md`.
pub fn fill_holes(mesh: &mut Mesh) -> Filled {
    let (rings, loops_left) = boundary_loops(mesh);
    let mut filled = Filled {
        loops_left,
        ..Filled::default()
    };
    for ring in rings {
        let added = patch(mesh, &ring);
        if added > 0 {
            filled.loops_filled += 1;
            filled.faces_added += added;
        } else {
            filled.loops_left += 1;
        }
    }
    filled
}

/// The mesh's boundary loops, each in the direction a patch closing it is wound, and how
/// many ends were reached without coming back round.
///
/// A boundary edge is used by one face, so the patch across it walks it the other way:
/// following those reversed edges gives a loop whose triangles agree with the surface.
fn boundary_loops(mesh: &Mesh) -> (Vec<Vec<u32>>, usize) {
    let uses = edge_uses(mesh);
    let mut onwards: FastMap<u32, Vec<u32>> = FastMap::default();
    let mut starts = Vec::new();
    for group in edge_groups(&uses) {
        if let [hole] = group {
            let (from, to) = if hole.forward {
                (hole.edge.1, hole.edge.0)
            } else {
                hole.edge
            };
            onwards.entry(from).or_default().push(to);
            starts.push(from);
        }
    }

    let mut rings = Vec::new();
    let mut left = 0;
    for start in starts {
        if onwards.get(&start).is_none_or(Vec::is_empty) {
            continue;
        }
        match walk(&mut onwards, start) {
            Some(ring) => rings.push(ring),
            None => left += 1,
        }
    }
    (rings, left)
}

/// Follows boundary edges from `start` until they come back to it, taking each edge only
/// once. `None` for a walk that runs out of edges first, which a branching boundary does.
fn walk(onwards: &mut FastMap<u32, Vec<u32>>, start: u32) -> Option<Vec<u32>> {
    let mut ring = vec![start];
    let mut at = start;
    loop {
        let to = onwards.get_mut(&at).and_then(Vec::pop)?;
        if to == start {
            return Some(ring);
        }
        ring.push(to);
        at = to;
    }
}

/// Lays triangles over one loop and returns how many it added, or zero for a loop with no
/// plane to lay them in.
fn patch(mesh: &mut Mesh, ring: &[u32]) -> usize {
    if ring.len() < 3 {
        return 0;
    }
    if ring.len() == 3 {
        mesh.faces.push([ring[0], ring[1], ring[2]]);
        return 1;
    }
    let Some(flat) = flatten(mesh, ring) else {
        return 0;
    };
    let corners: Vec<u32> = (0..ring.len() as u32).collect();
    let faces = triangulate(&corners, &[], &flat);
    // A ring whose projection folds over itself comes back short, and what is missing
    // would be a hole again; a fan at least closes it.
    if faces.len() + 2 != ring.len() {
        return fan(mesh, ring);
    }
    mesh.faces.extend(faces.iter().map(|face| {
        [
            ring[face[0] as usize],
            ring[face[1] as usize],
            ring[face[2] as usize],
        ]
    }));
    faces.len()
}

/// The loop in the two dimensions of its own plane, wound as it is round the plane's
/// normal, which is the loop's own.
fn flatten(mesh: &Mesh, ring: &[u32]) -> Option<Vec<Vec2>> {
    let points: Vec<Vec3> = ring
        .iter()
        .map(|index| *mesh.vertices.get(*index as usize).unwrap_or(&Vec3::ZERO))
        .collect();
    // Newell's normal: the area-weighted normal of a loop that need not be flat, and
    // independent of where the loop stands.
    let mut normal = Vec3::ZERO;
    for (at, point) in points.iter().enumerate() {
        normal += point.cross(points[(at + 1) % points.len()]);
    }
    if normal.length_squared() < MIN_NORMAL {
        return None;
    }
    let across = normal.normalize().any_orthonormal_vector();
    let up = normal.normalize().cross(across);
    Some(
        points
            .iter()
            .map(|point| Vec2::new(point.dot(across), point.dot(up)))
            .collect(),
    )
}

/// Closes the loop with a fan from one new vertex in its middle, which is what a loop too
/// long, too folded or too tangled to ear clip gets.
fn fan(mesh: &mut Mesh, ring: &[u32]) -> usize {
    let middle: Vec3 = ring
        .iter()
        .filter_map(|index| mesh.vertices.get(*index as usize))
        .copied()
        .sum::<Vec3>()
        / ring.len() as Scalar;
    let hub = mesh.vertices.len() as u32;
    mesh.vertices.push(middle);
    for (at, corner) in ring.iter().enumerate() {
        mesh.faces.push([*corner, ring[(at + 1) % ring.len()], hub]);
    }
    ring.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DEFAULT_WELD_TOLERANCE, diagnose, orient_outward, signed_volume, weld};

    /// A 2 mm cube, twelve triangles, wound outwards, standing on the origin.
    fn cube() -> Mesh {
        let corner = |x: Scalar, y: Scalar, z: Scalar| Vec3::new(x, y, z) * 2.0;
        Mesh::new(
            vec![
                corner(0.0, 0.0, 0.0),
                corner(1.0, 0.0, 0.0),
                corner(1.0, 1.0, 0.0),
                corner(0.0, 1.0, 0.0),
                corner(0.0, 0.0, 1.0),
                corner(1.0, 0.0, 1.0),
                corner(1.0, 1.0, 1.0),
                corner(0.0, 1.0, 1.0),
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

    /// The cube with the two triangles of its top taken off, which is what a file written
    /// by a broken exporter looks like: one square hole.
    fn cube_missing_top() -> Mesh {
        let whole = cube();
        let faces = [&whole.faces[..2], &whole.faces[4..]].concat();
        Mesh::new(whole.vertices, faces)
    }

    /// The cube open at both ends: two square holes, two loops, one patch each.
    fn cube_open_both_ends() -> Mesh {
        let whole = cube();
        Mesh::new(whole.vertices, whole.faces[4..].to_vec())
    }

    #[test]
    fn a_closed_mesh_is_left_as_it_is() {
        let mut mesh = cube();
        let filled = fill_holes(&mut mesh);
        assert!(filled.is_empty());
        assert_eq!(mesh.faces.len(), 12);
        assert_eq!(mesh.vertices.len(), 8);
    }

    #[test]
    fn a_square_hole_is_closed_with_two_triangles_and_no_new_vertex() {
        let mut mesh = cube_missing_top();
        let filled = fill_holes(&mut mesh);

        assert_eq!(filled.loops_filled, 1);
        assert_eq!(filled.faces_added, 2, "a square loop ear clips into two");
        assert_eq!(filled.loops_left, 0);
        assert_eq!(mesh.vertices.len(), 8, "ear clipping adds no vertex");
        assert!(diagnose(&mesh).is_closed());
    }

    #[test]
    fn a_patch_is_wound_like_the_surface_it_closes() {
        let mut mesh = cube_missing_top();
        fill_holes(&mut mesh);
        // The cube is 2 mm on a side, so a patch wound inwards would read -8.
        assert!(
            (signed_volume(&mesh) - 8.0).abs() < 1e-4,
            "a closed 2 mm cube encloses 8 mm³, got {}",
            signed_volume(&mesh)
        );
        assert_eq!(orient_outward(&mut mesh).flipped_faces, 0);
    }

    #[test]
    fn each_hole_of_a_mesh_open_at_both_ends_gets_its_own_patch() {
        let mut mesh = cube_open_both_ends();
        let filled = fill_holes(&mut mesh);

        assert_eq!(filled.loops_filled, 2);
        assert_eq!(filled.faces_added, 4);
        assert!(diagnose(&mesh).is_closed());
        assert!((signed_volume(&mesh) - 8.0).abs() < 1e-4);
    }

    #[test]
    fn a_loop_with_no_plane_is_left_open() {
        // Three vertices on one line: a loop with no plane to triangulate in, and a fan
        // from its middle would be as flat.
        let mut mesh = Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(3.0, 0.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        );
        let before = mesh.faces.len();
        let filled = fill_holes(&mut mesh);
        assert_eq!(filled.loops_filled, 0);
        assert_eq!(filled.loops_left, 1, "the boundary has no patch");
        assert_eq!(mesh.faces.len(), before, "nothing is laid over a line");
    }

    #[test]
    fn a_ring_no_triangulation_covers_is_fanned_from_a_vertex_of_its_own() {
        let mut mesh = Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(2.0, 2.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
            ],
            Vec::new(),
        );
        let added = fan(&mut mesh, &[0, 1, 2, 3]);

        assert_eq!(added, 4, "one triangle per edge of the ring");
        assert_eq!(mesh.vertices.len(), 5, "the fan's own middle");
        assert_eq!(mesh.vertices[4], Vec3::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn filling_a_welded_mesh_closes_what_welding_left_open() {
        let unwelded = cube_missing_top();
        let mut welded = weld(&unwelded, DEFAULT_WELD_TOLERANCE).mesh;
        fill_holes(&mut welded);
        assert!(diagnose(&welded).is_closed());
    }
}
