use serde::{Deserialize, Serialize};

use crate::{Aabb, Mat3, Mat4, Mesh, Quat, Scalar, Vec3};

/// Placement of a model on the build plate: non-uniform scale, then rotation, then a stretch
/// along the plate's axes, then translation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    /// What sizing a turned model along the plate's axes leaves, which no scale along the
    /// model's own can hold. The identity for a model only scaled and turned.
    #[serde(default, skip_serializing_if = "is_identity")]
    pub stretch: Mat3,
}

/// How far off 1 the cosine between a plate axis and a model axis may be for the two to be
/// one: a turn left a hair off a right angle by rounding still counts as square.
const ALIGNED: Scalar = 1e-6;

fn is_identity(matrix: &Mat3) -> bool {
    *matrix == Mat3::IDENTITY
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            stretch: Mat3::IDENTITY,
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

    /// Scale, then rotation, then translation, with nothing stretched along the plate.
    pub fn from_scale_rotation_translation(scale: Vec3, rotation: Quat, translation: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
            stretch: Mat3::IDENTITY,
        }
    }

    pub fn to_matrix(self) -> Mat4 {
        let placed =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        if is_identity(&self.stretch) {
            return placed;
        }
        Mat4::from_translation(self.translation)
            * Mat4::from_mat3(self.stretch * self.linear_unstretched())
    }

    /// Everything but the translation.
    pub fn linear(self) -> Mat3 {
        self.stretch * self.linear_unstretched()
    }

    fn linear_unstretched(self) -> Mat3 {
        Mat3::from_quat(self.rotation) * Mat3::from_diagonal(self.scale)
    }

    /// Whether the placement turns the model inside out, which reverses its winding.
    pub fn is_mirrored(self) -> bool {
        self.linear().determinant() < 0.0
    }

    /// The same placement turned by `turn` about the plate's axes, as a rigid body: a
    /// stretch it carries turns with it. The translation is the caller's to settle.
    #[must_use]
    pub fn turned(self, turn: Quat) -> Self {
        let stretch = if is_identity(&self.stretch) {
            self.stretch
        } else {
            let turn = Mat3::from_quat(turn);
            turn * self.stretch * turn.transpose()
        };
        Self {
            rotation: (turn * self.rotation).normalize(),
            stretch,
            ..self
        }
    }

    /// The same placement stretched by `factors` along the plate's X, Y and Z. Kept as a
    /// scale while each stretched axis of the plate is one of the model's own.
    #[must_use]
    pub fn stretched(self, factors: Vec3) -> Self {
        if is_identity(&self.stretch)
            && let Some(scale) = self.scale_along_plate(factors)
        {
            return Self { scale, ..self };
        }
        Self {
            stretch: Mat3::from_diagonal(factors) * self.stretch,
            ..self
        }
    }

    /// `factors` as a scale on the model's own axes, where every plate axis they stretch
    /// lies along one of those.
    fn scale_along_plate(self, factors: Vec3) -> Option<Vec3> {
        let turn = Mat3::from_quat(self.rotation);
        let mut scale = self.scale;
        for plate_axis in (0..3).filter(|axis| (factors[*axis] - 1.0).abs() > Scalar::EPSILON) {
            let own = (0..3).find(|own| turn.col(*own)[plate_axis].abs() > 1.0 - ALIGNED)?;
            scale[own] *= factors[plate_axis];
        }
        Some(scale)
    }

    /// How long each of the plate's axes reads through the placement: the length of each row
    /// of [`Self::linear`], which a stretch along that axis multiplies.
    pub fn plate_factors(self) -> Vec3 {
        let rows = self.linear().transpose();
        Vec3::new(
            rows.x_axis.length(),
            rows.y_axis.length(),
            rows.z_axis.length(),
        )
    }

    /// The placement with its turn and translation taken out, the stretch carried into the
    /// model's own axes: a mesh baked through it needs only the turn put back on, and
    /// measures as it does on the plate.
    #[must_use]
    pub fn sizing(self) -> Self {
        let stretch = if is_identity(&self.stretch) {
            self.stretch
        } else {
            let turn = Mat3::from_quat(self.rotation);
            turn.transpose() * self.stretch * turn
        };
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: self.scale,
            stretch,
        }
    }

    /// The placement with its turn taken out: an upper triangular map that measures every
    /// length as the plate does, and that no turn of the model changes. `diag(|scale|)` for a
    /// model with nothing stretched. Wall thicknesses are worked out in this space.
    pub fn unturned(self) -> Mat3 {
        let linear = self.linear();
        cholesky(linear.transpose() * linear)
    }

    /// The least any length is scaled by, in whatever direction: what a size asked for on
    /// the plate is divided by to be kept in the model's own space.
    pub fn least_scale(self) -> Scalar {
        smallest_scale(self.linear())
    }
}

