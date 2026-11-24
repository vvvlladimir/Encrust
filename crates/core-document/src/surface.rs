//! The surface point a plate point picks on a model standing on the plate.

use core_edit::ObjectId;
use core_geometry::{Bvh, Mat4, Mesh, Transform, Vec3};

use crate::error::ApplyError;

/// The point of `mesh`'s surface nearest `near` and its outward normal, plate mm, the model
/// standing under `transform` whose inverse is `into_model`.
pub(crate) fn surface_near(
    (mesh, bvh): (&Mesh, &Bvh),
    (transform, into_model): (Transform, Mat4),
    near: Vec3,
    object: ObjectId,
) -> Result<(Vec3, Vec3), ApplyError> {
    let closest =
        (bvh.closest(into_model.transform_point3(near))).ok_or(ApplyError::NoSurface(object))?;
    let face = mesh
        .triangle(closest.face)
        .ok_or(ApplyError::NoSurface(object))?;
    let normal = (transform.normal_matrix() * face.normal_unnormalized()).normalize_or_zero();
    Ok((
        transform.to_matrix().transform_point3(closest.point),
        normal,
    ))
}
