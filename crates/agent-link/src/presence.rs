//! The file each server keeps in `agents/`: who drives it, what it runs and the jobs it touched
//! (ADR 0462).

use serde::{Deserialize, Serialize};

/// The schema of an agent's file.
pub const AGENT_SCHEMA: u32 = 1;

/// What an agent's file says. Times are Unix seconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentFile {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub pid: u32,
    /// The client's own name for itself, as it introduced itself: `code-agent`, say.
    #[serde(default)]
    pub client: Option<String>,
    pub started_s: u64,
    pub seen_s: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_s: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working: Option<Working>,
    /// The jobs it touched, the latest first.
    #[serde(default)]
    pub jobs: Vec<String>,
}

/// The tool running, on which job, since when (a Unix second).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Working {
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    pub since_s: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_without_schema_pid_or_jobs_reads_as_an_idle_agent() {
        let text = r#"{"client": "code-agent", "started_s": 1, "seen_s": 2}"#;
        let read: AgentFile = serde_json::from_str(text).expect("an older file reads");
        assert_eq!((read.schema, read.pid, read.jobs.len()), (0, 0, 0));
        assert!(read.working.is_none());
    }
}
