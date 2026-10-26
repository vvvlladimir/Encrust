//! The path a camera flies between two views, zoomed out over a long way and back in, as
//! van Wijk and Nuij derive it (2003); see `docs/design/viewport.md`.

use core_geometry::Vec3;

use super::OrbitCamera;

/// How far the path zooms out for its length: van Wijk and Nuij's ρ, at their √2.
const RHO: f32 = std::f32::consts::SQRT_2;

/// Below this the target barely moves, and only the width changes.
const STILL_MM: f32 = 1e-3;

/// Seconds a flight takes per unit of its path, and the least and most it takes: the
/// paper's pace in seconds is too slow for a click.
const SECONDS_PER_UNIT: f64 = 0.35;
const SHORTEST_S: f64 = 0.35;
const LONGEST_S: f64 = 1.2;

/// The camera's target and orbit distance `share` of the way along the path.
pub(super) fn along(from: &OrbitCamera, to: &OrbitCamera, share: f32) -> (Vec3, f32) {
    let path = Path::between(from, to);
    let (moved, width) = path.at(share * path.length);
    let target = from.target + (to.target - from.target) * moved;
    (target, width / path.width_per_mm)
}

/// How long a flight between the two views takes, seconds.
pub(super) fn seconds(from: &OrbitCamera, to: &OrbitCamera) -> f64 {
    let length = f64::from(Path::between(from, to).length);
    (length * SECONDS_PER_UNIT).clamp(SHORTEST_S, LONGEST_S)
}

/// One flight in the paper's terms: widths w0 to w1 across a distance d.
struct Path {
    w0: f32,
    w1: f32,
    d: f32,
    /// r0 of the paper, where `d` is not zero.
    r0: f32,
    /// The path's length S.
    length: f32,
    /// The view's width per millimetre of orbit distance.
    width_per_mm: f32,
}

impl Path {
    fn between(from: &OrbitCamera, to: &OrbitCamera) -> Self {
        let width_per_mm = 2.0 * (from.fov_y_rad / 2.0).tan();
        let (w0, w1) = (
            from.distance_mm * width_per_mm,
            to.distance_mm * width_per_mm,
        );
        let d = from.target.distance(to.target);
        if d < STILL_MM {
            let length = (w1 / w0).ln().abs() / RHO;
            return Self {
                w0,
                w1,
                d,
                r0: 0.0,
                length,
                width_per_mm,
            };
        }
        let rho2 = RHO * RHO;
        let b = |w: f32, sign: f32| {
            (w1 * w1 - w0 * w0 + sign * rho2 * rho2 * d * d) / (2.0 * w * rho2 * d)
        };
        let r = |b: f32| ((b * b + 1.0).sqrt() - b).ln();
        let (r0, r1) = (r(b(w0, 1.0)), r(b(w1, -1.0)));
        Self {
            w0,
            w1,
            d,
            r0,
            length: (r1 - r0) / RHO,
            width_per_mm,
        }
    }

    /// The share of the distance covered and the view's width, `s` along the path.
    fn at(&self, s: f32) -> (f32, f32) {
        if self.d < STILL_MM {
            let share = if self.length > 0.0 {
                s / self.length
            } else {
                1.0
            };
            return (share, self.w0 * (self.w1 / self.w0).powf(share));
        }
        let (cosh, sinh) = (self.r0.cosh(), self.r0.sinh());
        let moved = self.w0 / (RHO * RHO * self.d) * (cosh * (RHO * s + self.r0).tanh() - sinh);
        (moved, self.w0 * cosh / (RHO * s + self.r0).cosh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(target: Vec3, distance_mm: f32) -> OrbitCamera {
        OrbitCamera {
            target,
            distance_mm,
            ..OrbitCamera::default()
        }
    }

    #[test]
    fn a_flight_starts_and_ends_on_its_two_views() {
        let (from, to) = (at(Vec3::ZERO, 50.0), at(Vec3::new(200.0, 0.0, 0.0), 20.0));
        let (start, start_mm) = along(&from, &to, 0.0);
        let (end, end_mm) = along(&from, &to, 1.0);
        assert!(start.length() < 1e-3 && (start_mm - 50.0).abs() < 1e-2);
        assert!(
            (end - to.target).length() < 1e-2 && (end_mm - 20.0).abs() < 1e-2,
            "ends at 200 mm, 20 mm back; got {end} at {end_mm}"
        );
    }

    #[test]
    fn a_long_flight_zooms_out_on_its_way_and_a_short_one_hardly_does() {
        let near = at(Vec3::ZERO, 20.0);
        let (_, far_mid) = along(&near, &at(Vec3::new(400.0, 0.0, 0.0), 20.0), 0.5);
        let (_, near_mid) = along(&near, &at(Vec3::new(2.0, 0.0, 0.0), 20.0), 0.5);
        assert!(
            far_mid > 100.0,
            "400 mm away pulls back past 100 mm; got {far_mid}"
        );
        assert!(
            near_mid < 21.0,
            "2 mm away stays about 20 mm back; got {near_mid}"
        );
    }

    #[test]
    fn zooming_in_place_scales_evenly_and_takes_no_longer_than_the_longest() {
        let (from, to) = (at(Vec3::ZERO, 100.0), at(Vec3::ZERO, 1.0));
        let (target, mid_mm) = along(&from, &to, 0.5);
        assert!(target.length() < 1e-6);
        assert!(
            (mid_mm - 10.0).abs() < 1e-2,
            "halfway in scale is 10 mm; got {mid_mm}"
        );
        assert!(seconds(&from, &to) <= LONGEST_S);
        assert!(seconds(&from, &from) >= SHORTEST_S);
    }
}
