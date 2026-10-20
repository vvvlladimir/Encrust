use core_geometry::{Bvh, ClosestPoint, Mesh, Scalar, Vec3};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error::VolumeError;

/// Which faces a relief leaves where they are: those turned within `bottom_deg` of straight
/// down or within `top_deg` of straight up. Zero holds none on that side.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AngleMask {
    /// Straight up in the space of the mesh pressed: the plate's +Z taken back through
    /// the model's turn.
    pub up: Vec3,
    pub bottom_deg: Scalar,
    pub top_deg: Scalar,
}

impl Default for AngleMask {
    fn default() -> Self {
        Self {
            up: Vec3::Z,
            bottom_deg: 0.0,
            top_deg: 0.0,
        }
    }
}

impl AngleMask {
    pub(super) fn check(&self) -> Result<(), VolumeError> {
        for angle in [self.bottom_deg, self.top_deg] {
            if !(0.0..=90.0).contains(&angle) {
                return Err(VolumeError::BadMaskAngle(angle));
            }
        }
        match self.up.try_normalize() {
            Some(_) => Ok(()),
            None => Err(VolumeError::BadMaskUp(self.up)),
        }
    }

    fn holds(&self, normal: Vec3, up: Vec3) -> bool {
        let within = |limit_deg: Scalar, toward: Vec3| {
            limit_deg > 0.0 && normal.dot(toward) >= limit_deg.to_radians().cos()
        };
        within(self.bottom_deg, -up) || within(self.top_deg, up)
    }
}

/// Two nearest points this much apart are one, millimetres: a lattice point as near a held
/// face as a moved one is held, so a relief never leaks out along the edge between them.
const TIE_MM: Scalar = 1e-4;

/// The faces a mask holds, with a hierarchy of their own to find the nearest of them.
pub(super) struct Held {
    mesh: Mesh,
    bvh: Bvh,
}

impl Held {
    /// `None` when the mask holds no face of `mesh`.
    pub(super) fn of(mesh: &Mesh, mask: &AngleMask) -> Option<Self> {
        let up = mask.up.try_normalize()?;
        let faces: Vec<[u32; 3]> = (0..mesh.faces.len())
            .into_par_iter()
            .filter(|&face| {
                mesh.triangle(face)
                    .and_then(|triangle| triangle.normal_unnormalized().try_normalize())
                    .is_some_and(|normal| mask.holds(normal, up))
            })
            .map(|face| mesh.faces[face])
            .collect();
        if faces.is_empty() {
            return None;
        }
        let mesh = Mesh::new(mesh.vertices.clone(), faces);
        let bvh = Bvh::build(&mesh);
        Some(Self { mesh, bvh })
    }

    pub(super) fn nearest(&self, point: Vec3) -> Option<ClosestPoint> {
        self.bvh.closest(point)
    }

    /// Whether a point whose nearest held point is `held` and whose nearest point of the
    /// whole model is `nearest` belongs to the held face, and so does not move.
    pub(super) fn is_held(held: &ClosestPoint, nearest: &ClosestPoint) -> bool {
        held.distance <= nearest.distance + TIE_MM
    }

    /// `moved`, the field at `point` once the relief moved it, cut off at the plane of
    /// `held` where the point lies past that face's edge.
    ///
    /// Without it a relief pressed out of a side would wrap round the edge it shares with a
    /// held bottom and hang below it. Directly over a held face is left alone: there the
    /// point is in the open beneath an overhang, and the wall beside it keeps its relief.
    pub(super) fn clipped(&self, moved: Scalar, point: Vec3, held: &ClosestPoint) -> Scalar {
        let Some(normal) = self
            .mesh
            .triangle(held.face)
            .and_then(|triangle| triangle.normal_unnormalized().try_normalize())
        else {
            return moved;
        };
        let off = point - held.point;
        let across = off.dot(normal);
        let lateral = (off - normal * across).length();
        // Both sides of the plane, so the lattice interpolates the cut onto it.
        if lateral > TIE_MM {
            moved.max(across)
        } else {
            moved
        }
    }
}
