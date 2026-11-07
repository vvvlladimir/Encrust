//! Holes drilled at points named as fractions of a model's placed box, each moved to the
//! nearest face and drilled there as a click drills one: a recipe's `at = "points"`.

use core_geometry::{Scalar, Vec3};
use core_volume::{HoleSize, ModelHollow};

use crate::scene::SceneObject;

/// Drills a hole of `size` at each of `points`, `[x, y, z]` fractions of `object`'s placed
/// box (`[0.5, 0.5, 0]` the middle of its floor), on the face nearest each.
pub fn drill_at_fractions(object: &mut SceneObject, points: &[[Scalar; 3]], size: HoleSize) {
    let Some(bounds) = object.world_bounds() else {
        return;
    };
    let Some(into_model) = object.transform.inverse_matrix() else {
        return;
    };
    let to_plate = object.transform.to_matrix();
    for fraction in points {
        let at = bounds.mins + Vec3::from_array(*fraction) * (bounds.maxs - bounds.mins);
        let Some(near) = object.bvh.closest(into_model.transform_point3(at)) else {
            continue;
        };
        let Some(face) = object.mesh.triangle(near.face) else {
            continue;
        };
        let normal = object.world_normal(face.normal_unnormalized());
        let point = to_plate.transform_point3(near.point);
        let (mesh, bvh, transform) = (&object.mesh, &object.bvh, object.transform);
        if let Some(hole) = ModelHollow::drain_at(mesh, bvh, point, normal, size, transform) {
            object.hollow.add_hole(hole);
        }
    }
}
