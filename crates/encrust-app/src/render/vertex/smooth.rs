use std::ops::Range;

use core_geometry::{Mesh, Vec3};

use super::{ModelVertex, normalized_or_up};

/// How far two faces meeting at a corner may turn from each other and still be shaded as one
/// surface, degrees: a tube's facets and rings blend, the rim of a pad stays a rim.
const CREASE_DEG: f32 = 50.0;

/// `faces` of a mesh as corners carrying normals smoothed over the faces round them within
/// [`CREASE_DEG`] of each other, and the triangles over them; a corner is laid out once a side.
pub fn smooth_vertices(mesh: &Mesh, faces: Range<usize>) -> (Vec<ModelVertex>, Vec<u32>) {
    let first = faces.start;
    // A face of no area has no way to face; it is left out rather than shaded straight up.
    let normals: Vec<Option<Vec3>> = faces
        .clone()
        .map(|face| {
            Some(mesh.triangle(face)?.normal_unnormalized()).filter(|n| n.length_squared() > 0.0)
        })
        .collect();
    let around = Around::of(mesh, &faces, &normals);
    let crease = CREASE_DEG.to_radians().cos();
    let unit = |local: usize| normalized_or_up(normals[local].unwrap_or(Vec3::Z));

    let mut vertices = Vec::new();
    let mut corners = vec![u32::MAX; normals.len() * 3];
    let mut laid: Vec<(Vec3, u32)> = Vec::new();
    for vertex in 0..mesh.vertices.len() {
        laid.clear();
        for &local in around.faces_of(vertex) {
            let own = unit(local);
            // Each face counts by its angle at the corner, so how a quad was split cannot lean it.
            let blended: Vec3 = around
                .faces_of(vertex)
                .iter()
                .filter(|&&other| unit(other).dot(own) >= crease)
                .map(|&other| unit(other) * angle_at(mesh, first + other, vertex))
                .sum();
            let normal = normalized_or_up(blended);
            let index = laid
                .iter()
                .find(|(seen, _)| *seen == normal)
                .map(|(_, index)| *index)
                .unwrap_or_else(|| {
                    vertices.push(ModelVertex {
                        position: mesh.vertices[vertex].to_array(),
                        normal: normal.to_array(),
                    });
                    let index = (vertices.len() - 1) as u32;
                    laid.push((normal, index));
                    index
                });
            for (slot, corner) in mesh.faces[first + local].iter().enumerate() {
                if *corner as usize == vertex {
                    corners[local * 3 + slot] = index;
                }
            }
        }
    }
    let indices = corners
        .chunks(3)
        .zip(&normals)
        .filter(|(_, normal)| normal.is_some())
        .flat_map(|(face, _)| face.iter().copied())
        .collect();
    (vertices, indices)
}

/// The angle face `face` of `mesh` makes at its corner `vertex`, radians.
fn angle_at(mesh: &Mesh, face: usize, vertex: usize) -> f32 {
    let corners = mesh.faces[face];
    let Some(slot) = corners.iter().position(|&corner| corner as usize == vertex) else {
        return 0.0;
    };
    let at = mesh.vertices[vertex];
    let next = mesh.vertices[corners[(slot + 1) % 3] as usize] - at;
    let previous = mesh.vertices[corners[(slot + 2) % 3] as usize] - at;
    let angle = next.angle_between(previous);
    if angle.is_finite() { angle } else { 0.0 }
}

/// The faces of a range round each vertex of the mesh, by their place in the range.
struct Around {
    starts: Vec<usize>,
    faces: Vec<usize>,
}

impl Around {
    fn of(mesh: &Mesh, faces: &Range<usize>, normals: &[Option<Vec3>]) -> Self {
        let mut starts = vec![0; mesh.vertices.len() + 1];
        let kept = || {
            faces
                .clone()
                .enumerate()
                .filter(|(local, _)| normals[*local].is_some())
        };
        for (_, face) in kept() {
            for corner in mesh.faces[face] {
                starts[corner as usize + 1] += 1;
            }
        }
        for vertex in 0..mesh.vertices.len() {
            starts[vertex + 1] += starts[vertex];
        }
        let mut filled = starts.clone();
        let mut around = vec![0; starts[mesh.vertices.len()]];
        for (local, face) in kept() {
            for corner in mesh.faces[face] {
                around[filled[corner as usize]] = local;
                filled[corner as usize] += 1;
            }
        }
        Self {
            starts,
            faces: around,
        }
    }

    fn faces_of(&self, vertex: usize) -> &[usize] {
        &self.faces[self.starts[vertex]..self.starts[vertex + 1]]
    }
}

#[cfg(test)]
mod tests;
