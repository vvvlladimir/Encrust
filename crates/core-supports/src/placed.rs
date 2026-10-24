use core_geometry::{Bvh, Mat4, Mesh, PlacedHit, Ray, Transform, Vec3, raycast_placed_by};

use crate::region::Blocked;

/// A placement this close to singular has flattened its model, which no ray then hits.
const SINGULAR: f32 = 1e-12;

/// The model a support has to live with: the mesh, its hierarchy, where it stands on the
/// plate, and the patch the user has forbidden supports on.
///
/// The four travel together because every placement question — where a column lands, what
/// a beam runs into, where a tip may be put — asks all of them at once.
#[derive(Debug, Clone, Copy)]
pub struct Placed<'a> {
    pub model: &'a Mesh,
    /// The hierarchy of `model`, built once and kept beside it.
    pub bvh: &'a Bvh,
    pub transform: Transform,
    /// Where supports are forbidden, or `None` when nothing is painted.
    pub blocked: Option<&'a Blocked>,
    /// `transform` as a matrix and its inverse, or `None` where it flattens the model.
    matrices: Option<(Mat4, Mat4)>,
}

impl<'a> Placed<'a> {
    pub fn new(model: &'a Mesh, bvh: &'a Bvh, transform: Transform) -> Self {
        let matrix = transform.to_matrix();
        Self {
            model,
            bvh,
            transform,
            blocked: None,
            matrices: (matrix.determinant().abs() >= SINGULAR).then(|| (matrix, matrix.inverse())),
        }
    }

    /// The nearest face of the model `ray`, in plate coordinates, hits: `raycast_placed`
    /// without working the placement's inverse out again for every ray.
    pub fn raycast(&self, ray: &Ray) -> Option<PlacedHit> {
        let (matrix, inverse) = self.matrices?;
        raycast_placed_by(self.model, self.bvh, matrix, inverse, ray)
    }

    /// The same model with `blocked` forbidden to supports.
    #[must_use]
    pub fn blocking(self, blocked: Option<&'a Blocked>) -> Self {
        Self { blocked, ..self }
    }

    /// Whether `point`, on the model and in plate coordinates, lies on a patch painted out
    /// of bounds. Asked of the point rather than its face, so it holds however the surface
    /// supports stand against is numbered: the model, its shell or the shell drilled.
    pub fn blocks_at(&self, point: Vec3) -> bool {
        self.blocked.is_some_and(|blocked| blocked.holds(point))
    }
}
