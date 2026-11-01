use core_geometry::{Mat4, Quat, Transform, Vec3};
use transform_gizmo_egui::math::{DMat4, DQuat, DVec3, Transform as GizmoTransform};
use transform_gizmo_egui::prelude::mint;
use transform_gizmo_egui::{
    EnumSet, Gizmo, GizmoConfig, GizmoExt as _, GizmoMode, GizmoOrientation, GizmoResult,
    GizmoVisuals,
};

use crate::camera::OrbitCamera;
use crate::ui::theme;

/// Length of the arrows and radius of the rings, points. The rings sit well inside the
/// arrows so each handle reads on its own; the crate draws both kinds at one size.
const ARROW_SIZE: f32 = 100.0;
const RING_SIZE: f32 = 52.0;

/// Steps a drag keeps to while Shift is held: whole millimetres and 15° turns.
const SNAP_MM: f32 = 1.0;
const SNAP_ANGLE: f32 = std::f32::consts::PI / 12.0;

/// How far the rings reach from the pivot on screen, points; what is written there is moved
/// out of their way.
pub const HANDLES_REACH: f32 = RING_SIZE + 8.0;

/// A drag is read as along an axis when its direction is within about 2.5° of it.
const ALONG_AXIS_DOT: f32 = 0.999;

fn arrows() -> EnumSet<GizmoMode> {
    GizmoMode::TranslateX | GizmoMode::TranslateY | GizmoMode::TranslateZ | GizmoMode::TranslateView
}

fn rings() -> EnumSet<GizmoMode> {
    GizmoMode::RotateX | GizmoMode::RotateY | GizmoMode::RotateZ
}

fn visuals(size: f32) -> GizmoVisuals {
    let [x_color, y_color, z_color] = theme::scene().gizmo;
    GizmoVisuals {
        x_color,
        y_color,
        z_color,
        s_color: theme::colors().knob,
        inactive_alpha: 0.65,
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
    drag: Option<GizmoDrag>,
    pub axes: Axes,
}

/// What a drag on the handles has done since it began, for the viewport to show beside it.
/// An axis is 0 for X, 1 for Y, 2 for Z, in the frame the handles follow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GizmoDrag {
    /// The pivot left `from` and has moved `by` on the plate, mm; no axis for the circle.
    /// `reading_mm` is how far along the arrow, signed, or how far at all for the circle.
    Move {
        from: Vec3,
        by: Vec3,
        axis: Option<usize>,
        reading_mm: f32,
    },
    /// Turned on the ring of `axis`, counter-clockwise looking down it.
    Turn { axis: Option<usize>, by_deg: f32 },
}

/// Which axes the handles, the turn buttons and the size fields follow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Axes {
    /// The plate's.
    #[default]
    Global,
    /// The model's own, turned with it.
    Local,
}

impl TransformGizmo {
    /// A handle is hovered or being dragged. Carried over from the last frame, because
    /// the gizmo is drawn after the input that has to yield to it is read.
    pub fn is_focused(&self) -> bool {
        self.arrows.is_focused() || self.rings.is_focused()
    }

    /// The drag under way, if any; the gizmo not being dragged is hidden until it ends.
    pub fn drag(&self) -> Option<GizmoDrag> {
        self.drag
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
        // A drag may end on a frame the handles are not shown; a fresh press starts anew.
        if ui.input(|input| input.pointer.primary_pressed() || !input.pointer.primary_down()) {
            self.drag = None;
        }
        if viewport.width() <= 0.0 || viewport.height() <= 0.0 {
            return None;
        }

        let snapping = ui.input(|input| input.modifiers.shift);
        self.configure(viewport, camera, snapping);

        let targets: Vec<GizmoTransform> = placements.iter().copied().map(into_gizmo).collect();
        // Where an arrow crosses a ring both can take the press; the arrow wins.
        let turned = (!matches!(self.drag, Some(GizmoDrag::Move { .. })))
            .then(|| self.rings.interact(ui, &targets))
            .flatten();
        let moved = (!matches!(self.drag, Some(GizmoDrag::Turn { .. })))
            .then(|| self.arrows.interact(ui, &targets))
            .flatten();
        let Some((result, placed)) = moved.or(turned) else {
            // The crate skips a frame's result when the cursor leaves its plane, mid-drag;
            // the held button, not the result, says when the drag ends.
            self.drag = self
                .drag
                .filter(|_| ui.input(|input| input.pointer.primary_down()));
            return None;
        };
        let placed: Vec<Transform> = placements
            .iter()
            .zip(placed.into_iter().map(from_gizmo))
            .map(|(was, now)| carried(*was, now))
            .collect();
        let from = match self.drag {
            Some(GizmoDrag::Move { from, .. }) => from,
            _ => centre(placements),
        };
        self.drag = read(result, from, centre(&placed));
        Some(placed)
    }

