//! The support trees and raft under a model as they print — its holes taken out of them —
//! with a hierarchy each, so the viewport shows a hole open through them and a click can
//! drill one; see `docs/design/hollowing.md`.

use std::sync::{Arc, Weak};

use core_geometry::{Bvh, Mesh, Transform};
use core_supports::Shaded;

/// A model's support groups as they print and their hierarchies, and what they were built
/// from, held weakly so a replaced group or cut is let go and no longer matches.
#[derive(Debug, Clone, Default)]
pub struct PrintedSupports {
    from: Option<BuiltFrom>,
    shaded: Vec<Shaded>,
    bvhs: Vec<Arc<Bvh>>,
    /// What a run was lost for, so the same groups are not run again every frame.
    lost: Option<BuiltFrom>,
}

/// What a model's printed supports are built from: its groups, its cuts, where it stands.
#[derive(Debug, Clone, Copy)]
pub struct SupportsUnder<'a> {
    pub groups: &'a [Shaded],
    pub cuts: Option<&'a Arc<Mesh>>,
    pub transform: Transform,
}

/// The groups, the cuts and the placement printed supports were built for.
#[derive(Debug, Clone)]
pub struct BuiltFrom {
    groups: Vec<Weak<Mesh>>,
    cuts: Option<Weak<Mesh>>,
    transform: Transform,
}

impl BuiltFrom {
    /// A note of `under`, kept beside what is built from it.
    pub fn of(under: SupportsUnder<'_>) -> Self {
        Self {
            groups: under
                .groups
                .iter()
                .map(|group| Arc::downgrade(&group.mesh))
                .collect(),
            cuts: under.cuts.map(Arc::downgrade),
            transform: under.transform,
        }
    }

    /// Whether `under` is still what this was taken of.
    pub fn matches(&self, under: SupportsUnder<'_>) -> bool {
        let same_cuts = match (&self.cuts, under.cuts) {
            (Some(was), Some(now)) => was.as_ptr() == Arc::as_ptr(now),
            (None, None) => true,
            _ => false,
        };
        same_cuts
            && self.transform == under.transform
            && self.groups.len() == under.groups.len()
            && self
                .groups
                .iter()
                .zip(under.groups)
                .all(|(was, now)| was.as_ptr() == Arc::as_ptr(&now.mesh))
    }
}

impl PrintedSupports {
    /// What the groups `under` are drawn as: drilled where they were built for it, as they
    /// stand otherwise.
    pub fn shown<'a>(&'a self, under: SupportsUnder<'a>) -> &'a [Shaded] {
        self.built(under).map_or(under.groups, |(shaded, _)| shaded)
    }

    /// The groups as they print and their hierarchies, where they were built for `under`.
    pub fn built(&self, under: SupportsUnder<'_>) -> Option<(&[Shaded], &[Arc<Bvh>])> {
        self.from
            .as_ref()
            .filter(|from| from.matches(under))
            .map(|_| (self.shaded.as_slice(), self.bvhs.as_slice()))
    }

    /// Whether groups stand `under` that nothing has been built for, nor a run lost for.
    pub fn wants(&self, under: SupportsUnder<'_>) -> bool {
        !under.groups.is_empty()
            && self.built(under).is_none()
            && !self.lost.as_ref().is_some_and(|lost| lost.matches(under))
    }

    /// Takes over what `other` built where nothing has been here; [`Self::built`] still
    /// reads it only for the groups and cuts it was made for.
    pub fn adopt(&mut self, other: &Self) {
        if self.from.is_none() {
            self.clone_from(other);
        }
    }

    /// Keeps the groups as they print and their hierarchies, built `from` what they stand on.
    pub fn keep(&mut self, from: BuiltFrom, shaded: Vec<Shaded>, bvhs: Vec<Arc<Bvh>>) {
        self.from = Some(from);
        self.shaded = shaded;
        self.bvhs = bvhs;
    }

    /// Notes that the run `from` this was lost, so it is not started again until that changes.
    pub fn lose(&mut self, from: BuiltFrom) {
        self.lost = Some(from);
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid};
    use core_supports::shaded;

    use super::*;

    #[test]
    fn printed_groups_are_shown_only_for_the_groups_cuts_and_placement_they_were_built_for() {
        let groups = shaded(vec![cuboid(Vec3::ZERO, Vec3::ONE)]);
        let drilled = shaded(vec![cuboid(Vec3::ZERO, Vec3::splat(0.5))]);
        let bvhs = vec![Arc::new(Bvh::build(&drilled[0].mesh))];
        let cuts = Arc::new(cuboid(Vec3::ZERO, Vec3::ONE));
        let placed = Transform::default();
        let under = |cuts, transform| SupportsUnder {
            groups: &groups,
            cuts,
            transform,
        };
        let mut supports = PrintedSupports::default();
        assert!(supports.wants(under(Some(&cuts), placed)));

        supports.keep(
            BuiltFrom::of(under(Some(&cuts), placed)),
            drilled.clone(),
            bvhs,
        );
        let shown = supports.shown(under(Some(&cuts), placed));
        assert!(Arc::ptr_eq(&shown[0].mesh, &drilled[0].mesh));
        assert!(!supports.wants(under(Some(&cuts), placed)));

        let moved = Transform::from_translation(Vec3::X);
        let other_cuts = Arc::new(cuboid(Vec3::ZERO, Vec3::ONE));
        for (cuts, placed, what) in [
            (Some(&other_cuts), placed, "cuts since changed"),
            (Some(&cuts), moved, "a model since moved"),
            (None, placed, "holes since cleared"),
        ] {
            let shown = supports.shown(under(cuts, placed));
            assert!(
                Arc::ptr_eq(&shown[0].mesh, &groups[0].mesh),
                "{what} leave the supports drawn as they stand"
            );
            assert!(
                supports.wants(under(cuts, placed)),
                "{what} build them again"
            );
        }
    }

    #[test]
    fn a_lost_run_is_not_started_again_until_what_it_was_for_changes() {
        let groups = shaded(vec![cuboid(Vec3::ZERO, Vec3::ONE)]);
        let under = |transform| SupportsUnder {
            groups: &groups,
            cuts: None,
            transform,
        };
        let mut supports = PrintedSupports::default();
        supports.lose(BuiltFrom::of(under(Transform::default())));
        assert!(
            !supports.wants(under(Transform::default())),
            "the same groups would be lost again"
        );
        assert!(
            supports.wants(under(Transform::from_translation(Vec3::X))),
            "a model since moved is tried again"
        );
    }
}
