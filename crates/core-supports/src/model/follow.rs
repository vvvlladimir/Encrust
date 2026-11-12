use super::ModelSupports;
use crate::column::MIN_PILLAR_HEIGHT_MM;
use crate::space::Space;
use crate::{Landing, Profiles};
use core_geometry::{Mat4, Scalar, Transform, Vec3};

/// How far off the plate a foot may stand and still stand on it, mm: float noise.
const PLANTED_MM: Scalar = 1e-4;

/// How far off square a turn may be and still keep the plate's up as up, as a cosine's
/// shortfall: float noise from composing transforms.
const UPRIGHT: Scalar = 1e-5;

impl ModelSupports {
    /// Stands the frozen trees again as `transform` now places the model: moved or turned about
    /// up, feet back on the plate; tilted or resized, back to growth (ADR 0439).
    pub(super) fn follow(&mut self, transform: Transform, profiles: Profiles) {
        let Some(was) = self.frozen_at.replace(transform) else {
            return;
        };
        if was == transform || self.frozen.is_empty() {
            return;
        }
        let (Some(from), Some(space)) = (was.inverse_matrix(), Space::new(transform)) else {
            return;
        };
        let lost: Vec<usize> = if keeps_upright(transform.to_matrix() * from) {
            (0..self.frozen.len())
                .filter(|frozen| !self.replant(*frozen, space, profiles))
                .collect()
        } else {
            (0..self.frozen.len()).collect()
        };
        self.built = None;
        for frozen in lost.into_iter().rev() {
            self.thaw(frozen, transform);
        }
        self.thaw_unseated(transform, profiles);
    }

    /// Stands frozen tree `frozen`'s foot back on the plate under it as `space` places the model;
    /// answers whether it still stands, a pillar's height at least over its pad.
    fn replant(&mut self, frozen: usize, space: Space, profiles: Profiles) -> bool {
        let tree = &mut self.frozen[frozen];
        let landing = tree.landing();
        if landing.on_model {
            return true;
        }
        let base = space.plate(landing.base);
        if base.z.abs() <= PLANTED_MM {
            return true;
        }
        if landing.in_plate {
            return false;
        }
        let planted = Landing {
            base: base.with_z(0.0),
            ..landing
        };
        let lowest = planted.pillar_bottom_z(profiles.of(tree.group())) + MIN_PILLAR_HEIGHT_MM;
        if space.plate(tree.root().position).z < lowest {
            return false;
        }
        tree.stand_on(Landing {
            base: space.model(planted.base),
            ..landing
        });
        true
    }
}

/// Whether `turn`, from one placement of a model to another on the plate, keeps its up as up
/// and its sizes: a move, and a turn about the plate's up.
fn keeps_upright(turn: Mat4) -> bool {
    let (x, y, z) = (
        turn.x_axis.truncate(),
        turn.y_axis.truncate(),
        turn.z_axis.truncate(),
    );
    z.dot(Vec3::Z) >= 1.0 - UPRIGHT
        && x.z.abs() <= UPRIGHT
        && y.z.abs() <= UPRIGHT
        && (x.length() - 1.0).abs() <= UPRIGHT
        && (y.length() - 1.0).abs() <= UPRIGHT
        && x.dot(y).abs() <= UPRIGHT
}