    /// Points both gizmos at the camera's view of `viewport`; `snapping` holds the steps.
    fn configure(&mut self, viewport: egui::Rect, camera: &OrbitCamera, snapping: bool) {
        let aspect = viewport.width() / viewport.height();
        let config = |modes, size| GizmoConfig {
            view_matrix: row_matrix(camera.view()),
            projection_matrix: row_matrix(camera.projection(aspect)),
            viewport,
            modes,
            orientation: match self.axes {
                Axes::Global => GizmoOrientation::Global,
                Axes::Local => GizmoOrientation::Local,
            },
            snapping,
            snap_angle: SNAP_ANGLE,
            snap_distance: SNAP_MM,
            visuals: visuals(size),
            ..GizmoConfig::default()
        };
        self.rings.update_config(config(rings(), RING_SIZE));
        self.arrows.update_config(config(arrows(), ARROW_SIZE));
    }
}

/// Where the crate stands the handles over `placements`: the mean of their positions.
pub fn centre(placements: &[Transform]) -> Vec3 {
    let sum: Vec3 = placements
        .iter()
        .map(|placement| placement.translation)
        .sum();
    sum / placements.len().max(1) as f32
}

/// The drag the crate reports, for a pivot that left `from` and stands at `now`. The crate
/// gives a move and a ring's axis in the frame the handles follow, and turns by minus `total`.
fn read(result: GizmoResult, from: Vec3, now: Vec3) -> Option<GizmoDrag> {
    match result {
        GizmoResult::Translation { total, .. } => {
            let total = from_vector(total);
            let axis = along_axis(total);
            Some(GizmoDrag::Move {
                from,
                by: now - from,
                axis,
                reading_mm: axis.map_or(total.length(), |index| total[index]),
            })
        }
        GizmoResult::Rotation { axis, total, .. } => {
            let normal = from_vector(axis);
            let ring = along_axis(normal);
            let sign = ring.map_or(1.0, |index| normal[index].signum());
            Some(GizmoDrag::Turn {
                axis: ring,
                by_deg: -(total as f32).to_degrees() * sign,
            })
        }
        GizmoResult::Scale { .. } | GizmoResult::Arcball { .. } => None,
    }
}

/// Which of X, Y and Z `direction` lies along, either way.
fn along_axis(direction: Vec3) -> Option<usize> {
    let direction = direction.try_normalize()?;
    (0..3).find(|&index| direction[index].abs() > ALONG_AXIS_DOT)
}

