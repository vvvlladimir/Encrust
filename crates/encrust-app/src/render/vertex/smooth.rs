use std::ops::Range;

use core_geometry::{Mesh, SmoothNormals};

use super::ModelVertex;

/// `faces` of a mesh as the corners of its [`SmoothNormals`] they reach, each laid out once,
/// and the triangles over them; a face of no area is left out.
pub fn smooth_vertices(
    mesh: &Mesh,
    smooth: &SmoothNormals,
    faces: Range<usize>,
) -> (Vec<ModelVertex>, Vec<u32>) {
    let mut local = vec![u32::MAX; smooth.normals.len()];
    let mut vertices = Vec::new();
    let mut indices = Vec::with_capacity(faces.len() * 3);
    for face in faces {
        let corners = &smooth.corners[face * 3..face * 3 + 3];
        if corners.contains(&u32::MAX) {
            continue;
        }
        for &corner in corners {
            let at = &mut local[corner as usize];
            if *at == u32::MAX {
                *at = vertices.len() as u32;
                vertices.push(ModelVertex {
                    position: mesh.vertices[smooth.vertices[corner as usize] as usize].to_array(),
                    normal: smooth.normals[corner as usize].to_array(),
                });
            }
            indices.push(*at);
        }
    }
    (vertices, indices)
}

#[cfg(test)]
mod tests;
