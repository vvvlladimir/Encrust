use core_geometry::{Scalar, Vec3};
use printer_profiles::{AUTO_SIZES, AutoProfile};

/// A contact automatic placement wants, and what it holds there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Need {
    pub contact: Vec3,
    pub reason: Reason,
}

impl Need {
    /// Which of `auto`'s shapes a support here is built to, lightest first, with
    /// [`AUTO_SIZES`] for its steadying one.
    pub fn size_in(&self, auto: &AutoProfile) -> usize {
        match self.reason {
            Reason::Island { area_mm2 } => auto.island_size(area_mm2),
            Reason::Overhang { carried_mm3 } => auto.overhang_size(carried_mm3),
            Reason::Step { new_mm3 } => auto.step_size(new_mm3).unwrap_or(0),
            Reason::Steady => AUTO_SIZES,
        }
    }
}

/// Why a contact is wanted, measured by what it holds: what an automatic profile sizes a
/// support by. See `docs/decisions/0325`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reason {
    /// The first ground of a part nothing holds, an island of `area_mm2` on the layer it
    /// begins on; a part that begins at a point is an island of no area.
    Island { area_mm2: Scalar },
    /// An overhang, with `carried_mm3` of the model standing above the contact's layer on
    /// the part it holds.
    Overhang { carried_mm3: Scalar },
    /// A step, or a face its reach takes in above it, with `new_mm3` the resin the step put out in
    /// its one layer; only a run sizing its supports asks for steps apart (ADR 0326).
    Step { new_mm3: Scalar },
    /// A support that steadies a part: one held at too few places to stand, or, in a run
    /// sizing its supports, a slope too gentle to sag (ADR 0327).
    Steady,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn a_need_is_sized_by_what_it_holds_and_a_steadying_one_last() {
        let auto = AutoProfile::from_toml_str(
            include_str!("../../../assets/profiles/auto/default.toml"),
            Path::new("auto/default.toml"),
        )
        .expect("the shipped profile reads");
        let need = |reason| Need {
            contact: Vec3::ZERO,
            reason,
        };
        assert_eq!(
            need(Reason::Island { area_mm2: 3.2 }).size_in(&auto),
            2,
            "an island of 3.2 mm² reaches the shipped heaviest threshold"
        );
        assert_eq!(need(Reason::Steady).size_in(&auto), AUTO_SIZES);
    }
}
