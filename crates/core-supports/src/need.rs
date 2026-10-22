use core_geometry::{Scalar, Vec3};

/// A contact automatic placement wants, and what it holds there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Need {
    pub contact: Vec3,
    pub reason: Reason,
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
    /// A part held at too few places to stand steady.
    Steady,
}
