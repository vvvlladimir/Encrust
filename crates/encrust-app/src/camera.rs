use std::f32::consts::{FRAC_PI_2, PI, TAU};

use core_geometry::glam::camera::rh::proj::directx::{orthographic, perspective};
use core_geometry::glam::camera::rh::view::look_at_mat4;
use core_geometry::{Aabb, Mat4, Vec2, Vec3};

use crate::plate::BuildPlate;

/// Kept away from straight up and straight down, where the view direction becomes
/// parallel to the up vector and the look-at basis is undefined.
const PITCH_LIMIT_RAD: f32 = FRAC_PI_2 - 0.01;

/// Below this the near plane and the orbit centre collapse into each other.
const MIN_DISTANCE_MM: f32 = 1.0;

/// A camera that orbits a point on the build plate.
///
/// The world is Z-up, in plate millimetres. Yaw turns around the world Z axis and pitch
/// lifts the eye off the XY plane, so the horizon stays level whatever the user does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw_rad: f32,
    pub pitch_rad: f32,
    pub distance_mm: f32,
    pub fov_y_rad: f32,
    /// Whether the view is drawn without perspective, parallel lines staying parallel.
    pub orthographic: bool,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw_rad: -FRAC_PI_2,
            pitch_rad: 0.45,
            distance_mm: 300.0,
            fov_y_rad: 0.8,
            orthographic: false,
        }
    }
}

