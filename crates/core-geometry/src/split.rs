use crate::{FastMap, Mesh};

/// Splits a mesh into the pieces that share no vertex, the biggest piece first.
///
/// Two triangles belong to the same piece when they share a vertex index, so a mesh that
/// was welded first splits into the shells a user would call separate parts, and one that
/// was not splits into whatever its indices say. A mesh that is all one piece comes back
/// as itself.
pub fn split(mesh: &Mesh) -> Vec<Mesh> {
    if mesh.faces.is_empty() {
        return Vec::new();
    }

    let mut parent: Vec<u32> = (0..mesh.vertices.len() as u32).collect();
    for face in &mesh.faces {
        let root = find(&mut parent, face[0]);
        for corner in &face[1..] {
            let other = find(&mut parent, *corner);
            union(&mut parent, root, other);
        }
    }

    let mut pieces: FastMap<u32, Mesh> = FastMap::default();
    let mut moved: FastMap<(u32, u32), u32> = FastMap::default();
    for face in &mesh.faces {
        let root = find(&mut parent, face[0]);
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

/// Root of the set `index` is in, flattening the path it walked on the way.
fn find(parent: &mut [u32], index: u32) -> u32 {
    let mut root = index;
    while parent[root as usize] != root {
        root = parent[root as usize];
    }
    let mut walk = index;
    while parent[walk as usize] != root {
        let next = parent[walk as usize];
        parent[walk as usize] = root;
        walk = next;
    }
    root
}

fn union(parent: &mut [u32], left: u32, right: u32) {
    let (left, right) = (find(parent, left), find(parent, right));
    if left != right {
        parent[right.max(left) as usize] = left.min(right);
    }
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
    fn a_mesh_with_no_faces_has_no_pieces() {
        assert!(split(&Mesh::default()).is_empty());
    }
}
