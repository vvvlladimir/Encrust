//! Where an object stands on the plate: moved, turned or sized about its centre of mass and
//! stood on the plate again.

use core_geometry::{
    Mesh, Quat, Transform, Vec3, center_of_mass, drop_to_plate, glam::EulerRot, transform_mesh,
};

/// The turn of `angles` degrees about X, then Y, then Z of the plate.
pub fn euler_degrees(angles: Vec3) -> Quat {
    let radians = angles * (std::f32::consts::PI / 180.0);
    Quat::from_rotation_z(radians.z)
        * Quat::from_rotation_y(radians.y)
        * Quat::from_rotation_x(radians.x)
}

/// `transform` with its translation moved by `by_mm`.
pub fn moved(transform: Transform, by_mm: Vec3) -> Transform {
    Transform {
        translation: transform.translation + by_mm,
        ..transform
    }
}

/// `transform` with its rotation set to the turn of `degrees`, about the centre of mass
/// of `mesh` (the object's own) where it stands, and the object stood on the plate.
pub fn turned_to(transform: Transform, mesh: &Mesh, degrees: Vec3) -> Transform {
    let turned = Transform {
        rotation: euler_degrees(degrees),
        ..transform
    };
    settled(transform, turned, mesh)
}

/// `transform` turned to stand at `rotation`, about the centre of mass of `mesh` and the
/// object stood on the plate.
pub fn oriented_to(transform: Transform, mesh: &Mesh, rotation: Quat) -> Transform {
    let by = rotation * transform.rotation.inverse();
    settled(transform, transform.turned(by), mesh)
}

/// The angles of `rotation` about X, then Y, then Z of the plate, degrees, as `Rotate` takes
/// them; zero is added to each, so float dust never reads as a turn of minus nothing.
pub fn degrees_of(rotation: Quat) -> Vec3 {
    let (x, y, z) = rotation.to_euler(EulerRot::XYZEx);
    Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees()).round() + 0.0
}

/// `transform` with its scale set to `factors`, about the centre of mass of `mesh` and the
/// object stood on the plate; `None` for a factor that is not finite or not above zero.
pub fn sized_to(transform: Transform, mesh: &Mesh, factors: Vec3) -> Option<Transform> {
    if !factors.to_array().iter().all(|f| f.is_finite() && *f > 0.0) {
        return None;
    }
    let sized = Transform {
        scale: factors,
        ..transform
    };
    Some(settled(transform, sized, mesh))
}

/// `after` moved so the centre of mass stays where `before` had it, then stood on z = 0.
fn settled(before: Transform, after: Transform, mesh: &Mesh) -> Transform {
    let Some(centre) = center_of_mass(mesh) else {
        return after;
    };
    let stayed =
        before.to_matrix().transform_point3(centre) - after.to_matrix().transform_point3(centre);
    let after = moved(after, stayed);
    match transform_mesh(mesh, after).aabb() {
        Some(bounds) => moved(after, drop_to_plate(&bounds)),
        None => after,
    }
}
