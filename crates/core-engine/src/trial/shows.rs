use core_geometry::{Bvh, Prominence, Quat, Vec3};
use core_plate::Printed;

/// How much a model shows where a support touches it, read once on the model as its file
/// has it and asked of every pose printed (ADR 0409, 0410).
pub(super) struct Shows {
    bvh: Bvh,
    prominence: Prominence,
}

impl Shows {
    /// What the user painted on the outer surface, where any is painted; otherwise read off
    /// the surface the supports touch, the shell's where it is hollowed.
    pub fn of(printed: &Printed) -> Self {
        let painted = printed.shown.filter(|painted| !painted.is_empty());
        let model = match painted {
            Some(_) => printed.outer,
            None => printed.shell.unwrap_or(printed.outer),
        };
        Self {
            bvh: Bvh::build(model),
            prominence: Prominence::shown(model, painted),
        }
    }

    /// The prominence under each of `contacts` summed: one on open detail counts one. The
    /// contacts are plate millimetres on the model turned by `rotation` and moved by `moved`.
    pub fn under(&self, contacts: &[Vec3], rotation: Quat, moved: Vec3) -> f64 {
        let back = rotation.inverse();
        contacts
            .iter()
            .filter_map(|contact| self.bvh.closest(back * (*contact - moved)))
            .map(|nearest| f64::from(self.prominence.face(nearest.face)))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::ball;

    use super::*;

    #[test]
    fn a_contact_on_a_turned_and_moved_ball_counts_what_the_ball_shows_there() {
        let model = ball(Vec3::ZERO, 10.0, 24, 48);
        let shows = Shows::of(&Printed::solid(&model));
        let rotation = Quat::from_rotation_x(1.0);
        let moved = Vec3::new(50.0, 40.0, 15.0);
        let contacts: Vec<Vec3> = [Vec3::NEG_Z, Vec3::X, Vec3::Y]
            .map(|way| rotation * (way * 10.0) + moved)
            .to_vec();
        let under = shows.under(&contacts, rotation, moved);
        // Every face of a ball is open and smooth: a third each, as `Prominence` reads it.
        assert!(
            (under - 1.0).abs() < 0.15,
            "three contacts on a ball count {under}, expected 1"
        );
    }

    #[test]
    fn a_contact_counts_one_on_a_painted_face_and_nothing_off_it() {
        let model = ball(Vec3::ZERO, 10.0, 24, 48);
        let painted: core_geometry::Region = (0..model.faces.len())
            .filter(|face| model.triangle(*face).is_some_and(|t| t.a.z < -9.0))
            .collect();
        let printed = Printed {
            shown: Some(&painted),
            ..Printed::solid(&model)
        };
        let shows = Shows::of(&printed);
        let contacts = [Vec3::NEG_Z * 10.0, Vec3::X * 10.0];
        let under = shows.under(&contacts, Quat::IDENTITY, Vec3::ZERO);
        assert!(
            (under - 1.0).abs() < 1e-6,
            "one contact on paint and one off count {under}, expected 1"
        );
    }
}
