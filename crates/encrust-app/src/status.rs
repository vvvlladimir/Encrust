/// The last thing that happened, shown in the window's status bar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    Info(String),
    Error(String),
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

    /// The whole cause chain of a failure on one line, which is all the status bar has
    /// room for.
    pub fn failed(error: &anyhow::Error) -> Self {
        let causes: Vec<String> = error.chain().map(ToString::to_string).collect();
        Self::Error(causes.join(": "))
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Idle => "Ready",
            Self::Info(message) | Self::Error(message) => message,
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
}