impl OrbitCamera {
    /// Position of the eye in plate coordinates.
    pub fn eye(&self) -> Vec3 {
        let (sin_pitch, cos_pitch) = self.pitch_rad.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw_rad.sin_cos();
        let direction = Vec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, sin_pitch);
        self.target + direction * self.distance_mm
    }

    pub fn view(&self) -> Mat4 {
        look_at_mat4(self.eye(), self.target, Vec3::Z)
    }

    /// Clip planes hug the orbit distance so the depth buffer keeps its precision on the
    /// model rather than spending it on empty space.
    ///
    /// Without perspective the view is as tall as the perspective one is at the target, so
    /// switching keeps what stands there the same size, and the near plane stands behind
    /// the eye: zoomed in close, a model the eye is inside is still drawn whole.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let aspect = aspect.max(1e-3);
        let far = self.distance_mm * 20.0;
        if self.orthographic {
            let half_height = self.distance_mm * (self.fov_y_rad / 2.0).tan();
            let half_width = half_height * aspect;
            return orthographic(
                -half_width,
                half_width,
                -half_height,
                half_height,
                -self.distance_mm * 10.0,
                far,
            );
        }
        let near = (self.distance_mm * 0.01).max(0.1);
        perspective(self.fov_y_rad, aspect, near, far)
    }

    /// Where a line of sight to `point` starts: the eye, or without perspective a point far
    /// back along the view axis from it, since every line of sight is parallel then.
    pub fn sight_to(&self, point: Vec3) -> Vec3 {
        if !self.orthographic {
            return self.eye();
        }
        let back = (self.eye() - self.target).normalize_or_zero();
        point + back * self.distance_mm * 10.0
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }

    /// Turns the camera around its target. Deltas are radians.
    pub fn orbit(&mut self, yaw_rad: f32, pitch_rad: f32) {
        self.yaw_rad += yaw_rad;
        self.pitch_rad = (self.pitch_rad + pitch_rad).clamp(-PITCH_LIMIT_RAD, PITCH_LIMIT_RAD);
    }

    /// Slides the target across the view plane. `delta_points` is a cursor movement and
    /// `viewport_height_points` the height it moved in, so a drag holds the same world
    /// point under the cursor at any zoom level.
    pub fn pan(&mut self, delta_points: Vec2, viewport_height_points: f32) {
        if viewport_height_points <= 0.0 {
            return;
        }

        let world_per_point =
            2.0 * self.distance_mm * (self.fov_y_rad / 2.0).tan() / viewport_height_points;
        let forward = (self.target - self.eye()).normalize_or_zero();
        let right = forward.cross(Vec3::Z).normalize_or_zero();
        let up = right.cross(forward);

        self.target += (right * -delta_points.x + up * delta_points.y) * world_per_point;
    }

    /// Multiplies the orbit distance. Values below one move the eye closer.
    pub fn zoom(&mut self, factor: f32) {
        self.distance_mm = (self.distance_mm * factor).max(MIN_DISTANCE_MM);
    }

    /// Points the camera at `bounds` from the current angle, far enough back that the
    /// whole box is inside the vertical field of view.
    pub fn frame(&mut self, bounds: &Aabb) {
        let radius = ((bounds.maxs - bounds.mins).length() / 2.0).max(MIN_DISTANCE_MM);
        self.target = (bounds.mins + bounds.maxs) / 2.0;
        self.distance_mm = (radius / (self.fov_y_rad / 2.0).sin()).max(MIN_DISTANCE_MM);
    }

    /// The same camera moved round its target so that the eye stands along `towards_eye`
    /// from it. Straight up or down keeps the current yaw, which is undefined there.
    pub fn seen_from(self, towards_eye: Vec3) -> Self {
        let direction = towards_eye.normalize_or_zero();
        let mut camera = self;
        if direction.truncate().length() > 1e-4 {
            camera.yaw_rad = direction.y.atan2(direction.x);
        }
        camera.pitch_rad = direction
            .z
            .clamp(-1.0, 1.0)
            .asin()
            .clamp(-PITCH_LIMIT_RAD, PITCH_LIMIT_RAD);
        camera
    }

    /// The camera `share` of the way from `self` to `to`, 0 to 1. The yaw takes the short
    /// way round, so a turn from just left of behind to just right of it is a small one.
    pub fn toward(&self, to: &Self, share: f32) -> Self {
        let share = share.clamp(0.0, 1.0);
        let lerp = |from: f32, to: f32| from + (to - from) * share;
        let mut yaw_turn = (to.yaw_rad - self.yaw_rad).rem_euclid(TAU);
        if yaw_turn > PI {
            yaw_turn -= TAU;
        }
        Self {
            target: self.target.lerp(to.target, share),
            yaw_rad: self.yaw_rad + yaw_turn * share,
            pitch_rad: lerp(self.pitch_rad, to.pitch_rad),
            distance_mm: lerp(self.distance_mm, to.distance_mm),
            fov_y_rad: lerp(self.fov_y_rad, to.fov_y_rad),
            orthographic: to.orthographic,
        }
    }

    /// The view an empty window opens with: the whole build volume, seen from the front.
    pub fn framing_plate(plate: &BuildPlate) -> Self {
        let mut camera = Self {
            target: plate.center(),
            ..Self::default()
        };
        camera.distance_mm = plate.diagonal_mm() / (camera.fov_y_rad / 2.0).sin() / 2.0;
        camera
    }
}

/// A swing of the camera from one view to another, eased in and out, so a click on the
/// view cube is seen to turn the plate rather than to cut to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraTurn {
    pub from: OrbitCamera,
    pub to: OrbitCamera,
    /// When it began and how long it takes, seconds on egui's clock.
    pub started_s: f64,
    pub length_s: f64,
}

