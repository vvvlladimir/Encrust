use super::{ModelSupports, next_revision};
use crate::{Blocked, Placed};
use core_geometry::{Adjacency, Mesh, Region, Scalar, Transform, Vec3};
use std::sync::Arc;

/// The painted patches as they were last drawn, and what they were built from.
#[derive(Debug, Clone, Default)]
pub(super) struct Patches {
    pub(super) painted: Option<Arc<Mesh>>,
    pub(super) blocked: Option<Arc<Mesh>>,
    pub(super) painted_of: Region,
    pub(super) blocked_of: Region,
    pub(super) transform: Transform,
}

/// Millimetres a painted patch is drawn off the surface it covers, so that it is not
/// fighting the model's own faces for the same depth.
pub(super) const PATCH_LIFT_MM: Scalar = 0.05;

impl ModelSupports {
    /// The patch painted to be supported, and the patch painted to be left alone, both in
    /// plate coordinates and both `None` until they have been built.
    pub fn patches(&self) -> (Option<&Arc<Mesh>>, Option<&Arc<Mesh>>) {
        (self.patches.painted.as_ref(), self.patches.blocked.as_ref())
    }

    /// Rebuilds the two patches' geometry if the paint or placement moved; cheap next to
    /// [`ModelSupports::refresh`], it copies painted triangles only, so a stroke shows as drawn.
    pub fn refresh_patches(&mut self, model: &Mesh, transform: Transform) {
        let fresh = self.patches.transform == transform
            && self.patches.painted_of == self.painted
            && self.patches.blocked_of == self.blocked;
        if fresh {
            return;
        }

        self.patches = Patches {
            painted: self
                .painted
                .submesh(model, transform, PATCH_LIFT_MM)
                .map(Arc::new),
            blocked: self
                .blocked
                .submesh(model, transform, PATCH_LIFT_MM)
                .map(Arc::new),
            painted_of: self.painted.clone(),
            blocked_of: self.blocked.clone(),
            transform,
        };
    }

    /// The faces painted to be supported.
    pub fn painted(&self) -> &Region {
        &self.painted
    }

    /// The faces painted to be left alone.
    pub fn blocked(&self) -> &Region {
        &self.blocked
    }

    /// Paints or unpaints the faces within `radius_mm` of `at`, in plate coordinates.
    /// `blocking` paints the patch supports keep out of rather than the one they fill.
    pub fn paint(
        &mut self,
        placed: &Placed,
        at: Vec3,
        radius_mm: Scalar,
        blocking: bool,
        marked: bool,
    ) -> usize {
        let region = if blocking {
            &mut self.blocked
        } else {
            &mut self.painted
        };
        let changed = region.brush(
            (placed.model, placed.bvh, placed.transform),
            at,
            radius_mm,
            marked,
        );
        if changed > 0 {
            self.paint_changed(blocking);
        }
        changed
    }

    /// Paints the surface the face `seed` belongs to, out to `max_angle_deg` of turn.
    pub fn paint_face(
        &mut self,
        model: &Mesh,
        adjacency: &Adjacency,
        seed: usize,
        max_angle_deg: Scalar,
        blocking: bool,
        marked: bool,
    ) -> usize {
        let region = if blocking {
            &mut self.blocked
        } else {
            &mut self.painted
        };
        let changed = region.flood(model, adjacency, seed, max_angle_deg, marked);
        if changed > 0 {
            self.paint_changed(blocking);
        }
        changed
    }

    /// Takes the paint off one of the two patches.
    pub fn clear_paint(&mut self, blocking: bool) {
        let region = if blocking {
            &mut self.blocked
        } else {
            &mut self.painted
        };
        if !region.is_empty() {
            region.clear();
            // Dropped rather than rebuilt: an object with nothing left on it is not asked
            // to refresh, so a stale patch would stay on screen.
            self.patches = Patches::default();
            self.paint_changed(blocking);
        }
    }

    /// Drops what was built from the paint, and with the blocked patch what the trees
    /// were grown from as well.
    pub(super) fn paint_changed(&mut self, blocking: bool) {
        self.built = None;
        if blocking {
            self.revision = next_revision();
        }
    }

    /// What placement has to keep out of, or `None` when nothing is painted out of
    /// bounds.
    pub fn keep_out(&self, model: &Mesh, transform: Transform) -> Option<Blocked> {
        Blocked::new(model, &self.blocked, transform)
    }
}
