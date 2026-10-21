use core_csg::{CsgError, Solid};
use core_geometry::{FastMap, FastSet, Mesh, fill_holes, orient_outward, remove_duplicate_faces};

use crate::boolean::solid_of;

/// `mesh` as the engine reads it, its shells unioned, mended first where it refuses it as it
/// stands (ADR 0321).
pub(crate) fn readable(mesh: &Mesh) -> Result<Solid, CsgError> {
    solid_of(mesh).or_else(|_| solid_of(&patched(mesh)))
}

/// `model` mended until the engine can read it: torn and folded faces patched, then every
/// face touching an edge still unpaired cut out and filled, which moves the surface only there.
fn patched(model: &Mesh) -> Mesh {
    let mut mesh = model.clone();
    remove_duplicate_faces(&mut mesh);
    orient_outward(&mut mesh);
    fill_holes(&mut mesh);
    let unsound = unpaired_corners(&mesh);
    if !unsound.is_empty() {
        mesh.faces
            .retain(|face| !face.iter().any(|corner| unsound.contains(corner)));
        fill_holes(&mut mesh);
    }
    orient_outward(&mut mesh);
    mesh
}

/// The corners of every edge not walked once each way, as a closed surface walks it.
fn unpaired_corners(mesh: &Mesh) -> FastSet<u32> {
    let mut walks: FastMap<(u32, u32), (u32, u32)> = FastMap::default();
    for face in &mesh.faces {
        for side in 0..3 {
            let (from, to) = (face[side], face[(side + 1) % 3]);
            let walked = walks.entry((from.min(to), from.max(to))).or_default();
            if from < to {
                walked.0 += 1;
            } else {
                walked.1 += 1;
            }
        }
    }
    walks
        .into_iter()
        .filter(|(_, walked)| *walked != (1, 1))
        .flat_map(|((low, high), _)| [low, high])
        .collect()
}
