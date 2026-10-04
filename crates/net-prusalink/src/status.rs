use serde::Deserialize;

use crate::error::PrusaLinkError;
use crate::link::Link;
use crate::session::Session;

/// What `GET /api/v1/status` says: the machine's state, and the job while there is one.
#[derive(Debug, Clone, Deserialize)]
pub struct Status {
    pub printer: Machine,
    #[serde(default)]
    pub job: Option<Job>,
}

/// The machine half of the status.
#[derive(Debug, Clone, Deserialize)]
pub struct Machine {
    /// As the API spells it: `IDLE`, `BUSY`, `PRINTING`, `PAUSED`, `FINISHED`, `STOPPED`,
    /// `ERROR`, `ATTENTION` or `READY`.
    pub state: String,
}

/// The print in progress. The status names no file; that is `GET /api/v1/job`'s.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Job {
    /// Percent, 0 to 100.
    #[serde(default)]
    pub progress: f32,
    #[serde(default)]
    pub time_remaining: Option<u64>,
}

/// Asks a machine what it is doing.
pub fn status(link: &Link) -> Result<Status, PrusaLinkError> {
    let body = Session::new(link).get("api/v1/status")?;
    Ok(serde_json::from_str(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_machine_reports_no_job() {
        let status: Status = serde_json::from_str(r#"{"printer":{"state":"IDLE"}}"#)
            .expect("the specification's smallest status");
        assert_eq!(status.printer.state, "IDLE");
        assert!(status.job.is_none());
    }

    #[test]
    fn a_printing_machine_reports_its_progress_in_percent() {
        let status: Status = serde_json::from_str(
            r#"{"printer":{"state":"PRINTING","axis_z":12.5},
                "job":{"id":420,"progress":42.0,"time_remaining":520,"time_printing":526}}"#,
        )
        .expect("the specification's example status");
        let job = status.job.expect("a print is running");
        assert!((job.progress - 42.0).abs() < f32::EPSILON);
        assert_eq!(job.time_remaining, Some(520));
    }

    #[test]
    fn a_status_without_a_state_is_malformed() {
        assert!(serde_json::from_str::<Status>(r#"{"printer":{}}"#).is_err());
    }
}
