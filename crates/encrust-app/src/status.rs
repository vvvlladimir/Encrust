use crate::i18n::tr;
/// The last thing that happened, shown over the stage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum Status {
    #[default]
    Idle,
    Info(String),
    /// What happened, and what it reads now, each against what it read before.
    Readings(String, Vec<Reading>),
    /// What happened, and under it each further fact about it, a sentence apiece.
    Noted(String, Vec<String>),
    Error(String),
}

/// One figure as the notice shows it, and which way it moved.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reading {
    pub text: String,
    pub change: Change,
}

impl Reading {
    /// `now` as `shown` writes it, against `was` where there was one before. Lower is
    /// better; a change too small to show is none.
    pub fn lower_better(now: f64, was: Option<f64>, shown: impl Fn(f64) -> String) -> Self {
        let text = shown(now);
        let change = match was {
            Some(was) if shown(was) != text && now < was => Change::Better,
            Some(was) if shown(was) != text => Change::Worse,
            _ => Change::Same,
        };
        Self { text, change }
    }
}

/// Which way a reading moved from the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Change {
    Same,
    Better,
    Worse,
}

impl Status {
    /// Records the outcome of an action the user asked for: the value on success, the
    /// whole error chain on failure, so the cause is visible without opening a terminal.
    pub fn report<T>(&mut self, action: &str, result: anyhow::Result<T>) -> Option<T> {
        match result {
            Ok(value) => {
                *self = Self::Info(action.to_owned());
                Some(value)
            }
            Err(error) => {
                *self = Self::failed(&error);
                None
            }
        }
    }

    /// The whole cause chain of a failure on one line, which is all the stage notice has
    /// room for.
    pub fn failed(error: &anyhow::Error) -> Self {
        let causes: Vec<String> = error.chain().map(ToString::to_string).collect();
        Self::Error(causes.join(": "))
    }

    /// What happened, with `notes` under it where there are any.
    pub fn noted(message: String, notes: Vec<String>) -> Self {
        if notes.is_empty() {
            Self::Info(message)
        } else {
            Self::Noted(message, notes)
        }
    }

    pub fn text(&self) -> String {
        match self {
            Self::Idle => tr!("card-ready"),
            Self::Info(message)
            | Self::Readings(message, _)
            | Self::Noted(message, _)
            | Self::Error(message) => message.clone(),
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context as _, anyhow};

    #[test]
    fn a_success_keeps_the_value_and_names_the_action() {
        let mut status = Status::default();
        assert_eq!(status.report("Opened cube.stl", Ok(7)), Some(7));
        assert_eq!(status.text(), "Opened cube.stl");
        assert!(!status.is_error());
    }

    #[test]
    fn a_failure_reports_every_cause() {
        let mut status = Status::default();
        let failed: anyhow::Result<()> =
            Err(anyhow!("no such file")).context("cannot load cube.stl");
        assert_eq!(status.report("Opened cube.stl", failed), None);
        assert!(status.is_error());
        assert_eq!(status.text(), "cannot load cube.stl: no such file");
    }

    #[test]
    fn a_reading_moves_only_as_far_as_it_shows() {
        let ml = |value: f64| format!("{value:.2} ml");
        assert_eq!(
            Reading::lower_better(1.0, Some(2.0), ml).change,
            Change::Better
        );
        assert_eq!(
            Reading::lower_better(2.0, Some(1.0), ml).change,
            Change::Worse
        );
        assert_eq!(
            Reading::lower_better(1.0, Some(1.001), ml).change,
            Change::Same,
            "both read 1.00 ml"
        );
        assert_eq!(Reading::lower_better(1.0, None, ml).change, Change::Same);
    }
}
