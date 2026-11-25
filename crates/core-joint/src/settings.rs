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

/// What a front end asks of a joint beyond its kind, each absent one left at the default.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct JointAsked {
    /// Pegs or magnets on each separate piece of the cut face.
    pub per_piece: Option<u32>,
    /// Gap a side between a peg, magnet or tongue and its socket, mm.
    pub clearance_mm: Option<Scalar>,
    /// A peg's diameter and length, mm, in place of sizing it from the face.
    pub peg_mm: Option<[Scalar; 2]>,
    /// A magnet's diameter and thickness, mm.
    pub magnet_mm: Option<[Scalar; 2]>,
    /// The tongue's width and height, mm.
    pub key_mm: Option<[Scalar; 2]>,
    /// Every peg the same width, so the halves go together either way.
    pub unkeyed: bool,
}

impl Joints {
    /// A joint of `kind` sized as `asked`, the window's defaults for the rest.
    pub fn asked(kind: Option<JointKind>, asked: &JointAsked) -> Self {
        let mut joints = Self {
            kind,
            ..Self::default()
        };
        if let Some(count) = asked.per_piece {
            (joints.pegs.per_piece, joints.magnets.per_piece) = (count, count);
        }
        if let Some(mm) = asked.clearance_mm {
            joints.pegs.clearance_mm = mm;
            joints.magnets.clearance_mm = mm;
            joints.key.clearance_mm = mm;
        }
        if let Some([diameter_mm, length_mm]) = asked.peg_mm {
            (joints.pegs.diameter_mm, joints.pegs.length_mm) = (diameter_mm, length_mm);
            joints.pegs.auto_size = false;
        }
        if let Some([diameter_mm, thickness_mm]) = asked.magnet_mm {
            (joints.magnets.diameter_mm, joints.magnets.thickness_mm) = (diameter_mm, thickness_mm);
        }
        if let Some([width_mm, height_mm]) = asked.key_mm {
            (joints.key.width_mm, joints.key.height_mm) = (width_mm, height_mm);
        }
        joints.pegs.keyed &= !asked.unkeyed;
        joints
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_joint_asked_by_its_sizes_takes_them_and_the_defaults_for_the_rest() {
        let asked = JointAsked {
            per_piece: Some(3),
            clearance_mm: Some(0.2),
            peg_mm: Some([5.0, 8.0]),
            unkeyed: true,
            ..JointAsked::default()
        };
        let joints = Joints::asked(Some(JointKind::Dowels), &asked);
        assert_eq!(joints.kind, Some(JointKind::Dowels));
        assert_eq!(
            joints.pegs,
            Pegs {
                per_piece: 3,
                auto_size: false,
                diameter_mm: 5.0,
                length_mm: 8.0,
                clearance_mm: 0.2,
                keyed: false,
            }
        );
        assert_eq!(joints.magnets.per_piece, 3);
        assert_eq!(
            joints.key,
            Key {
                clearance_mm: 0.2,
                ..Key::default()
            }
        );
    }

    #[test]
    fn a_joint_asked_nothing_is_the_default() {
        assert_eq!(
            Joints::asked(None, &JointAsked::default()),
            Joints::default()
        );
    }
}
