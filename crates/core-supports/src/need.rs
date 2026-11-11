use core_geometry::{Scalar, Vec3};
use printer_profiles::AUTO_SIZES;

/// A contact automatic placement wants, and what it holds there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Need {
    pub contact: Vec3,
    pub reason: Reason,
    /// Sizes heavier than the lightest, where what it carries proved too much for that.
    pub heavier: u8,
}

impl Need {
    /// A contact at `contact` wanted for `reason`, at the lightest size.
    pub fn new(contact: Vec3, reason: Reason) -> Self {
        Self {
            contact,
            reason,
            heavier: 0,
        }
    }

    /// Which of an automatic profile's shapes a support here is built to, lightest first, with
    /// [`AUTO_SIZES`] for its steadying one: the lightest until what it carries asks more.
    pub fn size(&self) -> usize {
        if self.reason == Reason::Steady {
            return AUTO_SIZES;
        }
        usize::from(self.heavier).min(AUTO_SIZES - 1)
    }
}

/// Why a contact is wanted, measured by what it holds. See `docs/decisions/0325`.
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
    /// sizing its supports, a slope too gentle to sag (ADR 0326).
    Steady,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_need_starts_lightest_whatever_it_holds_and_a_steadying_one_last() {
        let need = |reason| Need::new(Vec3::ZERO, reason);
        assert_eq!(
            need(Reason::Island { area_mm2: 50.0 }).size(),
            0,
            "a large island is sized by what it carries, not by its area"
        );
        assert_eq!(
            need(Reason::Overhang {
                carried_mm3: 5000.0
            })
            .size(),
            0,
            "much of the model above is no reason on its own"
        );
        assert_eq!(need(Reason::Steady).size(), AUTO_SIZES);
    }

    #[test]
    fn a_need_made_heavier_takes_a_heavier_size_but_never_the_steadying_one() {
        let heavier = |reason, heavier| Need {
            heavier,
            ..Need::new(Vec3::ZERO, reason)
        };
        let point = Reason::Island { area_mm2: 0.0 };
        assert_eq!(heavier(point, 1).size(), 1, "one size up from the lightest");
        assert_eq!(
            heavier(point, 9).size(),
            AUTO_SIZES - 1,
            "no further than the heaviest"
        );
        assert_eq!(
            heavier(Reason::Steady, 1).size(),
            AUTO_SIZES,
            "steadying stays so"
        );
    }
}