impl CameraTurn {
    /// Where the camera stands at `now_s`, and whether the turn is over.
    pub fn at(&self, now_s: f64) -> (OrbitCamera, bool) {
        let share = ((now_s - self.started_s) / self.length_s.max(1e-6)).clamp(0.0, 1.0) as f32;
        let eased = share * share * (3.0 - 2.0 * share);
        (self.from.toward(&self.to, eased), share >= 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_matrix_puts_the_target_at_the_origin() {
        let camera = OrbitCamera {
            target: Vec3::new(10.0, 20.0, 5.0),
            ..OrbitCamera::default()
        };
        let seen = camera.view().transform_point3(camera.target);
        assert!(
            seen.abs_diff_eq(Vec3::new(0.0, 0.0, -camera.distance_mm), 1e-3),
            "the target sits on the view axis at the orbit distance, got {seen}"
        );
    }

    #[test]
    fn the_eye_keeps_its_distance_from_the_target() {
        let mut camera = OrbitCamera::default();
        camera.orbit(1.3, -0.7);
        let distance = (camera.eye() - camera.target).length();
        assert!((distance - camera.distance_mm).abs() < 1e-3);
    }

    #[test]
    fn orbiting_back_and_forth_is_the_identity() {
        let start = OrbitCamera::default();
        let mut camera = start;
        camera.orbit(0.9, 0.3);
        camera.orbit(-0.9, -0.3);
        assert!((camera.yaw_rad - start.yaw_rad).abs() < 1e-5);
        assert!((camera.pitch_rad - start.pitch_rad).abs() < 1e-5);
    }

    #[test]
    fn pitch_stops_short_of_the_poles() {
        let mut camera = OrbitCamera::default();
        camera.orbit(0.0, 100.0);
        assert!(camera.pitch_rad <= PITCH_LIMIT_RAD);
        camera.orbit(0.0, -200.0);
        assert!(camera.pitch_rad >= -PITCH_LIMIT_RAD);
    }

    #[test]
    fn zooming_in_never_reaches_the_target() {
        let mut camera = OrbitCamera::default();
        for _ in 0..200 {
            camera.zoom(0.5);
        }
        assert!(camera.distance_mm >= MIN_DISTANCE_MM);
    }

    #[test]
    fn panning_moves_the_target_across_the_view_plane() {
        let mut camera = OrbitCamera::default();
        let before = camera.target;
        camera.pan(Vec2::new(30.0, 0.0), 600.0);

        let forward = (camera.target - camera.eye()).normalize();
        let moved = camera.target - before;
        assert!(moved.length() > 0.0, "a horizontal drag moves the target");
        assert!(
            moved.dot(forward).abs() < 1e-4,
            "panning does not change the orbit distance along the view axis"
        );
    }

    #[test]
    fn a_framed_box_fits_inside_the_field_of_view() {
        let mut camera = OrbitCamera::default();
        let bounds = Aabb::new(Vec3::new(-5.0, -5.0, 0.0), Vec3::new(5.0, 5.0, 20.0));
        camera.frame(&bounds);

        // Every corner has to land inside the clip volume once divided through by w.
        let view_projection = camera.view_projection(1.0);
        for corner in bounding_corners(&bounds) {
            let clip = view_projection * corner.extend(1.0);
            let ndc = clip.truncate() / clip.w;
            assert!(
                ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z),
                "corner {corner} falls outside the view at {ndc}"
            );
        }
    }

    #[test]
    fn the_opening_view_looks_at_the_middle_of_the_plate() {
        let plate = BuildPlate::default();
        let camera = OrbitCamera::framing_plate(&plate);
        assert_eq!(camera.target, plate.center());
        assert!(
            camera.eye().y < camera.target.y,
            "the default view is frontal"
        );
        assert!(camera.eye().z > camera.target.z, "and slightly from above");
    }

    fn bounding_corners(bounds: &Aabb) -> [Vec3; 8] {
        let (lo, hi) = (bounds.mins, bounds.maxs);
        [
            Vec3::new(lo.x, lo.y, lo.z),
            Vec3::new(hi.x, lo.y, lo.z),
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::new(hi.x, hi.y, lo.z),
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::new(lo.x, hi.y, hi.z),
            Vec3::new(hi.x, hi.y, hi.z),
        ]
    }

    #[test]
    fn seen_from_the_front_the_eye_stands_in_front_of_the_target() {
        let camera = OrbitCamera::default().seen_from(Vec3::NEG_Y);
        let eye = camera.eye() - camera.target;
        assert!(
            eye.normalize().abs_diff_eq(Vec3::NEG_Y, 1e-5),
            "the eye is straight down -Y from the target, got {eye}"
        );
    }

    #[test]
    fn seen_from_a_corner_the_eye_stands_on_its_diagonal() {
        let corner = Vec3::new(1.0, -1.0, 1.0);
        let camera = OrbitCamera::default().seen_from(corner);
        let eye = (camera.eye() - camera.target).normalize();
        assert!(eye.abs_diff_eq(corner.normalize(), 1e-5), "got {eye}");
    }

    #[test]
    fn seen_from_above_keeps_the_yaw_and_stops_short_of_the_pole() {
        let start = OrbitCamera {
            yaw_rad: 0.7,
            ..OrbitCamera::default()
        };
        let camera = start.seen_from(Vec3::Z);
        assert_eq!(camera.yaw_rad, start.yaw_rad);
        assert_eq!(camera.pitch_rad, PITCH_LIMIT_RAD);
    }

    #[test]
    fn a_turn_starts_where_it_was_and_ends_where_it_was_going() {
        let from = OrbitCamera::default();
        let to = OrbitCamera {
            target: Vec3::new(10.0, 0.0, 0.0),
            distance_mm: 120.0,
            ..from.seen_from(Vec3::X)
        };
        let turn = CameraTurn {
            from,
            to,
            started_s: 2.0,
            length_s: 0.5,
        };
        assert_eq!(turn.at(2.0), (from, false));
        let (end, over) = turn.at(2.5);
        assert!(over);
        assert!((end.yaw_rad - to.yaw_rad).abs() < 1e-5);
        assert!((end.distance_mm - to.distance_mm).abs() < 1e-3);
        assert!(end.target.abs_diff_eq(to.target, 1e-4));
    }

    #[test]
    fn a_turn_takes_the_short_way_round() {
        let from = OrbitCamera {
            yaw_rad: PI - 0.1,
            ..OrbitCamera::default()
        };
        let to = OrbitCamera {
            yaw_rad: -PI + 0.1,
            ..from
        };
        let halfway = from.toward(&to, 0.5);
        assert!(
            (halfway.yaw_rad - PI).abs() < 1e-5,
            "the eye passes behind, not round the front, got {}",
            halfway.yaw_rad
        );
    }

    #[test]
    fn without_perspective_a_framed_box_still_fits_the_view() {
        let mut camera = OrbitCamera {
            orthographic: true,
            ..OrbitCamera::default()
        };
        let bounds = Aabb::new(Vec3::new(-5.0, -5.0, 0.0), Vec3::new(5.0, 5.0, 20.0));
        camera.frame(&bounds);
        let view_projection = camera.view_projection(1.0);
        for corner in bounding_corners(&bounds) {
            let clip = view_projection * corner.extend(1.0);
            let ndc = clip.truncate() / clip.w;
            assert!(
                ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z),
                "corner {corner} falls outside the view at {ndc}"
            );
        }
    }

