use core_geometry::{Mesh, Vec3};

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
}

impl<'a> Printed<'a> {
    /// A model with no cavity and nothing cut into it.
    pub fn solid(outer: &'a Mesh) -> Self {
        Self {
            outer,
            shell: None,
            cuts: None,
            vents: &[],
        }
    }
}

#[cfg(test)]
mod tests;
