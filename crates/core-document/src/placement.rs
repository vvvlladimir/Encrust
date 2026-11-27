//! Where an object stands on the plate: moved, turned or sized about its centre of mass and
//! stood on the plate again.

use core_geometry::{
    Aabb, Mesh, Quat, Scalar, Transform, Vec2, Vec3, center_of_mass, drop_to_plate, glam::EulerRot,
    transform_mesh,
};

/// The turn of `angles` degrees about X, then Y, then Z of the plate.
pub fn euler_degrees(angles: Vec3) -> Quat {
    let radians = angles * (std::f32::consts::PI / 180.0);
    Quat::from_rotation_z(radians.z)
        * Quat::from_rotation_y(radians.y)
        * Quat::from_rotation_x(radians.x)
}

/// How far a model whose box is `bounds` (plate mm) moves to stand `lift_mm` over the raft it
/// stands `raft_lift_mm` above, as the window's Lift row stands it.
pub fn lift_by(bounds: &Aabb, raft_lift_mm: Scalar, lift_mm: Scalar) -> Vec3 {
    Vec3::new(0.0, 0.0, lift_mm - (bounds.mins.z - raft_lift_mm))
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

/// Where a model stands once turned: the plate it must stay on and the lift of a pose not set
/// on it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Footing {
    /// The usable area as its corner and its size, mm; none lets the model slide off.
    pub area_mm: Option<([Scalar; 2], [Scalar; 2])>,
    /// How far a pose not set on the plate stands over it, mm.
    pub lift_mm: Scalar,
}

/// `transform` turned to `rotation` over the middle of its box, slid into `footing`'s area,
/// dropped onto the plate and lifted by `footing` where `on_plate` is false.
pub fn stood_turned(
    transform: Transform,
    mesh: &Mesh,
    rotation: Quat,
    (footing, on_plate): (Footing, bool),
) -> Transform {
    let turned = transform.turned(rotation * transform.rotation.inverse());
    let (Some(before), Some(after)) = (
        transform_mesh(mesh, transform).aabb(),
        transform_mesh(mesh, turned).aabb(),
    ) else {
        return turned;
    };
    let middle = |bounds: &Aabb| ((bounds.mins + bounds.maxs) / 2.0).truncate();
    let shift = middle(&before) - middle(&after);
    let mut by = shift.extend(0.0) + drop_to_plate(&after);
    if let Some((low, plate)) = footing.area_mm {
        let min = after.mins.truncate() + shift - Vec2::from_array(low);
        let span = (after.maxs - after.mins).truncate();
        by += onto_plate(min, span, Vec2::from_array(plate)).extend(0.0);
    }
    if !on_plate {
        by.z += footing.lift_mm;
    }
    moved(turned, by)
}

/// How far a grid at `min` spanning `span` moves to stand on a plate `plate` across: its near
/// corner onto the plate's corner where it is too big.
pub fn onto_plate(min: Vec2, span: Vec2, plate: Vec2) -> Vec2 {
    let axis = |min: Scalar, span: Scalar, plate: Scalar| {
        if span >= plate {
            return -min;
        }
        (0.0 as Scalar).clamp(-min, plate - span - min)
    };
    Vec2::new(axis(min.x, span.x, plate.x), axis(min.y, span.y, plate.y))
}

/// The angles of `rotation` about X, then Y, then Z of the plate, degrees unrounded, as `Rotate`
/// replays them; zero is added to each, so a turn of none never reads as minus nothing.
pub fn degrees_of(rotation: Quat) -> Vec3 {
    let (x, y, z) = rotation.to_euler(EulerRot::XYZEx);
    Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees()) + 0.0
}

/// `transform` with its scale set to `factors`, a mirror kept, about the centre of mass of `mesh`
/// and the object stood on the plate; `None` for a factor not finite or not above zero.
pub fn sized_to(transform: Transform, mesh: &Mesh, factors: Vec3) -> Option<Transform> {
    if !factors.to_array().iter().all(|f| f.is_finite() && *f > 0.0) {
        return None;
    }
    let sized = Transform {
        scale: factors * transform.scale.signum(),
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

#[cfg(test)]
mod tests {
    use core_geometry::cuboid;

    use super::*;

    #[test]
    fn a_lift_is_measured_over_the_raft_the_model_stands_on() {
        let bar = cuboid(Vec3::new(0.0, 0.0, 6.0), Vec3::new(10.0, 10.0, 30.0));
        let bounds = bar.aabb().expect("a bar has faces");
        let by = lift_by(&bounds, 1.0, 4.0);
        assert!(
            (by.z - -1.0).abs() < 1e-6,
            "6 mm up on a 1 mm raft lift is 5 mm over it, so 4 mm moves it down 1, got {}",
            by.z
        );
    }

    #[test]
    fn the_angles_of_a_turn_are_reported_to_a_fraction_of_a_degree() {
        let turn = Vec3::new(30.3, -12.4, 0.2);

        let read = degrees_of(euler_degrees(turn));

        assert!(
            read.abs_diff_eq(turn, 1e-3),
            "(30.3, -12.4, 0.2) degrees replay as themselves, got {read}"
        );
    }

    #[test]
    fn a_pose_not_set_on_the_plate_stands_at_the_lift() {
        let bar = cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 30.0));
        let footing = Footing {
            area_mm: None,
            lift_mm: 4.0,
        };

        let lifted = stood_turned(Transform::default(), &bar, Quat::IDENTITY, (footing, false));
        let seated = stood_turned(Transform::default(), &bar, Quat::IDENTITY, (footing, true));

        let low = |t: Transform| {
            transform_mesh(&bar, t)
                .aabb()
                .expect("a bar has faces")
                .mins
                .z
        };
        assert!(
            (low(lifted) - 4.0).abs() < 1e-3,
            "lifted 4 mm, got {}",
            low(lifted)
        );
        assert!(
            low(seated).abs() < 1e-3,
            "set on the plate, got {}",
            low(seated)
        );
    }
}
