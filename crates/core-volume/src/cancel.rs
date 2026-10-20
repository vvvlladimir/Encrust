use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering::Relaxed};

/// Whether the caller has given up on a run, asked between the pieces of work it is made
/// of, and how far the run has got, told at the same places.
///
/// A field over a real model takes seconds, and a Ctrl-C has to land inside them rather
/// than after, the way placement already stops inside a block of layers (ADR 0033). A run
/// nothing stops passes [`Cancel::never`]; one nobody watches has no [`Progress`] (ADR 0317).
#[derive(Clone, Copy, Default)]
pub struct Cancel<'a> {
    asked: Option<&'a (dyn Fn() -> bool + Sync)>,
    progress: Option<&'a Progress>,
}

impl<'a> Cancel<'a> {
    /// A run that is never given up on.
    pub const fn never() -> Self {
        Self {
            asked: None,
            progress: None,
        }
    }

    /// A run that stops once `asked` answers true.
    pub const fn when(asked: &'a (dyn Fn() -> bool + Sync)) -> Self {
        Self {
            asked: Some(asked),
            progress: None,
        }
    }

    /// The same run, telling `progress` how far it has got.
    #[must_use]
    pub const fn reporting(self, progress: &'a Progress) -> Self {
        Self {
            progress: Some(progress),
            ..self
        }
    }

    /// Whether the run has been given up on. Asked often, so it does no work of its own.
    pub fn asked(&self) -> bool {
        self.asked.is_some_and(|asked| asked())
    }

    /// The run has started `step`, made of `total` equal pieces, or of none it can count.
    pub(crate) fn begin(&self, step: Step, total: usize) {
        if let Some(progress) = self.progress {
            progress.total.store(total, Relaxed);
            progress.done.store(0, Relaxed);
            progress.step.store(step as u8 + 1, Relaxed);
        }
    }

    /// `pieces` more of the step in hand are done.
    pub(crate) fn advance(&self, pieces: usize) {
        if let Some(progress) = self.progress {
            progress.done.fetch_add(pieces, Relaxed);
        }
    }
}

/// The steps a field and what is cut from it go through, in order, and those of an exact
/// boolean, which a run takes instead of a field or before falling back to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Step {
    /// Checking the mesh and working out its normals, before any lattice point.
    Preparing,
    /// The exact nearest faces right at the surface.
    Seeding,
    /// Carrying those faces out across the wall, counted in the tiles of its rings.
    Carrying,
    /// The stored tiles' values, tile by tile: most of a build.
    Refining,
    /// The surface read out of the field, layer of tiles by layer.
    Surfacing,
    /// The shell made of that surface: the floor, the lattice, the parts.
    Shaping,
    /// Models read into solids for an exact boolean, model by model.
    Reading,
    /// The exact boolean itself, which counts nothing inside it.
    Combining,
    /// The exact result made back into a mesh.
    Finishing,
}

impl Step {
    /// Every step, in the order of its discriminant.
    const ALL: [Self; 9] = [
        Self::Preparing,
        Self::Seeding,
        Self::Carrying,
        Self::Refining,
        Self::Surfacing,
        Self::Shaping,
        Self::Reading,
        Self::Combining,
        Self::Finishing,
    ];
}

/// How far a run has got: which step, and how many of its pieces are done. Written by the
/// run from any of its threads and read by whoever watches, at whatever pace.
#[derive(Debug, Default)]
pub struct Progress {
    /// The step in hand plus one, or zero before the run has begun one.
    step: AtomicU8,
    done: AtomicUsize,
    total: AtomicUsize,
}

impl Progress {
    /// Back to before the first step, for the next model of a run.
    pub fn restart(&self) {
        self.step.store(0, Relaxed);
        self.total.store(0, Relaxed);
        self.done.store(0, Relaxed);
    }

    /// The step in hand, its pieces done, and how many it has, or `None` before the run
    /// has begun a step: a run that counts none never leaves it. A total of zero is a step
    /// that cannot be counted.
    pub fn read(&self) -> Option<(Step, usize, usize)> {
        let step = *Step::ALL.get(usize::from(self.step.load(Relaxed)).checked_sub(1)?)?;
        let total = self.total.load(Relaxed);
        Some((step, self.done.load(Relaxed).min(total), total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watched_run_reports_its_step_and_pieces() {
        let progress = Progress::default();
        let run = Cancel::never().reporting(&progress);
        run.begin(Step::Refining, 4);
        run.advance(1);
        run.advance(2);
        assert_eq!(progress.read(), Some((Step::Refining, 3, 4)));

        run.begin(Step::Surfacing, 0);
        assert_eq!(progress.read(), Some((Step::Surfacing, 0, 0)));

        progress.restart();
        assert_eq!(progress.read(), None, "a run not yet begun has no step");
    }

    #[test]
    fn an_unwatched_run_reports_nothing() {
        let run = Cancel::never();
        run.begin(Step::Refining, 4);
        run.advance(1);
        assert!(!run.asked());
    }
}
