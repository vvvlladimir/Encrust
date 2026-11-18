//! The faces painted as what shows, drawn as a patch over the model while they are painted.

use std::sync::Arc;

use core_geometry::{Mesh, Region, Scalar, Transform};

/// Millimetres the patch is drawn off the surface it covers, so that it does not fight the
/// model's own faces for the same depth.
const PATCH_LIFT_MM: Scalar = 0.05;

/// The patch as it was last built, and what it was built from.
#[derive(Debug, Clone, Default)]
pub struct ShownPatch {
    mesh: Option<Arc<Mesh>>,
    of: Region,
    transform: Transform,
}

impl ShownPatch {
    /// The patch in plate coordinates, `None` until it is built or once it is put away.
    pub fn mesh(&self) -> Option<&Arc<Mesh>> {
        self.mesh.as_ref()
    }

    /// Builds the patch again where the paint or the placement moved; it copies painted
    /// triangles only, so a stroke shows as it is drawn.
    pub fn refresh(&mut self, painted: &Region, model: &Mesh, transform: Transform) {
        let fresh = self.mesh.is_some() && self.of == *painted && self.transform == transform;
        if fresh || (painted.is_empty() && self.mesh.is_none()) {
            return;
        }
        *self = Self {
            mesh: painted
                .submesh(model, transform, PATCH_LIFT_MM)
                .map(Arc::new),
            of: painted.clone(),
            transform,
        };
    }

    /// Stops drawing the patch, as when the brush that paints it is out of reach.
    pub fn put_away(&mut self) {
        if self.mesh.is_some() {
            *self = Self::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid};

    use super::*;

    #[test]
    fn a_patch_is_built_from_the_painted_faces_and_put_away_on_request() {
        let model = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let painted: Region = [0, 1].into_iter().collect();
        let mut patch = ShownPatch::default();
        patch.refresh(&painted, &model, Transform::default());
        let faces = patch.mesh().map(|mesh| mesh.faces.len());
        assert_eq!(faces, Some(2), "two painted faces make a patch of two");
        patch.put_away();
        assert!(patch.mesh().is_none());
    }
}
