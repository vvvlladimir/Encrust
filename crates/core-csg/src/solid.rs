use boolmesh::prelude::{Manifold, OpType, compute_boolean};
use core_geometry::{Mesh, Vec3};

use crate::error::CsgError;

/// What the engine answers when a boolean leaves nothing behind; it says so in its own
/// tests, which is the only place the case is stated.
const EMPTY: &str = "empty pos matrix";

/// A closed mesh readied for booleans, or the empty solid.
///
/// Combining never changes either side; each operation answers a new solid, so one
/// cavity can be cut by several lattices in turn.
#[derive(Clone)]
pub struct Solid {
    inner: Option<Manifold>,
}

impl std::fmt::Debug for Solid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Solid")
            .field("faces", &self.inner.as_ref().map_or(0, |inner| inner.nf))
            .finish()
    }
}

impl Solid {
    /// The solid nothing is in.
    pub fn empty() -> Self {
        Self { inner: None }
    }

    /// `mesh` read as a solid. It may be several closed shells, but none may cross another
    /// or itself; vertices on the same exact point are taken as one.
    pub fn new(mesh: &Mesh) -> Result<Self, CsgError> {
        if mesh.faces.is_empty() {
            return Ok(Self::empty());
        }
        let positions: Vec<f64> = mesh
            .vertices
            .iter()
            .flat_map(|vertex| vertex.to_array().map(f64::from))
            .collect();
        let indices: Vec<usize> = mesh
            .faces
            .iter()
            .flat_map(|face| face.map(|index| index as usize))
            .collect();
        Manifold::new(&positions, &indices)
            .map(|inner| Self { inner: Some(inner) })
            .map_err(CsgError::NotSolid)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_none()
    }

    /// Everything either solid holds.
    pub fn union(&self, other: &Self) -> Result<Self, CsgError> {
        match (&self.inner, &other.inner) {
            (None, _) => Ok(other.clone()),
            (_, None) => Ok(self.clone()),
            (Some(left), Some(right)) => combined(left, right, OpType::Add),
        }
    }

    /// This solid with `other` taken out of it.
    pub fn difference(&self, other: &Self) -> Result<Self, CsgError> {
        match (&self.inner, &other.inner) {
            (None, _) => Ok(Self::empty()),
            (_, None) => Ok(self.clone()),
            (Some(left), Some(right)) => combined(left, right, OpType::Subtract),
        }
    }

    /// Only what both solids hold.
    pub fn intersection(&self, other: &Self) -> Result<Self, CsgError> {
        match (&self.inner, &other.inner) {
            (Some(left), Some(right)) => combined(left, right, OpType::Intersect),
            _ => Ok(Self::empty()),
        }
    }

    /// The solid's surface as a mesh, wound outward.
    pub fn mesh(&self) -> Mesh {
        let Some(inner) = &self.inner else {
            return Mesh::default();
        };
        let vertices = inner
            .ps
            .iter()
            .map(|point| Vec3::new(point.x as f32, point.y as f32, point.z as f32))
            .collect();
        let faces = inner
            .hs
            .as_chunks::<3>()
            .0
            .iter()
            .map(|corners| corners.each_ref().map(|half| half.tail as u32))
            .collect();
        Mesh::new(vertices, faces)
    }
}

fn combined(left: &Manifold, right: &Manifold, op: OpType) -> Result<Solid, CsgError> {
    match compute_boolean(left, right, op) {
        Ok(inner) => Ok(Solid { inner: Some(inner) }),
        Err(reason) if reason == EMPTY => Ok(Solid::empty()),
        Err(reason) => Err(CsgError::Failed(reason)),
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{cuboid, diagnose, signed_volume};

    use super::*;

    fn block(min: [f32; 3], max: [f32; 3]) -> Solid {
        Solid::new(&cuboid(Vec3::from(min), Vec3::from(max))).expect("a box is a solid")
    }

    fn volume(solid: &Solid) -> f32 {
        let mesh = solid.mesh();
        let report = diagnose(&mesh);
        assert_eq!(report.boundary_edges, 0, "a boolean answers a closed mesh");
        signed_volume(&mesh)
    }

    fn near(actual: f32, expected: f32, why: &str) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "{why}: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn two_overlapping_cubes_combine_by_their_shared_corner() {
        // Two 2 mm cubes offset by 1 mm on every axis share a 1 mm cube.
        let (a, b) = (block([0.0; 3], [2.0; 3]), block([1.0; 3], [3.0; 3]));
        near(volume(&a.union(&b).expect("union")), 15.0, "8 + 8 - 1");
        near(volume(&a.difference(&b).expect("difference")), 7.0, "8 - 1");
        near(
            volume(&a.intersection(&b).expect("intersection")),
            1.0,
            "the shared cube",
        );
    }

    #[test]
    fn crossing_bars_on_shared_planes_are_unioned_once() {
        // Two 10 x 2 x 2 bars crossing at the origin share their top and bottom planes.
        let x = block([-5.0, -1.0, -1.0], [5.0, 1.0, 1.0]);
        let y = block([-1.0, -5.0, -1.0], [1.0, 5.0, 1.0]);
        near(
            volume(&x.union(&y).expect("union")),
            72.0,
            "40 + 40 - the 8 they share",
        );
    }

    #[test]
    fn solids_that_do_not_meet_intersect_in_nothing() {
        let apart = block([0.0; 3], [1.0; 3]).intersection(&block([2.0; 3], [3.0; 3]));
        assert!(apart.expect("an empty answer is not a failure").is_empty());
    }

    #[test]
    fn the_empty_solid_is_the_identity_of_union_and_difference() {
        let cube = block([0.0; 3], [2.0; 3]);
        near(
            volume(&cube.union(&Solid::empty()).expect("union")),
            8.0,
            "unchanged",
        );
        near(
            volume(&cube.difference(&Solid::empty()).expect("difference")),
            8.0,
            "unchanged",
        );
        assert!(
            Solid::empty()
                .difference(&cube)
                .expect("difference")
                .is_empty()
        );
        assert!(
            cube.intersection(&Solid::empty())
                .expect("intersection")
                .is_empty()
        );
    }

    #[test]
    fn an_open_mesh_is_not_a_solid() {
        let mut open = cuboid(Vec3::ZERO, Vec3::ONE);
        open.faces.truncate(10);
        assert!(matches!(Solid::new(&open), Err(CsgError::NotSolid(_))));
    }
}
