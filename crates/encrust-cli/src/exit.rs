//! How a run ends: the exit codes `docs/cli.md` promises, and the Ctrl-C that stops one.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

/// How a run that did not fail ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Success,
    /// `--strict`, and a model with defects or one that does not fit.
    Unclean,
    /// A batch in which at least one model failed.
    PartlyFailed,
}

impl Exit {
    pub fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Unclean => 3,
            Self::PartlyFailed => 4,
        }
    }
}

/// The exit code a run's result maps to: an error is 1, and a run the user stopped is 130,
/// as a shell reports a process Ctrl-C killed.
pub fn exit_code(result: &Result<Exit>) -> u8 {
    match result {
        Ok(exit) => exit.code(),
        Err(error) if is_cancelled(error) => 130,
        Err(_) => 1,
    }
}

pub fn is_cancelled(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| cause.is::<Cancelled>())
}

/// The error a run returns once the user stopped it.
#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// Whether the user has asked the run to stop, shared between the Ctrl-C handler and the
/// run it interrupts.
#[derive(Debug, Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    /// Asks the run to stop, and says whether it had been asked already.
    pub fn request(&self) -> bool {
        self.0.swap(true, Ordering::Relaxed)
    }

    pub fn requested(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Fails with [`Cancelled`] once a stop has been asked for.
    pub fn check(&self) -> Result<()> {
        if self.requested() {
            return Err(Cancelled.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use super::*;

    #[test]
    fn a_cancelled_run_exits_130_however_deep_the_context_wraps_it() {
        let stop = Stop::default();
        stop.request();
        let wrapped = stop
            .check()
            .context("writing the stack")
            .context("model.stl");
        assert_eq!(exit_code(&wrapped.map(|()| Exit::Success)), 130);
    }

    #[test]
    fn any_other_error_exits_1() {
        assert_eq!(exit_code(&Err(anyhow::anyhow!("no such file"))), 1);
    }

    #[test]
    fn a_second_request_is_told_the_first_came_before_it() {
        let stop = Stop::default();
        assert!(!stop.request());
        assert!(stop.request(), "the handler hard-exits on this one");
    }
}
