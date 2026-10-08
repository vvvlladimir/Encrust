use core_geometry::{Mesh, Scalar, Vec3, signed_volume, split};

/// Size and bounds of a loaded mesh.
pub struct MeshStats {
    pub vertices: usize,
    pub faces: usize,
    pub min: Vec3,
    pub max: Vec3,
    pub surface_area: Scalar,
    /// What the mesh encloses, cubic millimetres: one shell's own volume, or every
    /// shell's added up.
    pub volume: Scalar,
    /// Two shells that each hold material and whose boxes meet, so `volume` counts what
    /// they share twice. A cavity holds none and never raises it.
    pub shells_overlap: bool,
}

impl MeshStats {
    /// Returns `None` for a mesh with no vertices. `shells` is what `diagnose` counted,
    /// where it ran: more than one is what makes the volume worth looking into.
    pub fn of(mesh: &Mesh, shells: Option<usize>) -> Option<Self> {
        let bounds = mesh.aabb()?;
        Some(Self {
            vertices: mesh.vertices.len(),
            faces: mesh.faces.len(),
            min: bounds.mins,
            max: bounds.maxs,
            surface_area: mesh.triangles().map(|t| t.area()).sum(),
            volume: signed_volume(mesh),
            shells_overlap: shells.is_some_and(|shells| shells > 1) && overlapping(mesh),
        })
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

/// Whether any two shells that both enclose material have boxes that meet.
///
/// A box is all the test needs: the figure it qualifies is a sum, and a sum is only
/// certain to be right where nothing can overlap at all.
fn overlapping(mesh: &Mesh) -> bool {
    let solid: Vec<_> = split(mesh)
        .iter()
        .filter(|shell| signed_volume(shell) > 0.0)
        .filter_map(Mesh::aabb)
        .collect();
    solid.iter().enumerate().any(|(index, one)| {
        solid[index + 1..]
            .iter()
            .any(|other| one.mins.cmple(other.maxs).all() && other.mins.cmple(one.maxs).all())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_of_a_unit_right_triangle() {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        let stats = MeshStats::of(&mesh, None).expect("non-empty mesh");

        assert_eq!((stats.vertices, stats.faces), (3, 1));
        assert!((stats.surface_area - 0.5).abs() < 1e-6);
        assert_eq!(stats.size(), Vec3::new(1.0, 1.0, 0.0));
    }

    /// Axis-aligned box of 10 mm, twelve triangles, its lower corner at `at`.
    fn cuboid(at: Vec3) -> Mesh {
        let corner = |x: Scalar, y: Scalar, z: Scalar| at + Vec3::new(x, y, z);
        let vertices = vec![
            corner(0.0, 0.0, 0.0),
            corner(10.0, 0.0, 0.0),
            corner(10.0, 10.0, 0.0),
            corner(0.0, 10.0, 0.0),
            corner(0.0, 0.0, 10.0),
            corner(10.0, 0.0, 10.0),
            corner(10.0, 10.0, 10.0),
            corner(0.0, 10.0, 10.0),
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

    /// The meshes as one, each keeping its own vertices, so they stay separate shells.
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
    fn two_shells_in_one_another_make_the_volume_a_sum_of_overlaps() {
        let mesh = joined(&[cuboid(Vec3::ZERO), cuboid(Vec3::splat(5.0))]);
        let stats = MeshStats::of(&mesh, Some(2)).expect("two boxes");

        assert!(stats.shells_overlap, "they share a 5 mm corner");
        // 2000 mm^3 of shells over 1875 mm^3 of material: the corner is counted twice.
        assert!((stats.volume - 2000.0).abs() < 1e-2, "{}", stats.volume);
    }

    #[test]
    fn shells_that_stand_apart_add_up_to_what_they_enclose() {
        let mesh = joined(&[cuboid(Vec3::ZERO), cuboid(Vec3::splat(20.0))]);
        assert!(
            !MeshStats::of(&mesh, Some(2))
                .expect("two boxes")
                .shells_overlap
        );
    }

    #[test]
    fn a_cavity_inside_a_shell_is_no_overlap() {
        let shell = cuboid(Vec3::ZERO);
        let mut cavity = cuboid(Vec3::splat(2.0));
        cavity.vertices.iter_mut().for_each(|vertex| {
            *vertex = (*vertex - Vec3::splat(2.0)) * 0.6 + Vec3::splat(2.0);
        });
        // Wound inwards, which is what makes it hold air rather than material.
        cavity.faces.iter_mut().for_each(|face| face.swap(1, 2));

        let mesh = joined(&[shell, cavity]);
        assert!(
            !MeshStats::of(&mesh, Some(2))
                .expect("a hollow box")
                .shells_overlap,
            "a cavity subtracts itself and is counted once"
        );
    }

    #[test]
    fn empty_mesh_has_no_stats() {
        assert!(MeshStats::of(&Mesh::default(), None).is_none());
    }
}
