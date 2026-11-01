use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use crate::error::PlateError;

/// The part of an orientation search in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The candidates scored on the mesh and the shortlist cut: a fraction of a second.
    Looking,
    /// The finalists printed to choose between them: most of the search's time.
    Proving,
}

/// Where a bar stands when the shortlist is cut: weighed on three models, whose search took
/// 3 to 19 % of a run and whose proof took the rest; see ADR 0384.
const LOOKING_END: f32 = 0.12;

impl Phase {
    /// The share of a whole search done once `done` of `total` pieces of this phase are.
    pub fn share(self, done: usize, total: usize) -> f32 {
        let within = done.min(total.max(1)) as f32 / total.max(1) as f32;
        match self {
            Self::Looking => LOOKING_END * within,
            Self::Proving => LOOKING_END + (1.0 - LOOKING_END) * within,
        }
    }
}

/// Whether the caller has given up on a search, and how far it has got, told at the same
/// places. A search nobody watches passes [`Watch::never`].
#[derive(Clone, Copy, Default)]
pub struct Watch<'a> {
    told: Option<&'a (dyn Fn(Phase, usize, usize) -> bool + Sync)>,
}

impl<'a> Watch<'a> {
    /// A search that is never given up on and tells nobody.
    pub const fn never() -> Self {
        Self { told: None }
    }

    /// A search that tells `told` its phase, the pieces done and the pieces it has, and
    /// stops once `told` answers false.
    pub const fn by(told: &'a (dyn Fn(Phase, usize, usize) -> bool + Sync)) -> Self {
        Self { told: Some(told) }
    }

    /// Reports `done` of `total` pieces of `phase`, or stops the search where the watcher
    /// has given up on it.
    pub fn step(&self, phase: Phase, done: usize, total: usize) -> Result<(), PlateError> {
        match self.told {
            Some(told) if !told(phase, done, total) => Err(PlateError::Cancelled),
            _ => Ok(()),
        }
    }
}

/// The pieces of one phase, finished on any thread and counted as they come.
pub struct Pieces<'a> {
    watch: Watch<'a>,
    phase: Phase,
    total: usize,
    done: AtomicUsize,
}

impl<'a> Pieces<'a> {
    /// Begins `phase` of `total` pieces, none done.
    pub fn start(watch: Watch<'a>, phase: Phase, total: usize) -> Result<Self, PlateError> {
        watch.step(phase, 0, total)?;
        Ok(Self {
            watch,
            phase,
            total,
            done: AtomicUsize::new(0),
        })
    }

    /// One more piece is done.
    pub fn finish(&self) -> Result<(), PlateError> {
        let done = self.done.fetch_add(1, Relaxed) + 1;
        self.watch.step(self.phase, done, self.total)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn pieces_are_told_in_order_and_a_refusal_stops_the_search() {
        let seen = Mutex::new(Vec::new());
        let told = |phase, done, total| {
            seen.lock()
                .expect("no thread panics holding it")
                .push((phase, done, total));
            done < 2
        };
        let pieces = Pieces::start(Watch::by(&told), Phase::Proving, 3).expect("none done yet");
        pieces.finish().expect("one of three is no reason to stop");
        assert!(matches!(pieces.finish(), Err(PlateError::Cancelled)));
        assert_eq!(
            *seen.lock().expect("no thread panics holding it"),
            [
                (Phase::Proving, 0, 3),
                (Phase::Proving, 1, 3),
                (Phase::Proving, 2, 3)
            ]
        );
    }

    #[test]
    fn a_search_never_steps_back_from_looking_to_proving() {
        assert!(
            (Phase::Looking.share(0, 4) - 0.0).abs() < 1e-6,
            "nothing done is 0"
        );
        let looked = Phase::Looking.share(4, 4);
        assert!(
            (Phase::Proving.share(0, 3) - looked).abs() < 1e-6,
            "proving starts there"
        );
        assert!(
            (Phase::Proving.share(3, 3) - 1.0).abs() < 1e-6,
            "all proven is 1"
        );
    }

    #[test]
    fn a_search_nobody_watches_is_never_stopped() {
        let pieces = Pieces::start(Watch::never(), Phase::Looking, 1).expect("nobody asks");
        assert!(pieces.finish().is_ok());
    }
}
