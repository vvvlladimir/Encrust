use core_geometry::Scalar;
use serde::{Deserialize, Serialize};

use crate::error::JointError;

/// How the two halves of a cut are joined, and what every kind is set to, so switching
/// between kinds keeps each one's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Joints {
    /// The kind a cut is joined by, or `None` for a plain cut.
    pub kind: Option<JointKind>,
    /// What pins and dowels are set to.
    pub pegs: Pegs,
    pub magnets: Magnets,
    pub key: Key,
}

impl Joints {
    /// Refuses a size of the kind chosen that is not finite, or not above nought — at or
    /// above it for a clearance — as a project file or a typed field may carry.
    pub fn check(&self) -> Result<(), JointError> {
        let sizes: &[(&'static str, Scalar, bool)] = match self.kind {
            None => &[],
            Some(JointKind::Pins | JointKind::Dowels) => &[
                ("peg diameter", self.pegs.diameter_mm, false),
                ("peg length", self.pegs.length_mm, false),
                ("peg clearance", self.pegs.clearance_mm, true),
            ],
            Some(JointKind::Magnets) => &[
                ("magnet diameter", self.magnets.diameter_mm, false),
                ("magnet thickness", self.magnets.thickness_mm, false),
                ("magnet clearance", self.magnets.clearance_mm, true),
            ],
            Some(JointKind::Key) => &[
                ("key width", self.key.width_mm, false),
                ("key height", self.key.height_mm, false),
                ("key clearance", self.key.clearance_mm, true),
            ],
        };
        let unbuildable = |&&(_, value, nought): &&(&str, Scalar, bool)| {
            let above = if nought { value >= 0.0 } else { value > 0.0 };
            !value.is_finite() || !above
        };
        match sizes.iter().find(unbuildable) {
            Some(&(what, value_mm, _)) => Err(JointError::BadSize { what, value_mm }),
            None => Ok(()),
        }
    }
}

/// What holds the halves together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JointKind {
    /// Pegs standing on the lower half, into sockets in the upper.
    Pins,
    /// Sockets in both halves, and loose dowels printed beside them.
    Dowels,
    /// Sockets in both halves for disc magnets.
    Magnets,
    /// A tongue round the lower half's cut face, into a groove in the upper.
    Key,
}

impl JointKind {
    /// Every kind, in the order a picker lists them.
    pub const ALL: [Self; 4] = [Self::Dowels, Self::Pins, Self::Magnets, Self::Key];
}

/// Round pegs, standing on a half as pins or loose as dowels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pegs {
    /// Pegs on each separate piece of the cut face.
    pub per_piece: u32,
    /// Sized from the width of each piece of the face, the numbers below set aside.
    pub auto_size: bool,
    pub diameter_mm: Scalar,
    pub length_mm: Scalar,
    /// Gap between a peg and its socket, millimetres a side.
    pub clearance_mm: Scalar,
    /// Whether one peg is narrower than the rest, so the halves go together one way only.
    pub keyed: bool,
}

impl Default for Pegs {
    fn default() -> Self {
        Self {
            per_piece: 2,
            auto_size: true,
            diameter_mm: 4.0,
            length_mm: 6.0,
            clearance_mm: 0.1,
            keyed: true,
        }
    }
}

/// Disc magnets, one socket for each in either half.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Magnets {
    pub per_piece: u32,
    pub diameter_mm: Scalar,
    pub thickness_mm: Scalar,
    /// Gap between a magnet and its socket's wall, millimetres a side.
    pub clearance_mm: Scalar,
}

impl Default for Magnets {
    fn default() -> Self {
        Self {
            per_piece: 2,
            diameter_mm: 5.0,
            thickness_mm: 2.0,
            clearance_mm: 0.1,
        }
    }
}

/// A tongue following the edge of the cut face, for a face too thin to take a peg.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Key {
    /// Across the tongue, millimetres; a wall thinner than it takes a narrower one.
    pub width_mm: Scalar,
    /// How far the tongue stands off the cut, millimetres.
    pub height_mm: Scalar,
    /// Gap between the tongue and its groove, millimetres a side.
    pub clearance_mm: Scalar,
}

impl Default for Key {
    fn default() -> Self {
        Self {
            width_mm: 1.2,
            height_mm: 2.0,
            clearance_mm: 0.1,
        }
    }
}