/// The least `linear` scales any length by, in whatever direction: its smallest singular
/// value, kept off zero.
pub fn smallest_scale(linear: Mat3) -> Scalar {
    smallest_eigenvalue(linear.transpose() * linear)
        .max(0.0)
        .sqrt()
        .max(Scalar::EPSILON)
}

/// The upper triangular `m` with `mᵀ·m = metric`, for a symmetric positive definite
/// `metric`. A degenerate one leaves zeros where its square roots would be.
fn cholesky(metric: Mat3) -> Mat3 {
    let g = |row: usize, col: usize| metric.col(col)[row];
    let root = |value: Scalar| value.max(0.0).sqrt();
    let over = |value: Scalar, by: Scalar| if by > 0.0 { value / by } else { 0.0 };
    let m00 = root(g(0, 0));
    let m01 = over(g(0, 1), m00);
    let m02 = over(g(0, 2), m00);
    let m11 = root(g(1, 1) - m01 * m01);
    let m12 = over(g(1, 2) - m01 * m02, m11);
    let m22 = root(g(2, 2) - m02 * m02 - m12 * m12);
    Mat3::from_cols(
        Vec3::new(m00, 0.0, 0.0),
        Vec3::new(m01, m11, 0.0),
        Vec3::new(m02, m12, m22),
    )
}

