//! The raft under a model as it prints — its holes taken out of it — with a hierarchy, so the
//! viewport shows a hole open through it and a click can drill one; a hole never cuts a support
//! tree (ADR 0441). See `docs/design/hollowing.md`.

use std::sync::{Arc, Weak};

use core_geometry::{Bvh, Mesh, Transform};
use core_supports::Shaded;

/// A model's raft as it prints and its hierarchy, and what it was built from, held weakly so
/// a replaced raft or cut is let go and no longer matches.
#[derive(Debug, Clone, Default)]
pub struct PrintedSupports {
    from: Option<BuiltFrom>,
    raft: Option<(Shaded, Arc<Bvh>)>,
    /// What a run was lost for, so the same raft is not run again every frame.
    lost: Option<BuiltFrom>,
}

/// What a model's printed raft is built from: its raft, its cuts, where it stands.
#[derive(Debug, Clone, Copy)]
pub struct SupportsUnder<'a> {
    pub raft: Option<&'a Shaded>,
    pub cuts: Option<&'a Arc<Mesh>>,
    pub transform: Transform,
}

/// The raft, the cuts and the placement a printed raft was built for.
#[derive(Debug, Clone)]
pub struct BuiltFrom {
    raft: Option<Weak<Mesh>>,
    cuts: Option<Weak<Mesh>>,
    transform: Transform,
}

/// Whether `was` still points at `now`, both absent alike.
fn same(was: Option<&Weak<Mesh>>, now: Option<&Arc<Mesh>>) -> bool {
    match (was, now) {
        (Some(was), Some(now)) => was.as_ptr() == Arc::as_ptr(now),
        (None, None) => true,
        _ => false,
    }
}

impl BuiltFrom {
    /// A note of `under`, kept beside what is built from it.
    pub fn of(under: SupportsUnder<'_>) -> Self {
        Self {
            raft: under.raft.map(|raft| Arc::downgrade(&raft.mesh)),
            cuts: under.cuts.map(Arc::downgrade),
            transform: under.transform,
        }
    }

    /// Whether `under` is still what this was taken of.
    pub fn matches(&self, under: SupportsUnder<'_>) -> bool {
        same(self.cuts.as_ref(), under.cuts)
            && same(self.raft.as_ref(), under.raft.map(|raft| &raft.mesh))
            && self.transform == under.transform
    }
}

impl PrintedSupports {
    /// What the raft `under` is drawn as: drilled where it was built for it, as it stands
    /// otherwise.
    pub fn shown<'a>(&'a self, under: SupportsUnder<'a>) -> Option<&'a Shaded> {
        match self.built(under) {
            Some((raft, _)) => Some(raft),
            None => under.raft,
        }
    }

    /// The raft as it prints and its hierarchy, where it was built for `under`.
    pub fn built(&self, under: SupportsUnder<'_>) -> Option<(&Shaded, &Arc<Bvh>)> {
        self.from
            .as_ref()
            .filter(|from| from.matches(under))
            .and(self.raft.as_ref())
            .map(|(raft, bvh)| (raft, bvh))
    }

    /// Whether a raft stands `under` that nothing has been built for, nor a run lost for.
    pub fn wants(&self, under: SupportsUnder<'_>) -> bool {
        under.raft.is_some()
            && self.built(under).is_none()
            && !self.lost.as_ref().is_some_and(|lost| lost.matches(under))
    }

    /// Takes over what `other` built where nothing has been here; [`Self::built`] still
    /// reads it only for the raft and cuts it was made for.
    pub fn adopt(&mut self, other: &Self) {
        if self.from.is_none() {
            self.clone_from(other);
        }
    }

    /// Keeps the raft as it prints and its hierarchy, built `from` what it stands on.
    pub fn keep(&mut self, from: BuiltFrom, raft: Shaded, bvh: Arc<Bvh>) {
        self.from = Some(from);
        self.raft = Some((raft, bvh));
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

    fn one(mesh: Mesh) -> Shaded {
        shaded(vec![mesh]).pop().expect("one mesh shades")
    }

    #[test]
    fn a_printed_raft_is_shown_only_for_the_raft_cuts_and_placement_it_was_built_for() {
        let raft = one(cuboid(Vec3::ZERO, Vec3::ONE));
        let drilled = one(cuboid(Vec3::ZERO, Vec3::splat(0.5)));
        let bvh = Arc::new(Bvh::build(&drilled.mesh));
        let cuts = Arc::new(cuboid(Vec3::ZERO, Vec3::ONE));
        let placed = Transform::default();
        let under = |cuts, transform| SupportsUnder {
            raft: Some(&raft),
            cuts,
            transform,
        };
        let mut supports = PrintedSupports::default();
        assert!(supports.wants(under(Some(&cuts), placed)));

        supports.keep(
            BuiltFrom::of(under(Some(&cuts), placed)),
            drilled.clone(),
            bvh,
        );
        let shown = supports.shown(under(Some(&cuts), placed)).expect("a raft");
        assert!(Arc::ptr_eq(&shown.mesh, &drilled.mesh));
        assert!(!supports.wants(under(Some(&cuts), placed)));

        let moved = Transform::from_translation(Vec3::X);
        let other_cuts = Arc::new(cuboid(Vec3::ZERO, Vec3::ONE));
        for (cuts, placed, what) in [
            (Some(&other_cuts), placed, "cuts since changed"),
            (Some(&cuts), moved, "a model since moved"),
            (None, placed, "holes since cleared"),
        ] {
            let shown = supports.shown(under(cuts, placed)).expect("a raft");
            assert!(
                Arc::ptr_eq(&shown.mesh, &raft.mesh),
                "{what} leave the raft drawn as it stands"
            );
            assert!(supports.wants(under(cuts, placed)), "{what} build it again");
        }
    }

    #[test]
    fn a_lost_run_is_not_started_again_until_what_it_was_for_changes() {
        let raft = one(cuboid(Vec3::ZERO, Vec3::ONE));
        let under = |transform| SupportsUnder {
            raft: Some(&raft),
            cuts: None,
            transform,
        };
        let mut supports = PrintedSupports::default();
        supports.lose(BuiltFrom::of(under(Transform::default())));
        assert!(
            !supports.wants(under(Transform::default())),
            "the same raft would be lost again"
        );
        assert!(
            supports.wants(under(Transform::from_translation(Vec3::X))),
            "a model since moved is tried again"
        );
    }
}
