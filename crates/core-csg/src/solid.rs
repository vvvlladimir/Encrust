use std::panic::{self, AssertUnwindSafe};

use boolmesh::prelude::{Manifold, OpType, compute_boolean};
use core_geometry::{Aabb, Mesh, Vec3};

use crate::error::CsgError;
use crate::seams::{Unzipped, unzipped};

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

    /// `mesh` read as a solid. It may be several closed shells, touching along an edge or
    /// at a vertex but crossing none; vertices on the same exact point are taken as one.
    pub fn new(mesh: &Mesh) -> Result<Self, CsgError> {
        let Unzipped { positions, indices } = unzipped(mesh)?;
        // Welding can leave no face at all, which the engine would refuse as not solid.
        if indices.is_empty() {
            return Ok(Self::empty());
        }
        guarded(|| Manifold::new(&positions, &indices))
            .map(|inner| Self { inner: Some(inner) })
            .map_err(CsgError::NotSolid)
    }

    /// Whether the solid holds nothing.
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

    /// The box the solid fills, or `None` for the empty solid.
    pub fn aabb(&self) -> Option<Aabb> {
        let bounds = &self.inner.as_ref()?.bb;
        let (low, high) = (bounds.min, bounds.max);
        Some(Aabb::new(
            Vec3::new(low.x as f32, low.y as f32, low.z as f32),
            Vec3::new(high.x as f32, high.y as f32, high.z as f32),
        ))
    }
}

fn combined(left: &Manifold, right: &Manifold, op: OpType) -> Result<Solid, CsgError> {
    match guarded(|| compute_boolean(left, right, op)) {
        Ok(inner) => Ok(Solid { inner: Some(inner) }),
        Err(reason) if reason == EMPTY => Ok(Solid::empty()),
        Err(reason) => Err(CsgError::Failed(reason)),
    }
}

/// The engine's answer, or its panic as an error: it indexes past its own tables on some
/// inputs rather than refusing them.
fn guarded<T>(run: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(run))
        .unwrap_or_else(|_| Err("the boolean engine panicked".to_owned()))
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

    fn touching(min: [f32; 3], max: [f32; 3]) -> Solid {
        let mut both = cuboid(Vec3::ZERO, Vec3::ONE);
        both.append(&cuboid(Vec3::from(min), Vec3::from(max)));
        Solid::new(&both).expect("two cubes that only touch are a solid")
    }

    #[test]
    fn cubes_touching_along_an_edge_are_one_solid_the_engine_can_cut() {
        // The unit cube and its diagonal neighbour share the edge x = y = 1, used four times.
        let pair = touching([1.0, 1.0, 0.0], [2.0, 2.0, 1.0]);
        near(volume(&pair), 2.0, "two unit cubes");
        let slab = block([-1.0, -1.0, 0.5], [3.0, 3.0, 2.0]);
        near(
            volume(&pair.difference(&slab).expect("difference")),
            1.0,
            "the lower half of each",
        );
    }

    #[test]
    fn cubes_touching_at_a_corner_are_one_solid_the_engine_can_cut() {
        let pair = touching([1.0; 3], [2.0; 3]);
        near(volume(&pair), 2.0, "two unit cubes");
        let slab = block([-1.0, -1.0, 0.5], [3.0, 3.0, 1.5]);
        near(
            volume(&pair.intersection(&slab).expect("intersection")),
            1.0,
            "half of each",
        );
    }

    #[test]
    fn an_open_mesh_is_not_a_solid() {
        let mut open = cuboid(Vec3::ZERO, Vec3::ONE);
        open.faces.truncate(10);
        assert!(matches!(Solid::new(&open), Err(CsgError::NotSolid(_))));
    }

    #[test]
    fn a_mesh_whose_faces_all_weld_away_is_the_empty_solid() {
        let point = Vec3::splat(1.0);
        let flat = Mesh::new(vec![point, point, Vec3::ZERO], vec![[0, 1, 2]]);
        let solid = Solid::new(&flat).expect("nothing left is the empty solid");
        assert!(solid.is_empty());
    }
}