/// The smallest eigenvalue of a symmetric matrix, in closed form: Smith, "Eigenvalues of a
/// symmetric 3 × 3 matrix", Communications of the ACM 4 (1961).
fn smallest_eigenvalue(matrix: Mat3) -> Scalar {
    let a = |row: usize, col: usize| matrix.col(col)[row];
    let off = a(0, 1) * a(0, 1) + a(0, 2) * a(0, 2) + a(1, 2) * a(1, 2);
    if off == 0.0 {
        return a(0, 0).min(a(1, 1)).min(a(2, 2));
    }
    let mean = (a(0, 0) + a(1, 1) + a(2, 2)) / 3.0;
    let spread = ((a(0, 0) - mean).powi(2)
        + (a(1, 1) - mean).powi(2)
        + (a(2, 2) - mean).powi(2)
        + 2.0 * off)
        / 6.0;
    let spread = spread.sqrt();
    let shifted = (matrix - Mat3::from_diagonal(Vec3::splat(mean))) * (1.0 / spread);
    let angle = (shifted.determinant() / 2.0).clamp(-1.0, 1.0).acos() / 3.0;
    mean + 2.0 * spread * (angle + 2.0 * std::f32::consts::FRAC_PI_3).cos()
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

    let mirrored = transform.is_mirrored();
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
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

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
        let t = Transform::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::from_rotation_z(FRAC_PI_2),
            Vec3::new(0.0, 0.0, 5.0),
        );
        let moved = t.to_matrix().transform_point3(Vec3::X);
        assert!(moved.abs_diff_eq(Vec3::new(0.0, 2.0, 5.0), 1e-5));
    }

    fn turned_on_its_edge() -> Transform {
        Transform::from_scale_rotation_translation(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::from_rotation_x(FRAC_PI_4),
            Vec3::new(5.0, 6.0, 7.0),
        )
    }

    #[test]
    fn a_stretch_along_the_plate_multiplies_that_axis_alone() {
        let placed = turned_on_its_edge();
        let stretched = placed.stretched(Vec3::new(1.0, 1.0, 2.0));
        let (before, after) = (placed.linear(), stretched.linear());
        for column in 0..3 {
            let (from, to) = (before.col(column), after.col(column));
            assert!(
                to.abs_diff_eq(from * Vec3::new(1.0, 1.0, 2.0), 1e-5),
                "every model axis grows only in Z, column {column}: {from:?} to {to:?}"
            );
        }
        assert!(
            stretched
                .to_matrix()
                .transform_point3(Vec3::ZERO)
                .abs_diff_eq(placed.translation, 1e-6),
            "the stretch is about the model's origin, not the plate's"
        );
    }

    #[test]
    fn a_turn_carries_a_stretch_with_it() {
        let stretched = turned_on_its_edge().stretched(Vec3::new(1.0, 1.0, 2.0));
        let turn = Quat::from_rotation_y(0.7);
        let expected = Mat3::from_quat(turn) * stretched.linear();
        assert!(
            stretched.turned(turn).linear().abs_diff_eq(expected, 1e-5),
            "turning a stretched model turns the stretched shape as it stands"
        );
    }

    #[test]
    fn the_plate_factors_read_back_a_stretch() {
        let placed = Transform::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::from_rotation_z(0.4),
            Vec3::ZERO,
        );
        assert!(placed.plate_factors().abs_diff_eq(Vec3::splat(2.0), 1e-5));
        let stretched = placed.stretched(Vec3::new(1.5, 1.0, 1.0));
        assert!(
            stretched
                .plate_factors()
                .abs_diff_eq(Vec3::new(3.0, 2.0, 2.0), 1e-5),
            "X read 2 and was stretched by 1.5, got {:?}",
            stretched.plate_factors()
        );
    }

    #[test]
    fn unturned_is_the_scale_for_a_model_only_scaled() {
        let placed = Transform::from_scale_rotation_translation(
            Vec3::new(-1.5, 2.0, 0.5),
            Quat::from_rotation_y(1.1),
            Vec3::ONE,
        );
        assert!(
            placed
                .unturned()
                .abs_diff_eq(Mat3::from_diagonal(Vec3::new(1.5, 2.0, 0.5)), 1e-5),
            "a turn and a mirror measure nothing, got {:?}",
            placed.unturned()
        );
    }

    #[test]
    fn unturned_measures_as_the_plate_does_and_ignores_a_turn() {
        let stretched = turned_on_its_edge().stretched(Vec3::new(2.0, 0.5, 1.5));
        let unturned = stretched.unturned();
        let metric = stretched.linear().transpose() * stretched.linear();
        assert!((unturned.transpose() * unturned).abs_diff_eq(metric, 1e-4));
        let turned = stretched.turned(Quat::from_rotation_z(2.0)).unturned();
        assert!(
            turned.abs_diff_eq(unturned, 1e-4),
            "{turned:?} against {unturned:?}"
        );
    }

    #[test]
    fn the_least_scale_is_the_shortest_axis_of_the_stretched_body() {
        let placed = Transform::from_scale_rotation_translation(
            Vec3::new(2.0, 3.0, 0.5),
            Quat::from_rotation_z(0.3),
            Vec3::ZERO,
        );
        assert!((placed.least_scale() - 0.5).abs() < 1e-5);
        // An unscaled model turned anyhow and squashed to a quarter along the plate's Z.
        let squashed = Transform::from_scale_rotation_translation(
            Vec3::ONE,
            Quat::from_rotation_x(0.9),
            Vec3::ZERO,
        )
        .stretched(Vec3::new(1.0, 1.0, 0.25));
        assert!(
            (squashed.least_scale() - 0.25).abs() < 1e-4,
            "the plate's Z is squashed to a quarter, got {}",
            squashed.least_scale()
        );
    }

    #[test]
    fn a_stretch_does_not_mirror_and_a_negative_scale_does() {
        let stretched = turned_on_its_edge().stretched(Vec3::new(3.0, 1.0, 1.0));
        assert!(!stretched.is_mirrored());
        let mirrored = Transform {
            scale: Vec3::new(-1.0, 1.0, 1.0),
            ..stretched
        };
        assert!(mirrored.is_mirrored());
    }

    #[test]
    fn the_turn_put_back_on_the_sizing_is_the_placement() {
        let placed = turned_on_its_edge().stretched(Vec3::new(0.5, 2.0, 1.5));
        let sizing = placed.sizing();
        let rebuilt = Mat3::from_quat(placed.rotation) * sizing.linear();
        assert!(rebuilt.abs_diff_eq(placed.linear(), 1e-5));
        assert_eq!(
            Transform::from_scale_rotation_translation(
                Vec3::splat(2.0),
                Quat::from_rotation_z(1.0),
                Vec3::ONE
            )
            .sizing(),
            Transform {
                scale: Vec3::splat(2.0),
                ..Transform::default()
            },
            "a model only scaled bakes its scale alone"
        );
    }

    #[test]
    fn a_stretch_along_one_of_the_models_own_axes_stays_a_scale() {
        let lying = Transform::from_scale_rotation_translation(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::from_rotation_x(FRAC_PI_2),
            Vec3::ZERO,
        );
        let stretched = lying.stretched(Vec3::new(1.0, 1.0, 2.0));
        assert_eq!(stretched.stretch, Mat3::IDENTITY);
        assert!(
            stretched.scale.abs_diff_eq(Vec3::new(1.0, 4.0, 3.0), 1e-6),
            "laid on its back, the model's Y stands along the plate's Z, got {:?}",
            stretched.scale
        );
        let slanted = turned_on_its_edge().stretched(Vec3::new(1.0, 1.0, 2.0));
        assert_ne!(
            slanted.stretch,
            Mat3::IDENTITY,
            "at 45 degrees no axis of the model is Z"
        );
    }
}
