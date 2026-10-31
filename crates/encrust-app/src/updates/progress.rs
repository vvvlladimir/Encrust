//! How far an install has got, shared by the thread doing it and the window drawing it.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

/// The three things an install does, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Download,
    Verify,
    Install,
}

/// Counters the install thread moves on and the window reads each frame.
#[derive(Debug, Default)]
pub struct Progress {
    received: AtomicU64,
    /// The archive's size as the server stated it, 0 while unknown.
    total: AtomicU64,
    step: AtomicU8,
    cancelled: AtomicBool,
}

impl Progress {
    pub fn step(&self) -> Step {
        match self.step.load(Ordering::Relaxed) {
            0 => Step::Download,
            1 => Step::Verify,
            _ => Step::Install,
        }
    }

    pub(crate) fn enter(&self, step: Step) {
        self.step.store(step as u8, Ordering::Relaxed);
    }

    /// The download's seconds left at the rate it has gone at for `elapsed_s`, once that
    /// rate and the size are known.
    pub fn seconds_left(&self, elapsed_s: f64) -> Option<u64> {
        let (received, total) = self.bytes();
        seconds_left(received, total, elapsed_s)
    }

    /// Bytes downloaded, and of how many when the server said.
    pub fn bytes(&self) -> (u64, Option<u64>) {
        let total = self.total.load(Ordering::Relaxed);
        (
            self.received.load(Ordering::Relaxed),
            (total > 0).then_some(total),
        )
    }

    pub(crate) fn expect(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }

    pub(crate) fn add(&self, bytes: u64) {
        self.received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Asks the download to stop at its next chunk; past it, the install runs to its end.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

fn seconds_left(received: u64, total: Option<u64>, elapsed_s: f64) -> Option<u64> {
    let total = total.filter(|_| received > 0 && elapsed_s > 0.0)?;
    let rate = received as f64 / elapsed_s;
    Some((total.saturating_sub(received) as f64 / rate).ceil() as u64)
}

#[cfg(test)]
mod tests {
    use super::seconds_left;

    #[test]
    fn the_time_left_goes_at_the_rate_so_far() {
        assert_eq!(
            seconds_left(10, Some(40), 2.0),
            Some(6),
            "30 bytes at 5 a second"
        );
    }

    #[test]
    fn the_time_left_is_unknown_before_a_rate_or_a_size() {
        assert_eq!(seconds_left(0, Some(40), 2.0), None);
        assert_eq!(seconds_left(10, None, 2.0), None);
        assert_eq!(seconds_left(10, Some(40), 0.0), None);
    }
}
