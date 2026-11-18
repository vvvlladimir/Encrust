use std::borrow::Cow;

use core_geometry::{Mesh, Prominence, Region, Vec3};

/// A model as it prints, in its own space: what an orientation is judged on once the
/// model has been hollowed or drilled.
#[derive(Debug, Clone, Copy)]
pub struct Printed<'a> {
    /// The outer surface, which is what supports stand under.
    pub outer: &'a Mesh,
    /// The hollowed model, its cavity wound inward, where it has one.
    pub shell: Option<&'a Mesh>,
    /// The holes and channels cut into it, wound inward.
    pub cuts: Option<&'a Mesh>,
    /// The way out of each drain hole. A hole lets air into a cavity from the first layer
    /// only when it points at the plate, so each of these is tried as the way down.
    pub vents: &'a [Vec3],
    /// The faces of `outer` the user painted as what shows, where any are painted.
    pub shown: Option<&'a Region>,
    /// How much each face of `outer` shows, where it is read already off `shown` and the
    /// shape: the search and the proof then take it from here rather than read it again.
    pub shows: Option<&'a Prominence>,
}

impl<'a> Printed<'a> {
    /// A model with no cavity and nothing cut into it.
    pub fn solid(outer: &'a Mesh) -> Self {
        Self {
            outer,
            shell: None,
            cuts: None,
            vents: &[],
            shown: None,
            shows: None,
        }
    }

    /// How much each face of `outer` shows: [`Self::shows`] where it is given, and read now
    /// off what is painted and the shape where not.
    pub fn prominence(&self) -> Cow<'a, Prominence> {
        self.shows.map_or_else(
            || Cow::Owned(Prominence::shown(self.outer, self.shown)),
            Cow::Borrowed,
        )
    }
}

#[cfg(test)]
mod tests;
