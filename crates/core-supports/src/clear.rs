use crate::placed::Placed;
use core_geometry::{Ray, Scalar, Vec3};

/// A segment shorter than this is a joint, not a body: there is nothing to sweep and
/// nothing to hit. Well under any printable feature.
pub(crate) const MIN_BODY_MM: Scalar = 1e-4;

/// How many rays the surface of a body is tested with: enough to catch a thin obstacle
/// against the cost of casting them; see `docs/decisions/0077`.
const BEAM_RAYS: u32 = 8;

/// How far apart the balls filling a beam stand, as a share of their radius: at half a
/// radius their union covers the beam to within 3% of its radius. See ADR 0341.
const BALL_STEP: Scalar = 0.5;

/// The smallest ball a beam is filled with, millimetres: nearer an end than this the
/// ring of rays answers alone.
const SMALLEST_BALL_MM: Scalar = 0.05;

/// The solid a length of support sweeps: a cone from `from` to `to` with the air it must
/// keep around itself already added to both radii.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Beam {
    pub from: Vec3,
    pub to: Vec3,
    pub from_radius_mm: Scalar,
    pub to_radius_mm: Scalar,
}

impl Beam {
    /// The same beam with `clearance_mm` of air added all round.
    pub fn with_clearance(self, clearance_mm: Scalar) -> Self {
        Self {
            from_radius_mm: self.from_radius_mm + clearance_mm,
            to_radius_mm: self.to_radius_mm + clearance_mm,
            ..self
        }
    }

    /// The same beam with `along_mm` taken off the `from` end, which is how the head a
    /// tip is planted with is kept out of a test it would always fail.
    pub fn trimmed(self, along_mm: Scalar) -> Self {
        let offset = self.to - self.from;
        let length = offset.length();
        if length < MIN_BODY_MM || along_mm <= 0.0 {
            return self;
        }
        let fraction = (along_mm / length).min(1.0);
        Self {
            from: self.from + offset * fraction,
            from_radius_mm: self.from_radius_mm
                + (self.to_radius_mm - self.from_radius_mm) * fraction,
            ..self
        }
    }
}

/// Whether `beam` reaches its far end without meeting `model`: a ring of rays down its surface
/// (ADR 0077) and a row of balls down its axis kept inside its ends (ADR 0341).
pub(crate) fn clear(placed: &Placed, beam: &Beam) -> bool {
    let sweep = || rays_clear(placed, beam) && balls_clear(placed, beam);
    match placed.clearances {
        Some(kept) => kept.answer(placed, beam, sweep),
        None => sweep(),
    }
}

/// Whether no face of the model comes inside a row of balls filling `beam`, each as wide
/// as the beam where it stands and no wider than its distance to the nearer end.
fn balls_clear(placed: &Placed, beam: &Beam) -> bool {
    let offset = beam.to - beam.from;
    let length = offset.length();
    if length < 2.0 * SMALLEST_BALL_MM {
        return true;
    }
    let direction = offset / length;
    let mut found = Vec::new();
    let mut along = SMALLEST_BALL_MM;
    while along <= length - SMALLEST_BALL_MM {
        let wide_at = |at: Scalar| {
            let share = (at / length).clamp(0.0, 1.0);
            beam.from_radius_mm + (beam.to_radius_mm - beam.from_radius_mm) * share
        };
        // Narrowest over the ball's own length, so it stays inside a tapering beam.
        let wide_mm = wide_at(along);
        let ball_mm = wide_mm
            .min(wide_at(along - wide_mm))
            .min(wide_at(along + wide_mm))
            .min(along)
            .min(length - along);
        if placed.reaches_ball(beam.from + direction * along, ball_mm, &mut found) {
            return false;
        }
        along += (ball_mm * BALL_STEP).max(SMALLEST_BALL_MM);
    }
    true
}

/// The ring of rays and the axis ray of ADR 0077.
fn rays_clear(placed: &Placed, beam: &Beam) -> bool {
    let offset = beam.to - beam.from;
    let length = offset.length();
    if length < MIN_BODY_MM {
        return true;
    }

    let direction = offset / length;
    let (across, along) = direction.any_orthonormal_pair();
    let mut ring = (0..BEAM_RAYS).map(|step| {
        let angle = std::f32::consts::TAU * step as Scalar / BEAM_RAYS as Scalar;
        let out = across * angle.cos() + along * angle.sin();
        (
            beam.from + out * beam.from_radius_mm,
            beam.to + out * beam.to_radius_mm,
        )
    });

    // Only the axis asks past its own end: whether the beam set off inside the solid.
    unobstructed(placed, beam.from, beam.to, Scalar::INFINITY)
        && ring.all(|(from, to)| unobstructed(placed, from, to, 0.0))
}

/// Whether the segment `from`..`to` crosses the model or, looking `past_mm` beyond its end,
/// starts inside it: a first hit on a back face means it set off in the solid.
fn unobstructed(placed: &Placed, from: Vec3, to: Vec3, past_mm: Scalar) -> bool {
    let offset = to - from;
    let length = offset.length();
    if length < MIN_BODY_MM {
        return true;
    }

    let direction = offset / length;
    let ray = Ray::new(from, direction);
    // The tolerance is what keeps a beam that ends exactly on a surface — which is how
    // one lands on the model — from reporting its own landing as an obstacle.
    placed
        .raycast_within(&ray, length + MIN_BODY_MM + past_mm)
        .is_none_or(|hit| hit.distance + MIN_BODY_MM >= length && hit.normal.dot(direction) <= 0.0)
}

#[cfg(test)]
mod tests;
