use super::triangulation::{MIN_NORMAL, ear_clipped, least_area, newell};
use crate::hash::FastMap;
use crate::topology::{edge_groups, edge_uses};
use crate::{Mesh, Scalar, Vec3};

/// Longest loop whose patch is chosen by area. The choice costs the cube of the corner
/// count, so a longer loop, which in practice is a flat cut, is ear clipped in its plane.
const MAX_MINIMAL_RING: usize = 256;

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
    fill_holes_within(mesh, usize::MAX)
}

/// Closes the boundary loops of at most `most_corners` corners as [`fill_holes`] does, and
/// leaves the longer ones open without counting them.
pub fn fill_holes_within(mesh: &mut Mesh, most_corners: usize) -> Filled {
    let (rings, loops_left) = boundary_loops(mesh);
    let mut filled = Filled {
        loops_left,
        ..Filled::default()
    };
    for ring in rings.into_iter().filter(|ring| ring.len() <= most_corners) {
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

/// The mesh's boundary loops, wound as a closing patch is, and how many ends never came back round.
/// An odd-use edge cannot pair off without one more face; a fin is closed by its own reverse.
fn boundary_loops(mesh: &Mesh) -> (Vec<Vec<u32>>, usize) {
    let uses = edge_uses(mesh);
    let mut onwards: FastMap<u32, Vec<u32>> = FastMap::default();
    let mut starts = Vec::new();
    for group in edge_groups(&uses).filter(|group| group.len() % 2 == 1) {
        let forward = group.iter().filter(|edge_use| edge_use.forward).count();
        let backward = group.len() - forward;
        let (low, high) = group[0].edge;
        let (from, to) = if forward > backward {
            (high, low)
        } else {
            (low, high)
        };
        for _ in 0..forward.abs_diff(backward) {
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
    let points: Vec<Vec3> = ring
        .iter()
        .map(|index| *mesh.vertices.get(*index as usize).unwrap_or(&Vec3::ZERO))
        .collect();
    let minimal = (ring.len() <= MAX_MINIMAL_RING)
        .then(|| least_area(&points))
        .flatten();
    let Some(faces) = minimal.or_else(|| ear_clipped(&points)) else {
        // A ring whose projection folds over itself would be a hole again if left short;
        // a fan at least closes it. One with no plane has nothing a fan could lie in.
        return if newell(&points).length_squared() < MIN_NORMAL {
            0
        } else {
            fan(mesh, ring)
        };
    };
    mesh.faces.extend(
        faces
            .iter()
            .map(|face| face.map(|corner| ring[corner as usize])),
    );
    faces.len()
}

/// Closes the loop with a fan from one new vertex in its middle, which is what a loop too
/// long, too folded or too tangled to triangulate otherwise gets.
pub(super) fn fan(mesh: &mut Mesh, ring: &[u32]) -> usize {
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
