use crate::placed::Placed;
use core_geometry::{Ray, Scalar, Vec3, raycast_placed};

/// A segment shorter than this is a joint, not a body: there is nothing to sweep and
/// nothing to hit. Well under any printable feature.
pub(crate) const MIN_BODY_MM: Scalar = 1e-4;

/// How many rays the surface of a body is tested with: enough to catch a thin obstacle
/// against the cost of casting them; see `docs/decisions/0077`.
const BEAM_RAYS: u32 = 8;

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

/// Whether `beam` reaches its far end without meeting `model`.
///
/// The surface of the beam is sampled with a ring of rays rather than its axis alone: a
/// body has width, and a single axis ray misses everything the body's own radius would
/// have run into; see `docs/decisions/0077`.
pub(crate) fn clear(placed: &Placed, beam: &Beam) -> bool {
    let offset = beam.to - beam.from;
    let length = offset.length();
    if length < MIN_BODY_MM {
        return true;
    }

    let direction = offset / length;
    let (across, along) = direction.any_orthonormal_pair();
    let ring = (0..BEAM_RAYS).map(|step| {
        let angle = std::f32::consts::TAU * step as Scalar / BEAM_RAYS as Scalar;
        let out = across * angle.cos() + along * angle.sin();
        (
            beam.from + out * beam.from_radius_mm,
            beam.to + out * beam.to_radius_mm,
        )
    });

    std::iter::once((beam.from, beam.to))
        .chain(ring)
        .all(|(from, to)| unobstructed(placed, from, to))
}

/// Whether the segment `from`..`to` crosses the model, or starts inside it.
///
/// A first hit on a back face means the segment set off in the solid, which is the case
/// a forward ray otherwise reports as clear air all the way to the far wall.
fn unobstructed(placed: &Placed, from: Vec3, to: Vec3) -> bool {
    let offset = to - from;
    let length = offset.length();
    if length < MIN_BODY_MM {
        return true;
    }

    let direction = offset / length;
    let ray = Ray::new(from, direction);
    // The tolerance is what keeps a beam that ends exactly on a surface — which is how
    // one lands on the model — from reporting its own landing as an obstacle.
    raycast_placed(placed.model, placed.bvh, placed.transform, &ray)
        .is_none_or(|hit| hit.distance + MIN_BODY_MM >= length && hit.normal.dot(direction) <= 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::box_mesh;
    use core_geometry::{Bvh, Mesh, Transform};

    /// A wall filling 0..1 on x, standing beside the origin.
    fn wall() -> Mesh {
        box_mesh(Vec3::new(0.0, -10.0, 0.0), Vec3::new(1.0, 10.0, 20.0))
    }

    /// A beam of one radius, run against the wall.
    fn clears(from: Vec3, to: Vec3, radius_mm: Scalar) -> bool {
        let model = wall();
        clear(
            &Placed::new(&model, &Bvh::build(&model), Transform::default()),
            &Beam {
                from,
                to,
                from_radius_mm: radius_mm,
                to_radius_mm: radius_mm,
            },
        )
    }

    #[test]
    fn a_body_well_clear_of_the_model_passes() {
        assert!(clears(
            Vec3::new(5.0, 0.0, 15.0),
            Vec3::new(5.0, 0.0, 1.0),
            0.4
        ));
    }

    #[test]
    fn a_body_running_alongside_a_wall_within_its_own_radius_is_blocked() {
        // Axis 0.3 mm clear of the wall's face at x = 1, body 0.4 mm across: the axis
        // ray never meets the wall and the body is inside it.
        assert!(!clears(
            Vec3::new(1.3, 0.0, 15.0),
            Vec3::new(1.3, 0.0, 1.0),
            0.4
        ));
    }

    #[test]
    fn a_body_crossing_the_model_is_blocked() {
        assert!(!clears(
            Vec3::new(-2.0, 0.0, 10.0),
            Vec3::new(3.0, 0.0, 10.0),
            0.1
        ));
    }

    #[test]
    fn a_body_starting_inside_the_model_is_blocked() {
        assert!(!clears(
            Vec3::new(0.5, 0.0, 10.0),
            Vec3::new(0.5, 0.0, 1.0),
            0.1
        ));
    }

    #[test]
    fn a_body_of_no_length_meets_nothing() {
        let at = Vec3::new(0.5, 0.0, 10.0);
        assert!(clears(at, at, 0.4), "there is no body to sweep");
    }

    #[test]
    fn clearance_pushes_a_body_away_from_a_wall_it_was_passing() {
        let model = wall();
        let bvh = Bvh::build(&model);
        let beam = Beam {
            from: Vec3::new(1.5, 0.0, 15.0),
            to: Vec3::new(1.5, 0.0, 1.0),
            from_radius_mm: 0.2,
            to_radius_mm: 0.2,
        };

        assert!(clear(
            &Placed::new(&model, &bvh, Transform::default()),
            &beam
        ));
        assert!(
            !clear(
                &Placed::new(&model, &bvh, Transform::default()),
                &beam.with_clearance(0.5)
            ),
            "half a millimetre of air does not fit in the 0.3 mm left beside the wall"
        );
    }

    #[test]
    fn a_trimmed_beam_starts_past_what_was_taken_off_it() {
        let beam = Beam {
            from: Vec3::new(0.0, 0.0, 10.0),
            to: Vec3::new(0.0, 0.0, 0.0),
            from_radius_mm: 0.2,
            to_radius_mm: 0.4,
        };
        let trimmed = beam.trimmed(5.0);

        assert!((trimmed.from.z - 5.0).abs() < 1e-5);
        assert!(
            (trimmed.from_radius_mm - 0.3).abs() < 1e-5,
            "the radius is read off the taper where the trim ends"
        );
        assert!((trimmed.to_radius_mm - 0.4).abs() < 1e-5);
    }

    #[test]
    fn trimming_past_the_far_end_leaves_no_body() {
        let beam = Beam {
            from: Vec3::new(1.3, 0.0, 15.0),
            to: Vec3::new(1.3, 0.0, 1.0),
            from_radius_mm: 0.4,
            to_radius_mm: 0.4,
        };
        let model = wall();
        assert!(clear(
            &Placed::new(&model, &Bvh::build(&model), Transform::default()),
            &beam.trimmed(100.0)
        ));
    }
}
