use core_geometry::{Mat4, Quat, Transform, Vec3};
use transform_gizmo_egui::math::{DMat4, DQuat, DVec3, Transform as GizmoTransform};
use transform_gizmo_egui::prelude::mint;
use transform_gizmo_egui::{EnumSet, Gizmo, GizmoConfig, GizmoExt as _, GizmoMode, GizmoVisuals};

use crate::camera::OrbitCamera;
use crate::ui::theme;

/// Length of the arrows and radius of the rings, points. The rings sit well inside the
/// arrows so each handle reads on its own; the crate draws both kinds at one size.
const ARROW_SIZE: f32 = 100.0;
const RING_SIZE: f32 = 52.0;

fn arrows() -> EnumSet<GizmoMode> {
    GizmoMode::TranslateX | GizmoMode::TranslateY | GizmoMode::TranslateZ | GizmoMode::TranslateView
}

fn rings() -> EnumSet<GizmoMode> {
    GizmoMode::RotateX | GizmoMode::RotateY | GizmoMode::RotateZ | GizmoMode::RotateView
}

fn visuals(size: f32) -> GizmoVisuals {
    let [x_color, y_color, z_color] = theme::scene().gizmo;
    GizmoVisuals {
        x_color,
        y_color,
        z_color,
        s_color: theme::colors().text_high,
        inactive_alpha: 1.0,
        highlight_color: Some(theme::scene().gizmo_hot),
        stroke_width: 3.5,
        gizmo_size: size,
        ..GizmoVisuals::default()
    }
}

/// The move and rotate handles drawn over the selected objects: arrows and a smaller set
/// of rings on the same pivot, as two gizmos of the crate's.
///
/// Drawn by `transform-gizmo-egui` as ordinary egui shapes on top of the viewport, not by
/// our wgpu pipelines; see `docs/decisions/0017-transform-gizmo-crate.md`.
#[derive(Default)]
pub struct TransformGizmo {
    arrows: Gizmo,
    rings: Gizmo,
}

impl TransformGizmo {
    /// A handle is hovered or being dragged. Carried over from the last frame, because
    /// the gizmo is drawn after the input that has to yield to it is read.
    pub fn is_focused(&self) -> bool {
        self.arrows.is_focused() || self.rings.is_focused()
    }

    /// Draws the handles over `placements` and returns where the user dragged them to,
    /// on the frames where they dragged. Several placements move as one body:
    /// the crate puts the handles on their shared pivot.
    pub fn show(
        &mut self,
        ui: &egui::Ui,
        viewport: egui::Rect,
        camera: &OrbitCamera,
        placements: &[Transform],
    ) -> Option<Vec<Transform>> {
        if viewport.width() <= 0.0 || viewport.height() <= 0.0 {
            return None;
        }

        let aspect = viewport.width() / viewport.height();
        let config = |modes, size| GizmoConfig {
            view_matrix: row_matrix(camera.view()),
            projection_matrix: row_matrix(camera.projection(aspect)),
            viewport,
            modes,
            visuals: visuals(size),
            ..GizmoConfig::default()
        };
        self.rings.update_config(config(rings(), RING_SIZE));
        self.arrows.update_config(config(arrows(), ARROW_SIZE));

        let targets: Vec<GizmoTransform> = placements.iter().copied().map(into_gizmo).collect();
        // Where an arrow crosses a ring both can take the press; the arrow wins.
        let turned = self.rings.interact(ui, &targets);
        let moved = self.arrows.interact(ui, &targets);
        moved
            .or(turned)
            .map(|(_, placed)| placed.into_iter().map(from_gizmo).collect())
    }
}

/// The gizmo works in f64 and takes its matrices row by row, where `glam` stores columns.
fn row_matrix(matrix: Mat4) -> mint::RowMatrix4<f64> {
    DMat4::from_cols_array(&matrix.to_cols_array().map(f64::from)).into()
}

fn into_gizmo(transform: Transform) -> GizmoTransform {
    GizmoTransform::from_scale_rotation_translation(
        into_vector(transform.scale),
        DQuat::from_xyzw(
            f64::from(transform.rotation.x),
            f64::from(transform.rotation.y),
            f64::from(transform.rotation.z),
            f64::from(transform.rotation.w),
        ),
        into_vector(transform.translation),
    )
}

fn from_gizmo(transform: GizmoTransform) -> Transform {
    Transform {
        translation: from_vector(transform.translation),
        // The gizmo accumulates rotations in f64 and hands back a quaternion that has
        // drifted off unit length by the time it reaches f32.
        rotation: Quat::from_xyzw(
            transform.rotation.v.x as f32,
            transform.rotation.v.y as f32,
            transform.rotation.v.z as f32,
            transform.rotation.s as f32,
        )
        .normalize(),
        scale: from_vector(transform.scale),
    }
}

fn into_vector(value: Vec3) -> DVec3 {
    DVec3::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
}

fn from_vector(value: mint::Vector3<f64>) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_3;

    #[test]
    fn a_transform_survives_the_trip_through_the_gizmo() {
        let original = Transform {
            translation: Vec3::new(12.5, -3.25, 40.0),
            rotation: Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).normalize(), FRAC_PI_3),
            scale: Vec3::new(1.5, 2.0, 0.75),
        };
        let returned = from_gizmo(into_gizmo(original));

        assert!(returned.translation.abs_diff_eq(original.translation, 1e-5));
        assert!(returned.scale.abs_diff_eq(original.scale, 1e-5));
        assert!(
            returned.rotation.abs_diff_eq(original.rotation, 1e-5),
            "expected {:?}, got {:?}",
            original.rotation,
            returned.rotation
        );
    }

    #[test]
    fn the_matrix_is_handed_over_row_by_row() {
        // Asymmetric on purpose: a transposed matrix would pass a symmetric one.
        let matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let rows: mint::RowMatrix4<f64> = row_matrix(matrix);
        let round_trip = DMat4::from(rows);

        for (expected, actual) in matrix
            .to_cols_array()
            .iter()
            .zip(round_trip.to_cols_array())
        {
            assert!((f64::from(*expected) - actual).abs() < 1e-12);
        }
    }
}
