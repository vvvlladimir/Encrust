use printer_profiles::SupportProfile;

/// The profile each group of supports is built to.
///
/// A support carries the number of its group, not a profile of its own: two supports of
/// one group are the same shape, and a support is retuned by moving it to another group.
/// Group 0 is the one everything falls back to, and the one the raft and the bracing —
/// which belong to the plate rather than to any one support — are built from.
/// See `docs/decisions/0094`.
#[derive(Debug, Clone, Copy)]
pub struct Profiles<'a>(&'a [SupportProfile]);

impl<'a> Profiles<'a> {
    /// A plate whose supports are all one shape.
    pub fn single(profile: &'a SupportProfile) -> Self {
        Self(std::slice::from_ref(profile))
    }

    /// One profile per group, in group order, or `None` for a table with no groups in it
    /// at all: there is no shape to build a support to then.
    pub fn new(profiles: &'a [SupportProfile]) -> Option<Self> {
        (!profiles.is_empty()).then_some(Self(profiles))
    }

    /// The profile a support of `group` is built to, or group 0's when the table is
    /// shorter than that.
    pub fn of(&self, group: u16) -> &'a SupportProfile {
        self.0.get(group as usize).unwrap_or(&self.0[0])
    }

    /// The profile the plate itself is built to: the raft under the supports and the
    /// bracing between them.
    pub fn ground(&self) -> &'a SupportProfile {
        self.of(0)
    }

    /// Every group in turn, as its number and its profile.
    pub fn groups(&self) -> impl Iterator<Item = (u16, &'a SupportProfile)> {
        self.0
            .iter()
            .enumerate()
            .map(|(group, profile)| (group as u16, profile))
    }
}
