use serde::{Deserialize, Serialize};

use crate::{Aabb, Mat4, Mesh, Quat, Scalar, Vec3};

/// Placement of a model on the build plate: non-uniform scale, then rotation, then translation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::default()
        }
    }

    pub fn to_matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// Applies a transform to every vertex.
///
/// A mirroring scale reverses winding order, so faces are flipped to keep normals
/// pointing outwards.
pub fn transform_mesh(mesh: &Mesh, transform: Transform) -> Mesh {
    let matrix = transform.to_matrix();
    let vertices = mesh
        .vertices
        .iter()
        .map(|v| matrix.transform_point3(*v))
        .collect();

    let mirrored = transform.scale.x * transform.scale.y * transform.scale.z < 0.0;
    let faces = mesh
        .faces
        .iter()
        .map(|f| if mirrored { [f[0], f[2], f[1]] } else { *f })
        .collect();

    Mesh::new(vertices, faces)
}

/// Translation that puts the lowest point of `bounds` on the build plate.
pub fn drop_to_plate(bounds: &Aabb) -> Vec3 {
    Vec3::new(0.0, 0.0, -bounds.mins.z)
}

/// Translation that stands `bounds` `lift_mm` clear of the plate. X and Y are untouched.
///
/// Supports need room under a part, and the layers of a part printed straight onto the
/// plate are the ones that stick to it hardest.
pub fn lift_over_plate(bounds: &Aabb, lift_mm: Scalar) -> Vec3 {
    Vec3::new(0.0, 0.0, lift_mm - bounds.mins.z)
}

/// Translation that centres `bounds` over a plate of the given size. Z is untouched.
pub fn center_over_plate(bounds: &Aabb, plate_x: Scalar, plate_y: Scalar) -> Vec3 {
    let middle = (bounds.mins + bounds.maxs) / 2.0;
    Vec3::new(plate_x / 2.0 - middle.x, plate_y / 2.0 - middle.y, 0.0)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;

    #[test]
    fn lifting_stands_a_body_clear_of_the_plate() {
        let bounds = Aabb::new(Vec3::new(-1.0, -1.0, 3.0), Vec3::new(1.0, 1.0, 9.0));
        let offset = lift_over_plate(&bounds, 5.0);
        assert!(
            (bounds.mins.z + offset.z - 5.0).abs() < 1e-6,
            "the lowest point ends up at the lift, got {}",
            bounds.mins.z + offset.z
        );
        assert!(
            offset.x.abs() < 1e-6 && offset.y.abs() < 1e-6,
            "only z moves"
        );
    }

    #[test]
    fn lifting_by_nothing_is_dropping_to_the_plate() {
        let bounds = Aabb::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(1.0, 1.0, 4.0));
        assert_eq!(lift_over_plate(&bounds, 0.0), drop_to_plate(&bounds));
    }

    #[test]
    fn default_transform_is_identity() {
        assert_eq!(Transform::default().to_matrix(), Mat4::IDENTITY);
    }

    #[test]
    fn mirroring_scale_reverses_winding() {
        let mesh = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y], vec![[0, 1, 2]]);
        let mirror = Transform {
            scale: Vec3::new(-1.0, 1.0, 1.0),
            ..Transform::default()
        };
        assert_eq!(transform_mesh(&mesh, mirror).faces, vec![[0, 2, 1]]);
        assert_eq!(
            transform_mesh(&mesh, Transform::default()).faces,
            vec![[0, 1, 2]]
        );
    }

    #[test]
    fn placement_helpers_move_the_bounds_where_asked() {
        let bounds = Aabb::new(Vec3::new(2.0, 2.0, 5.0), Vec3::new(4.0, 6.0, 9.0));
        assert_eq!(drop_to_plate(&bounds), Vec3::new(0.0, 0.0, -5.0));
        assert_eq!(
            center_over_plate(&bounds, 100.0, 60.0),
            Vec3::new(47.0, 26.0, 0.0)
        );
    }

    #[test]
    fn scale_then_rotate_then_translate() {
        let t = Transform {
            translation: Vec3::new(0.0, 0.0, 5.0),
            rotation: Quat::from_rotation_z(FRAC_PI_2),
            scale: Vec3::splat(2.0),
        };
        let moved = t.to_matrix().transform_point3(Vec3::X);
        assert!(moved.abs_diff_eq(Vec3::new(0.0, 2.0, 5.0), 1e-5));
    }
}
