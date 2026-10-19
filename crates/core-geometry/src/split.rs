use crate::topology::{Components, edge_groups, edge_uses};
use crate::{FastMap, Mesh};

/// Splits a mesh into the pieces that share no vertex, the biggest piece first.
///
/// Two triangles belong to the same piece when they share a vertex index, so a mesh that
/// was welded first splits into the shells a user would call separate parts, and one that
/// was not splits into whatever its indices say. A mesh that is all one piece comes back
/// as itself.
pub fn split(mesh: &Mesh) -> Vec<Mesh> {
    let mut vertices = Components::new(mesh.vertices.len());
    for [a, b, c] in &mesh.faces {
        vertices.union(*a, *b);
        vertices.union(*a, *c);
    }
    let roots: Vec<u32> = mesh
        .faces
        .iter()
        .map(|face| vertices.find(face[0]))
        .collect();
    pieces(mesh, &roots)
}

/// The pieces `mesh` is made of, two faces in one piece only through an edge exactly the
/// two of them use. Two bodies meeting at a corner, or along an edge three or four faces
/// use, come apart, where [`split`] would read them as one.
pub fn split_at_edges(mesh: &Mesh) -> Vec<Mesh> {
    let mut faces = Components::new(mesh.faces.len());
    for group in edge_groups(&edge_uses(mesh)) {
        if let [left, right] = group {
            faces.union(left.face, right.face);
        }
    }
    let roots: Vec<u32> = (0..mesh.faces.len() as u32)
        .map(|face| faces.find(face))
        .collect();
    pieces(mesh, &roots)
}

/// The faces of `mesh` gathered by `roots`, one root a face, each piece holding only the
/// vertices it uses, biggest first.
fn pieces(mesh: &Mesh, roots: &[u32]) -> Vec<Mesh> {
    let mut pieces: FastMap<u32, Mesh> = FastMap::default();
    let mut moved: FastMap<(u32, u32), u32> = FastMap::default();
    for (face, &root) in mesh.faces.iter().zip(roots) {
        let piece = pieces.entry(root).or_default();
        let corners = face.map(|index| {
            *moved.entry((root, index)).or_insert_with(|| {
                piece.vertices.push(mesh.vertices[index as usize]);
                (piece.vertices.len() - 1) as u32
            })
        });
        piece.faces.push(corners);
    }

    // Biggest first, so the body a user cares about is the piece they get first. Two
    // pieces of the same face count are ordered by volume, or the map's order would
    // decide it.
    let mut parts: Vec<Mesh> = pieces.into_values().collect();
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
        let vertices = vec![
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];
        let faces = vec![
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
        ];
        Mesh::new(vertices, faces)
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
