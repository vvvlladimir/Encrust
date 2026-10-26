//! Bringing the camera to a spot on a model: framed with room round it, seen from the side
//! it faces and not through another model; see `docs/design/viewport.md`.

use core_geometry::Vec3;

use super::{MIN_DISTANCE_MM, OrbitCamera};
use crate::highlight::Spot;

/// The share of the view's narrower side a spot is framed to fill: the rest shows what it
/// lies in, so a hole reads as a hole in the model rather than as a shape of its own.
const SPOT_SHARE: f32 = 0.35;

/// The least radius framed, mm: a spot of one face still shows the faces round it.
const LEAST_FRAMED_MM: f32 = 3.0;

/// How squarely the view must already face a spot to keep its angle, as the cosine of
/// the angle between them: within 45° the camera only moves, and the user keeps their way.
const FACING_COS: f32 = std::f32::consts::FRAC_1_SQRT_2;

impl OrbitCamera {
    /// This camera brought to `spot` in a view `aspect` wide for each unit high, from the
    /// first of [`sides`] `hidden(eye, point)` does not block, or the first of them.
    pub fn focusing(&self, spot: &Spot, aspect: f32, hidden: impl Fn(Vec3, Vec3) -> bool) -> Self {
        let radius = (spot.radius_mm / SPOT_SHARE).max(LEAST_FRAMED_MM);
        let half_y = self.fov_y_rad / 2.0;
        let half = half_y.min((half_y.tan() * aspect.max(1e-3)).atan());
        // A sphere touching the frustum stands r / sin back; without perspective the view's
        // half height at the target is distance × tan, so r / tan frames it.
        let distance_mm = match self.orthographic {
            true => radius / half.tan(),
            false => radius / half.sin(),
        }
        .max(MIN_DISTANCE_MM);
        let now = (self.eye() - self.target).normalize_or(Vec3::Z);
        let sides = sides(now, spot.normal);
        let side = sides
            .iter()
            .copied()
            .find(|side| !hidden(spot.center + *side * distance_mm, spot.center))
            .unwrap_or(sides[0]);
        Self {
            target: spot.center,
            distance_mm,
            ..*self
        }
        .seen_from(side)
    }
}

/// Tilts off a spot's own side tried when that is blocked, radians: still well outside the
/// surface, so the camera never looks at the spot from inside the model.
const TILTS_RAD: [f32; 2] = [std::f32::consts::FRAC_PI_6, std::f32::consts::FRAC_PI_3];

/// Turns round the spot's side at each tilt, the first towards up.
const TURNS: usize = 6;

