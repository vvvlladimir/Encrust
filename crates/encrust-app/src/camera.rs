use std::f32::consts::FRAC_PI_2;

use core_geometry::glam::camera::rh::proj::directx::perspective;
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
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw_rad: -FRAC_PI_2,
            pitch_rad: 0.45,
            distance_mm: 300.0,
            fov_y_rad: 0.8,
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
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let near = (self.distance_mm * 0.01).max(0.1);
        let far = self.distance_mm * 20.0;
        perspective(self.fov_y_rad, aspect.max(1e-3), near, far)
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
}