/// `was` where the crate put it. The crate knows a turn and a scale but not a stretch along
/// the plate, so what it did is taken as a turn of the whole body and a move.
fn carried(was: Transform, now: Transform) -> Transform {
    Transform {
        translation: now.translation,
        ..was.turned(now.rotation * was.rotation.inverse())
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
    Transform::from_scale_rotation_translation(
        from_vector(transform.scale),
        // The gizmo accumulates rotations in f64 and hands back a quaternion that has
        // drifted off unit length by the time it reaches f32.
        Quat::from_xyzw(
            transform.rotation.v.x as f32,
            transform.rotation.v.y as f32,
            transform.rotation.v.z as f32,
            transform.rotation.s as f32,
        )
        .normalize(),
        from_vector(transform.translation),
    )
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
    use core_geometry::Mat3;
    use std::f32::consts::FRAC_PI_3;

    #[test]
    fn a_transform_survives_the_trip_through_the_gizmo() {
        let original = Transform {
            translation: Vec3::new(12.5, -3.25, 40.0),
            rotation: Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).normalize(), FRAC_PI_3),
            scale: Vec3::new(1.5, 2.0, 0.75),
            ..Transform::default()
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
    fn a_stretch_the_gizmo_cannot_see_turns_and_moves_with_the_model() {
        let was = Transform::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::from_rotation_x(0.6),
            Vec3::ONE,
        )
        .stretched(Vec3::new(1.0, 1.0, 1.5));
        let turn = Quat::from_rotation_z(FRAC_PI_3);
        let now = Transform::from_scale_rotation_translation(
            was.scale,
            turn * was.rotation,
            Vec3::new(4.0, 5.0, 6.0),
        );
        let placed = carried(was, from_gizmo(into_gizmo(now)));

        assert!(placed.translation.abs_diff_eq(now.translation, 1e-5));
        assert!(
            placed
                .linear()
                .abs_diff_eq(Mat3::from_quat(turn) * was.linear(), 1e-4),
            "the stretched body turned as it stood"
        );
    }

    #[test]
    fn a_move_along_an_arrow_names_its_axis_and_signs_the_reading() {
        let result = GizmoResult::Translation {
            delta: DVec3::ZERO.into(),
            total: DVec3::new(0.0, -4.0, 0.0).into(),
        };
        let Some(GizmoDrag::Move {
            axis, reading_mm, ..
        }) = read(result, Vec3::ZERO, Vec3::new(0.0, -4.0, 0.0))
        else {
            panic!("a translation reads as a move");
        };
        assert_eq!(axis, Some(1));
        assert!(
            (reading_mm + 4.0).abs() < 1e-6,
            "expected -4 mm, got {reading_mm}"
        );
    }

    #[test]
    fn a_move_off_every_axis_reads_its_length() {
        let result = GizmoResult::Translation {
            delta: DVec3::ZERO.into(),
            total: DVec3::new(3.0, 4.0, 0.0).into(),
        };
        let Some(GizmoDrag::Move {
            axis, reading_mm, ..
        }) = read(result, Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0))
        else {
            panic!("a translation reads as a move");
        };
        assert_eq!(axis, None);
        assert!(
            (reading_mm - 5.0).abs() < 1e-6,
            "expected 5 mm, got {reading_mm}"
        );
    }

    #[test]
    fn a_turn_reads_the_same_whichever_way_the_ring_faces() {
        let turn = |z: f64, total: f64| {
            let result = GizmoResult::Rotation {
                axis: DVec3::new(0.0, 0.0, z).into(),
                delta: 0.0,
                total,
                is_view_axis: false,
            };
            read(result, Vec3::ZERO, Vec3::ZERO)
        };
        assert_eq!(turn(1.0, -0.5), turn(-1.0, 0.5));
        assert_eq!(
            turn(1.0, -f64::from(FRAC_PI_3)),
            Some(GizmoDrag::Turn {
                axis: Some(2),
                by_deg: FRAC_PI_3.to_degrees(),
            }),
            "the crate turns the model by minus its total"
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

    /// BUG: a drag that ended on a frame the handles were not shown stayed in `drag` and
    /// kept the gizmo hidden.
    #[test]
    fn a_drag_left_over_from_a_frame_without_handles_is_dropped_once_the_button_is_up() {
        let mut gizmo = TransformGizmo {
            drag: Some(GizmoDrag::Turn {
                axis: Some(2),
                by_deg: 10.0,
            }),
            ..TransformGizmo::default()
        };
        let camera = OrbitCamera::default();
        let mut output = egui::Context::default().run_ui(egui::RawInput::default(), |ui| {
            gizmo.show(ui, egui::Rect::NOTHING, &camera, &[]);
        });
        output.textures_delta.clear();
        assert_eq!(gizmo.drag(), None, "no button is down, so no drag stands");
    }
}
