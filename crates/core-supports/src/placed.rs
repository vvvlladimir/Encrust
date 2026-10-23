use core_geometry::{Bvh, Mesh, Transform, Vec3};

use crate::region::Blocked;

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
}

impl<'a> Placed<'a> {
    pub fn new(model: &'a Mesh, bvh: &'a Bvh, transform: Transform) -> Self {
        Self {
            model,
            bvh,
            transform,
            blocked: None,
        }
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