/// The sides to look from, best first. With a side of its own, a spot is seen from it, or
/// within 45° of it the way the camera looks now, then from rings tilted round it, the one
/// above first, and last the way it looks now if that is still outside; never from behind.
/// Without, the way it looks now, then the axes and diagonals nearest to that.
fn sides(now: Vec3, normal: Vec3) -> Vec<Vec3> {
    if normal == Vec3::ZERO {
        let mut round: Vec<Vec3> = [-1.0f32, 0.0, 1.0]
            .into_iter()
            .flat_map(|x| [-1.0f32, 0.0, 1.0].map(|y| (x, y)))
            .flat_map(|(x, y)| [-1.0f32, 0.0, 1.0].map(|z| Vec3::new(x, y, z)))
            .filter(|side| *side != Vec3::ZERO)
            .map(Vec3::normalize)
            .collect();
        round.sort_by(|a, b| b.dot(now).total_cmp(&a.dot(now)));
        return [vec![now], round].concat();
    }
    let facing = now.dot(normal) >= FACING_COS;
    let mut sides = vec![if facing { now } else { normal }];
    let up = (Vec3::Z - normal * normal.dot(Vec3::Z))
        .try_normalize()
        .unwrap_or_else(|| normal.any_orthonormal_vector());
    let across = normal.cross(up);
    for tilt in TILTS_RAD {
        for turn in 0..TURNS {
            let angle = std::f32::consts::TAU * turn as f32 / TURNS as f32;
            let off = up * angle.cos() + across * angle.sin();
            sides.push(normal * tilt.cos() + off * tilt.sin());
        }
    }
    if facing {
        sides.insert(1, normal);
    } else if now.dot(normal) > 0.0 {
        sides.push(now);
    }
    sides
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot(normal: Vec3) -> Spot {
        Spot {
            center: Vec3::new(10.0, 20.0, 30.0),
            radius_mm: 3.5,
            normal,
        }
    }

    fn looks_along(camera: &OrbitCamera) -> Vec3 {
        (camera.eye() - camera.target).normalize()
    }

    #[test]
    fn a_spot_is_framed_at_the_distance_its_sphere_touches_the_narrower_side() {
        let camera = OrbitCamera::default().focusing(&spot(Vec3::Z), 2.0, |_, _| false);
        let radius = 3.5 / SPOT_SHARE;
        let expected = radius / (camera.fov_y_rad / 2.0).sin();
        assert_eq!(camera.target, spot(Vec3::Z).center);
        assert!(
            (camera.distance_mm - expected).abs() < 1e-3,
            "a wide view frames by its height, {expected} mm back; got {}",
            camera.distance_mm
        );
        let tall = OrbitCamera::default().focusing(&spot(Vec3::Z), 0.5, |_, _| false);
        assert!(
            tall.distance_mm > camera.distance_mm,
            "a tall view by its width"
        );
    }

    #[test]
    fn a_spot_facing_away_is_seen_face_on() {
        let camera = OrbitCamera::default().focusing(&spot(Vec3::X), 1.0, |_, _| false);
        assert!(
            (looks_along(&camera) - Vec3::X).length() < 1e-3,
            "seen from +X; got {}",
            looks_along(&camera)
        );
    }

    #[test]
    fn a_spot_the_view_already_faces_keeps_the_angle() {
        let before = OrbitCamera::default();
        let tilted = (looks_along(&before) + Vec3::X * 0.3).normalize();
        let camera = before.focusing(&spot(tilted), 1.0, |_, _| false);
        assert!(
            (looks_along(&camera) - looks_along(&before)).length() < 1e-3,
            "within 45° the camera only moves"
        );
    }

    #[test]
    fn a_side_another_model_blocks_is_passed_over_for_one_tilted_up_off_it() {
        let blocked_from_x = |eye: Vec3, point: Vec3| (eye - point).normalize().x > 0.9;
        let camera = OrbitCamera::default().focusing(&spot(Vec3::X), 1.0, blocked_from_x);
        let along = looks_along(&camera);
        assert!(along.x < 0.9, "not from straight +X; got {along}");
        assert!(
            along.z > 0.45,
            "tilted 30° up off it, the next side tried; got {along}"
        );
    }

    #[test]
    fn a_spot_is_never_seen_from_behind_its_own_side() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let only_behind = |eye: Vec3, point: Vec3| (eye - point).normalize().dot(normal) > -0.5;
        let camera = OrbitCamera::default().focusing(&spot(normal), 1.0, only_behind);
        assert!(
            (looks_along(&camera) - normal).length() < 1e-3,
            "every side outside is blocked, so it takes its own and not the open one inside"
        );
        for side in sides(-normal, normal) {
            assert!(side.dot(normal) > 0.4, "{side} looks from inside");
        }
    }

    #[test]
    fn a_spot_with_no_side_is_seen_from_the_nearest_open_way_round() {
        let before = OrbitCamera::default();
        let now = looks_along(&before);
        let not_now = |eye: Vec3, point: Vec3| (eye - point).normalize().dot(now) > 0.99;
        let camera = before.focusing(&spot(Vec3::ZERO), 1.0, not_now);
        let along = looks_along(&camera);
        assert!(along.dot(now) < 0.99, "the way it looked is blocked");
        assert!(along.dot(now) > 0.5, "the next is close to it; got {along}");
    }

    #[test]
    fn a_spot_with_no_side_keeps_the_angle_and_a_tiny_one_still_shows_round_it() {
        let before = OrbitCamera::default();
        let tiny = Spot {
            radius_mm: 0.01,
            ..spot(Vec3::ZERO)
        };
        let camera = before.focusing(&tiny, 1.0, |_, _| false);
        assert!((looks_along(&camera) - looks_along(&before)).length() < 1e-3);
        let least = LEAST_FRAMED_MM / (camera.fov_y_rad / 2.0).sin();
        assert!((camera.distance_mm - least).abs() < 1e-3);
    }
}
