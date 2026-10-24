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

/// Whether `beam` reaches its far end without meeting `model`.
///
/// A ring of rays down its surface finds a wall it crosses and a start inside the solid
/// (ADR 0077); a row of balls down its axis finds whatever pokes in between those rays,
/// each ball kept inside the beam's own ends (ADR 0341).
pub(crate) fn clear(placed: &Placed, beam: &Beam) -> bool {
    rays_clear(placed, beam) && balls_clear(placed, beam)
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
    placed
        .raycast(&ray)
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

    #[test]
    fn a_rod_standing_inside_a_body_between_its_rays_is_blocked() {
        // A rod 0.1 mm square, 0.5 mm off the axis of a body 1 mm in radius: parallel to
        // every ray, it lies between the axis and the ring and meets none of them.
        let model = box_mesh(Vec3::new(0.45, -0.05, 5.0), Vec3::new(0.55, 0.05, 10.0));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let beam = Beam {
            from: Vec3::new(0.0, 0.0, 15.0),
            to: Vec3::new(0.0, 0.0, 1.0),
            from_radius_mm: 1.0,
            to_radius_mm: 1.0,
        };
        assert!(rays_clear(&placed, &beam), "the rays alone pass it by");
        assert!(!clear(&placed, &beam), "the balls down the axis find it");
    }

    #[test]
    fn a_body_ending_on_the_face_it_lands_on_is_clear() {
        // Standing straight on the wall's top at z = 20, as a landing on the model does.
        let model = wall();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let beam = Beam {
            from: Vec3::new(0.5, 0.0, 30.0),
            to: Vec3::new(0.5, 0.0, 20.0 + 1e-3),
            from_radius_mm: 0.4,
            to_radius_mm: 0.4,
        };
        assert!(clear(&placed, &beam));
    }

    #[test]
    fn a_ball_is_measured_on_the_plate_through_a_stretched_placement() {
        // Doubled along x, the wall's face stands at x = 2 on the plate.
        let model = wall();
        let bvh = Bvh::build(&model);
        let transform = Transform {
            scale: Vec3::new(2.0, 1.0, 1.0),
            ..Transform::default()
        };
        let placed = Placed::new(&model, &bvh, transform);
        let mut found = Vec::new();
        assert!(placed.reaches_ball(Vec3::new(2.4, 0.0, 10.0), 0.5, &mut found));
        assert!(!placed.reaches_ball(Vec3::new(2.6, 0.0, 10.0), 0.5, &mut found));
    }
}
