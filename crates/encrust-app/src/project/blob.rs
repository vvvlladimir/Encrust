//! A mesh as bytes inside a project file.
//!
//! Not STL: the painted patches of `core_supports::Region` are sets of face indices, so a
//! round trip that renumbered or dropped a face would move every patch on the model. This
//! writes the index buffer as it stands. See `docs/formats/encrust-project.md`.

use core_geometry::{Mesh, Vec3};

use crate::project::ProjectError;

const MAGIC: [u8; 4] = *b"ENCM";
const HEADER: usize = 12;

pub fn write(mesh: &Mesh) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER + mesh.vertices.len() * 12 + mesh.faces.len() * 12);
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&(mesh.vertices.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(mesh.faces.len() as u32).to_le_bytes());
    for vertex in &mesh.vertices {
        for axis in [vertex.x, vertex.y, vertex.z] {
            bytes.extend_from_slice(&axis.to_le_bytes());
        }
    }
    for face in &mesh.faces {
        for index in face {
            bytes.extend_from_slice(&index.to_le_bytes());
        }
    }
    bytes
}

pub fn read(name: &str, bytes: &[u8]) -> Result<Mesh, ProjectError> {
    let bad = || ProjectError::BadMesh {
        name: name.to_owned(),
    };
    if bytes.len() < HEADER || bytes[..4] != MAGIC {
        return Err(bad());
    }
    let vertices = u32_at(bytes, 4) as usize;
    let faces = u32_at(bytes, 8) as usize;
    if bytes.len() != HEADER + vertices * 12 + faces * 12 {
        return Err(bad());
    }

    let mesh = Mesh::new(
        (0..vertices)
            .map(|i| {
                let at = HEADER + i * 12;
                Vec3::new(
                    f32_at(bytes, at),
                    f32_at(bytes, at + 4),
                    f32_at(bytes, at + 8),
                )
            })
            .collect(),
        (0..faces)
            .map(|i| {
                let at = HEADER + vertices * 12 + i * 12;
                [
                    u32_at(bytes, at),
                    u32_at(bytes, at + 4),
                    u32_at(bytes, at + 8),
                ]
            })
            .collect(),
    );

    let limit = vertices as u32;
    if mesh.faces.iter().flatten().any(|index| *index >= limit) {
        return Err(bad());
    }
    Ok(mesh)
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tetrahedron() -> Mesh {
        Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        )
    }

    #[test]
    fn a_mesh_comes_back_with_the_same_faces_in_the_same_order() {
        let mesh = tetrahedron();
        let back = read("models/0.mesh", &write(&mesh)).expect("what was just written reads");
        assert_eq!(
            back, mesh,
            "a face index is what a painted patch is stored as"
        );
    }

    #[test]
    fn an_empty_mesh_round_trips() {
        let back = read("models/0.mesh", &write(&Mesh::default())).expect("an empty mesh reads");
        assert!(back.is_empty());
    }

    #[test]
    fn a_blob_without_the_magic_is_refused() {
        let error = read("models/0.mesh", b"not a mesh at all").expect_err("the magic is wrong");
        assert!(matches!(error, ProjectError::BadMesh { .. }));
    }

    #[test]
    fn a_truncated_blob_is_refused() {
        let bytes = write(&tetrahedron());
        let error =
            read("models/0.mesh", &bytes[..bytes.len() - 4]).expect_err("the body is short");
        assert!(matches!(error, ProjectError::BadMesh { .. }));
    }

    #[test]
    fn a_face_pointing_past_the_vertices_is_refused() {
        let mesh = Mesh::new(vec![Vec3::ZERO], vec![[0, 1, 2]]);
        let error = read("models/0.mesh", &write(&mesh)).expect_err("the face is out of range");
        assert!(matches!(error, ProjectError::BadMesh { .. }));
    }
}
