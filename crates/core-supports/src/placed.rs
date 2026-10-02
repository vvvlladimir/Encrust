use core_geometry::{Bvh, Mesh, Transform};

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

    /// Whether a face of the model has been painted out of bounds.
    pub fn blocks_face(&self, face: usize) -> bool {
        self.blocked
            .is_some_and(|blocked| blocked.contains_face(face))
    }
}
