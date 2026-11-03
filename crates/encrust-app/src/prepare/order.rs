//! The steps of a run, in the order they run: the command line's, turned before it is
//! repaired, shaped before it is hollowed.

use crate::i18n::tr;

/// The steps in the order they run, the command line's: turned before it is repaired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Orient,
    Repair,
    /// Faces and shells changed by hand, then the modelling, each by the window's own tool.
    Mend,
    Relief,
    Combine,
    Label,
    Cut,
    Split,
    Arrange,
    Hollow,
    /// Holes drilled at the points a plan gives, before any are searched for.
    Drill,
    Drain,
    Supports,
    Cavity,
    /// Holes drilled into the supports, once they are built.
    SupportHoles,
}

impl Step {
    pub(super) const ALL: [Self; 15] = [
        Self::Orient,
        Self::Repair,
        Self::Mend,
        Self::Relief,
        Self::Combine,
        Self::Label,
        Self::Cut,
        Self::Split,
        Self::Arrange,
        Self::Hollow,
        Self::Drill,
        Self::Drain,
        Self::Supports,
        Self::Cavity,
        Self::SupportHoles,
    ];

    pub(super) fn index(self) -> usize {
        Self::ALL.iter().position(|step| *step == self).unwrap_or(0)
    }

    pub(super) fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }

    /// What the bar says while it runs.
    pub fn label(self) -> String {
        match self {
            Self::Orient => tr!("prepare-step-orient"),
            Self::Repair => tr!("prepare-step-repair"),
            Self::Mend => tr!("prepare-step-mend"),
            Self::Relief => tr!("prepare-step-relief"),
            Self::Combine => tr!("prepare-step-combine"),
            Self::Label => tr!("prepare-step-label"),
            Self::Cut => tr!("prepare-step-cut"),
            Self::Split => tr!("prepare-step-split"),
            Self::Arrange => tr!("prepare-step-arrange"),
            Self::Hollow => tr!("prepare-step-hollow"),
            Self::Drill => tr!("prepare-step-drill"),
            Self::Drain => tr!("prepare-step-drain"),
            Self::Supports => tr!("prepare-step-supports"),
            Self::Cavity => tr!("prepare-step-cavity"),
            Self::SupportHoles => tr!("prepare-step-support-holes"),
        }
    }
}