    /// Without perspective the view is as tall as the perspective one is at the target,
    /// so a point standing there lands in the same place either way.
    #[test]
    fn switching_perspective_keeps_the_target_plane_the_same_size() {
        let perspective = OrbitCamera::default();
        let flat = OrbitCamera {
            orthographic: true,
            ..perspective
        };
        let up = perspective.view().inverse().transform_vector3(Vec3::Y);
        let point = perspective.target + up * 20.0;
        let seen = |camera: &OrbitCamera| {
            let clip = camera.view_projection(1.5) * point.extend(1.0);
            clip.truncate() / clip.w
        };
        assert!(
            (seen(&perspective).y - seen(&flat).y).abs() < 1e-4,
            "{} against {}",
            seen(&perspective),
            seen(&flat)
        );
    }

    #[test]
    fn without_perspective_every_line_of_sight_runs_along_the_view_axis() {
        let camera = OrbitCamera {
            orthographic: true,
            ..OrbitCamera::default()
        };
        let point = camera.target + Vec3::new(30.0, 10.0, 5.0);
        let sight = (point - camera.sight_to(point)).normalize();
        let axis = (camera.target - camera.eye()).normalize();
        assert!(sight.abs_diff_eq(axis, 1e-5), "got {sight}");
        assert_eq!(
            OrbitCamera::default().sight_to(point),
            OrbitCamera::default().eye()
        );
    }
}
