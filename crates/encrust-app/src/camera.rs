use std::f32::consts::{FRAC_PI_2, PI, TAU};

use core_geometry::glam::camera::rh::proj::directx::{orthographic, perspective};
use core_geometry::glam::camera::rh::view::look_at_mat4;
use core_geometry::{Aabb, Mat4, Vec2, Vec3};

use crate::plate::BuildPlate;

mod flight;
mod focus;

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
    /// A flight from `from` to `to` starting at `started_s`, as long as its path asks.
    pub fn flight(from: OrbitCamera, to: OrbitCamera, started_s: f64) -> Self {
        Self {
            from,
            to,
            started_s,
            length_s: flight::seconds(&from, &to),
        }
    }

    /// Where the camera stands at `now_s`, and whether the turn is over. The target and
    /// distance take the path that zooms out over a long way; see `flight`.
    pub fn at(&self, now_s: f64) -> (OrbitCamera, bool) {
        let share = ((now_s - self.started_s) / self.length_s.max(1e-6)).clamp(0.0, 1.0) as f32;
        // The ends are the two views exactly, not as the path's arithmetic rounds them.
        match share {
            0.0 => return (self.from, false),
            1.0 => return (self.to, true),
            _ => {}
        }
        let eased = share * share * (3.0 - 2.0 * share);
        let (target, distance_mm) = flight::along(&self.from, &self.to, eased);
        let camera = OrbitCamera {
            target,
            distance_mm,
            ..self.from.toward(&self.to, eased)
        };
        (camera, share >= 1.0)
    }
}

#[cfg(test)]
mod tests;
