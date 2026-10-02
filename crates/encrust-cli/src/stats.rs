use core_geometry::{Mesh, Scalar, Vec3, signed_volume};

/// Size and bounds of a loaded mesh.
pub struct MeshStats {
    pub vertices: usize,
    pub faces: usize,
    pub min: Vec3,
    pub max: Vec3,
    pub surface_area: Scalar,
    pub volume: Scalar,
}

impl MeshStats {
    /// Returns `None` for a mesh with no vertices.
    pub fn of(mesh: &Mesh) -> Option<Self> {
        let bounds = mesh.aabb()?;
        Some(Self {
            vertices: mesh.vertices.len(),
            faces: mesh.faces.len(),
            min: bounds.mins,
            max: bounds.maxs,
            surface_area: mesh.triangles().map(|t| t.area()).sum(),
            volume: signed_volume(mesh),
        })
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_of_a_unit_right_triangle() {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        let stats = MeshStats::of(&mesh).expect("non-empty mesh");

        assert_eq!((stats.vertices, stats.faces), (3, 1));
        assert!((stats.surface_area - 0.5).abs() < 1e-6);
        assert_eq!(stats.size(), Vec3::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn empty_mesh_has_no_stats() {
        assert!(MeshStats::of(&Mesh::default()).is_none());
    }
}
