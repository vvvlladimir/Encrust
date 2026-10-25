use rayon::prelude::*;

use super::remove_duplicate_faces;
use crate::hash::FastMap;
use crate::{Mesh, Scalar};

/// How near two corners of a face with no area must stand to be one point, mm. Two cells'
/// vertices on one lattice point differ in the last bits of an `f32`, far under this; a
/// sliver whose corners are further apart is left whole.
const COINCIDENT_MM: Scalar = 1e-4;

/// Welds the corners of a zero-area face on one point and drops degenerate faces; returns the
/// count. Collapsing a zero-length edge keeps a closed surface closed.
pub fn collapse_flat_faces(mesh: &mut Mesh) -> usize {
    let mut onto: FastMap<u32, u32> = FastMap::default();
    for face in flat_faces(mesh) {
        for (one, other) in [(face[0], face[1]), (face[1], face[2]), (face[0], face[2])] {
            let gap = mesh.vertices[one as usize] - mesh.vertices[other as usize];
            if one != other && gap.length() <= COINCIDENT_MM {
                let (one, other) = (root(&onto, one), root(&onto, other));
                if one != other {
                    onto.insert(one.max(other), one.min(other));
                }
            }
        }
    }
    if onto.is_empty() {
        return 0;
    }
    for face in &mut mesh.faces {
        for corner in face.iter_mut() {
            *corner = root(&onto, *corner);
        }
    }
    let before = mesh.faces.len();
    mesh.faces
        .retain(|face| face[0] != face[1] && face[1] != face[2] && face[0] != face[2]);
    // A face the weld folded onto another is that one's reverse, and the two go together.
    remove_duplicate_faces(mesh);
    before - mesh.faces.len()
}

/// The corner `corner` was welded onto, through every weld after it.
fn root(onto: &FastMap<u32, u32>, mut corner: u32) -> u32 {
    while let Some(next) = onto.get(&corner) {
        corner = *next;
    }
    corner
}

/// The faces whose corners enclose no area at all.
fn flat_faces(mesh: &Mesh) -> Vec<[u32; 3]> {
    mesh.faces
        .par_iter()
        .filter(|face| {
            let [a, b, c] = face.map(|corner| mesh.vertices[corner as usize]);
            (b - a).cross(c - a).length_squared() == 0.0
        })
        .copied()
        .collect()
}
