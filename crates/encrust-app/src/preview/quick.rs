//! The quick check run in the background: the stack read in squares of `QUICK_CHECK_BIN`
//! pixels once the plate has stood still, on half the cores (ADR 0419).

use std::sync::Arc;
use std::time::Duration;

use core_analysis::Measured;
use core_engine::QUICK_CHECK_BIN;
use core_raster::RasterSettings;
use web_time::Instant;

use super::{Fold, MeasureKey, Preview, as_seen};
use crate::job::{MeasureJob, MeasureOutcome, cores};

/// How long the plate stands unchanged before a quick check of it starts.
pub const SETTLE: Duration = Duration::from_millis(1500);

/// The quick check: the plate it waits on, what it found and the pass under way.
#[derive(Default)]
pub(super) struct Quick {
    /// The plate last seen, and when it came to stand so.
    seen: Option<(u64, Instant)>,
    /// The plate a cut was started for, so a cut that fails is not started again.
    cut_for: Option<u64>,
    measured: Option<(MeasureKey, Measured)>,
    measuring: Option<(MeasureKey, MeasureJob)>,
}

impl Preview {
    /// Notes the plate standing as `fingerprint` at `now`. A change stops the quick check
    /// under way and a cut started for one, and lets a plate come back to be cut again.
    pub fn note_plate(&mut self, fingerprint: u64, now: Instant) {
        if self.quick.seen.is_some_and(|(seen, _)| seen == fingerprint) {
            return;
        }
        self.quick.seen = Some((fingerprint, now));
        self.quick.measuring = None;
        self.quick.cut_for = None;
        if self.job.as_ref().is_some_and(|build| build.quiet) {
            self.job = None;
        }
    }

    /// How long the plate noted last has still to stand before a quick check, zero once it
    /// has stood long enough.
    pub fn settles_in(&self, now: Instant) -> Option<Duration> {
        let (_, since) = self.quick.seen?;
        Some(SETTLE.saturating_sub(now.saturating_duration_since(since)))
    }

    /// Whether a cut for the quick check of the plate `fingerprint` may start: none has
    /// been tried for it since it was noted. Notes it as tried.
    pub fn try_cut_for(&mut self, fingerprint: u64) -> bool {
        let first = self.quick.cut_for != Some(fingerprint);
        self.quick.cut_for = Some(fingerprint);
        first
    }

    /// Starts a quick check of the stack as `settings` would write it, unless one is held
    /// or under way.
    pub fn measure_quick(&mut self, settings: &RasterSettings, fold: Fold) {
        let Some(key) = self.measure_key(settings, fold) else {
            return;
        };
        let known = |held: Option<MeasureKey>| held == Some(key);
        if known(self.quick.measured.as_ref().map(|(key, _)| *key))
            || known(self.quick.measuring.as_ref().map(|(key, _)| *key))
        {
            return;
        }
        let Some(stack) = self.source.as_ref() else {
            return;
        };
        let job = MeasureJob::spawn(
            Arc::clone(&stack.solid),
            stack.windows.clone(),
            as_seen(settings).binned(QUICK_CHECK_BIN),
            (fold.tolerance, fold.start()),
            (cores() / 2).max(1),
        );
        self.quick.measuring = Some((key, job));
    }

    /// What the quick check found, once it has read the plate as it now is for `settings`.
    pub fn quick_measured(&self, settings: &RasterSettings, fold: Fold) -> Option<&Measured> {
        let key = self.measure_key(settings, fold)?;
        let (measured_key, measured) = self.quick.measured.as_ref()?;
        (*measured_key == key).then_some(measured)
    }

    /// Drains the quick check. Returns whether it is still going.
    pub fn poll_quick(&mut self) -> bool {
        let Some((key, job)) = self.quick.measuring.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };
        let key = *key;
        self.quick.measuring = None;
        // A quick check that fails says nothing: the full one, once Check is open, says why.
        if let MeasureOutcome::Measured(measured) = outcome {
            self.quick.measured = Some((key, *measured));
        }
        false
    }
}

#[cfg(test)]
mod tests;
