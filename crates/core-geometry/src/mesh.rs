use crate::{Aabb, Scalar, Triangle, Vec3};

/// An indexed triangle mesh in model space, millimetres.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<[u32; 3]>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vec3>, faces: Vec<[u32; 3]>) -> Self {
        Self { vertices, faces }
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Resolves a face index into its three vertices.
    pub fn triangle(&self, face: usize) -> Option<Triangle> {
        let [i, j, k] = *self.faces.get(face)?;
        Some(Triangle::new(
            *self.vertices.get(i as usize)?,
            *self.vertices.get(j as usize)?,
            *self.vertices.get(k as usize)?,
        ))
    }

    pub fn triangles(&self) -> impl Iterator<Item = Triangle> + '_ {
        (0..self.faces.len()).filter_map(|i| self.triangle(i))
    }

    /// Total area of every face, in square millimetres.
    ///
    /// What a field over the mesh costs follows this rather than the model's longest side:
    /// a spire and a block of the same height are not the same surface.
    pub fn surface_area(&self) -> Scalar {
        self.triangles().map(|triangle| triangle.area()).sum()
    }

    /// Axis-aligned bounds of all vertices, or `None` for a mesh without vertices.
    pub fn aabb(&self) -> Option<Aabb> {
        let first = *self.vertices.first()?;
        let (min, max) = self
            .vertices
            .iter()
            .fold((first, first), |(min, max), v| (min.min(*v), max.max(*v)));
        Some(Aabb::new(min, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slanted_quad() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 2.0),
                Vec3::new(0.0, 1.0, 2.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    #[test]
    fn aabb_covers_every_vertex() {
        let aabb = slanted_quad().aabb().expect("mesh has vertices");
        assert_eq!(aabb.mins, Vec3::ZERO);
        assert_eq!(aabb.maxs, Vec3::new(1.0, 1.0, 2.0));
    }

    #[test]
    fn empty_mesh_has_no_aabb() {
        assert!(Mesh::default().aabb().is_none());
    }

    #[test]
    fn triangles_match_face_count() {
        let mesh = slanted_quad();
        let total: Scalar = mesh.triangles().map(|t| t.area()).sum();
        assert_eq!(mesh.triangles().count(), 2);
        assert!(total > 0.0);
    }

    #[test]
    fn out_of_range_face_is_none() {
        assert!(slanted_quad().triangle(9).is_none());
    }
}
