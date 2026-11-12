use core_geometry::{Scalar, Vec3};
use printer_profiles::{AUTO_SIZES, AutoProfile};

/// A contact automatic placement wants, and what it holds there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Need {
    pub contact: Vec3,
    pub reason: Reason,
    /// Sizes heavier than its reason asks, where what it carries proved too much for that.
    pub heavier: u8,
    /// On the side the user painted to be seen: built lightest, and given a neighbour
    /// rather than a heavier tip (ADR 0425).
    pub shown: bool,
}

impl Reason {
    /// The first ground of a part that begins at a point.
    pub const POINT: Self = Self::Island {
        area_mm2: 0.0,
        share_mm3: 0.0,
    };
}

impl Need {
    /// A contact at `contact` wanted for `reason`, sized by it alone.
    pub fn new(contact: Vec3, reason: Reason) -> Self {
        Self {
            contact,
            reason,
            heavier: 0,
            shown: false,
        }
    }

    /// Which of `auto`'s shapes a support here is built to, lightest first, with
    /// [`AUTO_SIZES`] for its steadying one and the lightest on the side meant to be seen.
    pub fn size_in(&self, auto: &AutoProfile) -> usize {
        if self.shown && self.reason != Reason::Steady {
            return 0;
        }
        let asked = match self.reason {
            Reason::Island { share_mm3, .. } | Reason::Overhang { share_mm3 } => {
                auto.share_size(share_mm3)
            }
            Reason::Step { new_mm3 } => auto.step_size(new_mm3).unwrap_or(0),
            Reason::Steady => return AUTO_SIZES,
        };
        (asked + usize::from(self.heavier)).min(AUTO_SIZES - 1)
    }
}

/// Why a contact is wanted, measured by what it holds: what an automatic profile sizes a
/// support by. See `docs/decisions/0325`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reason {
    /// The first ground of a part nothing holds, an island of `area_mm2` on the layer it
    /// begins on, `share_mm3` as an overhang's; a part that begins at a point has neither.
    Island { area_mm2: Scalar, share_mm3: Scalar },
    /// An overhang, with `share_mm3` the model above the contact's layer shared out among the
    /// contacts holding it, its own and every one above it (ADR 0423).
    Overhang { share_mm3: Scalar },
    /// A step, or a face its reach takes in above it, with `new_mm3` the resin the step put out in
    /// its one layer; only a run sizing its supports asks for steps apart (ADR 0326).
    Step { new_mm3: Scalar },
    /// A support that steadies a part: one held at too few places to stand, or, in a run
    /// sizing its supports, a slope too gentle to sag (ADR 0326).
    Steady,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn a_need_is_sized_by_its_share_and_a_steadying_one_last() {
        let auto = AutoProfile::from_toml_str(
            include_str!("../../../assets/profiles/auto/default.toml"),
            Path::new("auto/default.toml"),
        )
        .expect("the shipped profile reads");
        let need = |reason| Need::new(Vec3::ZERO, reason);
        assert_eq!(
            need(Reason::Island {
                area_mm2: 3.2,
                share_mm3: 500.0
            })
            .size_in(&auto),
            2,
            "an island contact holding 500 mm³ reaches the shipped heaviest threshold"
        );
        assert_eq!(need(Reason::Steady).size_in(&auto), AUTO_SIZES);
    }

    #[test]
    fn a_need_made_heavier_takes_a_heavier_size_but_never_the_steadying_one() {
        let auto = AutoProfile::from_toml_str(
            include_str!("../../../assets/profiles/auto/default.toml"),
            Path::new("auto/default.toml"),
        )
        .expect("the shipped profile reads");
        let heavier = |reason, heavier| Need {
            heavier,
            ..Need::new(Vec3::ZERO, reason)
        };
        let point = Reason::POINT;
        assert_eq!(
            heavier(point, 1).size_in(&auto),
            1,
            "one size up from the lightest"
        );
        assert_eq!(
            heavier(point, 9).size_in(&auto),
            AUTO_SIZES - 1,
            "no further than the heaviest"
        );
        assert_eq!(
            heavier(Reason::Steady, 1).size_in(&auto),
            AUTO_SIZES,
            "steadying stays so"
        );
    }
}
