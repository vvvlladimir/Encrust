use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rayon::slice::ParallelSlice;

use crate::topology::{Components, edge_uses};
use crate::{FastMap, Mesh};

/// Splits a mesh into the pieces that share no vertex, the biggest piece first.
/// Pieces follow vertex indices, so weld first to get the shells a user would call parts.
pub fn split(mesh: &Mesh) -> Vec<Mesh> {
    let joins = mesh
        .faces
        .par_iter()
        .flat_map_iter(|&[a, b, c]| [[a, b], [a, c]]);
    let (vertex_piece, count) = Components::joined(mesh.vertices.len(), joins).labels();
    let piece: Vec<u32> = mesh
        .faces
        .par_iter()
        .map(|face| vertex_piece[face[0] as usize])
        .collect();
    pieces(mesh, &piece, count)
}

/// The pieces `mesh` is made of, two faces in one piece only through an edge exactly the
/// two of them use. Two bodies meeting at a corner, or along an edge three or four faces
/// use, come apart, where [`split`] would read them as one.
pub fn split_at_edges(mesh: &Mesh) -> Vec<Mesh> {
    let uses = edge_uses(mesh);
    let joins = uses
        .par_chunk_by(|a, b| a.edge == b.edge)
        .filter_map(|group| match group {
            [left, right] => Some([left.face, right.face]),
            _ => None,
        });
    let (piece, count) = Components::joined(mesh.faces.len(), joins).labels();
    pieces(mesh, &piece, count)
}

/// The faces of `mesh` gathered by `piece`, one of `count` labels a face, each piece holding
/// only the vertices it uses in the order its faces first use them, biggest first.
fn pieces(mesh: &Mesh, piece: &[u32], count: usize) -> Vec<Mesh> {
    let mut pieces = vec![Mesh::default(); count];
    // A vertex is nearly always in one piece only, so the first piece to use it keeps its
    // new index here and any other goes to the map.
    let mut first: Vec<(u32, u32)> = vec![(u32::MAX, 0); mesh.vertices.len()];
    let mut others: FastMap<(u32, u32), u32> = FastMap::default();
    for (face, &label) in mesh.faces.iter().zip(piece) {
        let piece = &mut pieces[label as usize];
        let corners = face.map(|index| {
            let mut take = || {
                piece.vertices.push(mesh.vertices[index as usize]);
                (piece.vertices.len() - 1) as u32
            };
            match first[index as usize] {
                (owner, at) if owner == label => at,
                (u32::MAX, _) => {
                    let at = take();
                    first[index as usize] = (label, at);
                    at
                }
                _ => *others.entry((label, index)).or_insert_with(take),
            }
        });
        piece.faces.push(corners);
    }
    pieces.retain(|piece| !piece.faces.is_empty());

    // Biggest first, so the body a user cares about is the piece they get first. Two
    // pieces of the same face count are ordered by volume, then by their first face.
    let mut parts = pieces;
    parts.sort_by(|a, b| {
        b.faces.len().cmp(&a.faces.len()).then_with(|| {
            crate::signed_volume(b)
                .abs()
                .total_cmp(&crate::signed_volume(a).abs())
        })
    });
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Vec3, signed_volume, weld};

    /// Axis-aligned box, twelve triangles, spanning `min`..`max`.
    fn cuboid(min: Vec3, max: Vec3) -> Mesh {
        crate::cuboid(min, max)
    }

    fn joined(meshes: &[Mesh]) -> Mesh {
        let mut merged = Mesh::default();
        for mesh in meshes {
            let offset = merged.vertices.len() as u32;
            merged.vertices.extend(mesh.vertices.iter().copied());
            merged.faces.extend(
                mesh.faces
                    .iter()
                    .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
            );
        }
        merged
    }

    #[test]
    fn two_boxes_that_touch_nothing_split_in_two() {
        let small = cuboid(Vec3::ZERO, Vec3::splat(2.0));
        let large = cuboid(Vec3::splat(10.0), Vec3::splat(20.0));
        let parts = split(&joined(&[small, large]));

        assert_eq!(parts.len(), 2);
        assert!(
            (signed_volume(&parts[0]) - 1000.0).abs() < 1e-3,
            "the biggest piece comes first"
        );
        assert!((signed_volume(&parts[1]) - 8.0).abs() < 1e-3);
    }

    #[test]
    fn one_solid_comes_back_whole() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(5.0));
        let parts = split(&cube);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].faces.len(), cube.faces.len());
    }

    #[test]
    fn a_hollow_model_splits_into_its_shell_and_its_cavity() {
        let mut hollow = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let inner = cuboid(Vec3::splat(2.0), Vec3::splat(8.0));
        let offset = hollow.vertices.len() as u32;
        hollow.vertices.extend(inner.vertices.iter().copied());
        hollow.faces.extend(
            inner
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
        );

        // The two shells share no vertex, so they are two pieces however they nest.
        assert_eq!(split(&hollow).len(), 2);
    }

    #[test]
    fn welding_first_is_what_makes_touching_copies_one_piece() {
        let mesh = joined(&[
            cuboid(Vec3::ZERO, Vec3::splat(5.0)),
            cuboid(Vec3::new(5.0, 0.0, 0.0), Vec3::new(10.0, 5.0, 5.0)),
        ]);
        assert_eq!(split(&mesh).len(), 2, "as indexed, they are separate");

        let welded = weld(&mesh, crate::DEFAULT_WELD_TOLERANCE);
        assert_eq!(
            split(&welded.mesh).len(),
            1,
            "sharing a face's vertices makes them one"
        );
    }

    #[test]
    fn boxes_meeting_along_an_edge_are_one_piece_by_vertex_and_two_by_edge() {
        let welded = weld(
            &joined(&[
                cuboid(Vec3::ZERO, Vec3::ONE),
                cuboid(Vec3::new(1.0, 1.0, 0.0), Vec3::new(2.0, 2.0, 1.0)),
            ]),
            1e-6,
        )
        .mesh;
        assert_eq!(
            split(&welded).len(),
            1,
            "the shared edge's vertices join them"
        );
        assert_eq!(
            split_at_edges(&welded).len(),
            2,
            "the edge four faces use joins nothing"
        );
    }

    #[test]
    fn a_mesh_with_no_faces_has_no_pieces() {
        assert!(split(&Mesh::default()).is_empty());
    }
}
