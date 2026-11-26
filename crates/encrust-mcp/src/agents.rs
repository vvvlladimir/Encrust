//! This server as one agent the open window lists: a file of its own in the agents' folder
//! saying which client drives it, what it is doing and the jobs it touched
//! (`docs/design/window-link.md`, ADR 0465).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent_link::{AGENT_SCHEMA, AgentFile, Working};
use serde_json::Value;

/// How long the file of an agent long gone is kept, for the window to list its jobs.
const KEPT: Duration = Duration::from_hours(24 * 7);
/// The most jobs one file names, the latest first.
const MOST_JOBS: usize = 20;
/// How often a running server says it is still there; the window reads one silent for
/// three of these as ended.
pub const BEAT: Duration = Duration::from_secs(20);

/// The agent's file, and what it says.
#[derive(Debug, Clone)]
pub struct Presence {
    file: Option<PathBuf>,
    said: Arc<Mutex<AgentFile>>,
}

impl Presence {
    /// This server's file in `dir`, written at once; the files of agents gone longer than a
    /// week are taken away. `None` keeps no file.
    pub fn new(dir: Option<PathBuf>) -> Self {
        let now = now_s();
        let pid = std::process::id();
        let file = dir.and_then(|dir| {
            prune(&dir, now);
            fs::create_dir_all(&dir).ok()?;
            Some(dir.join(format!("{pid}-{now}.json")))
        });
        let presence = Self {
            file,
            said: Arc::new(Mutex::new(AgentFile {
                schema: AGENT_SCHEMA,
                pid,
                client: None,
                started_s: now,
                seen_s: now,
                ended_s: None,
                working: None,
                jobs: Vec::new(),
            })),
        };
        presence.update(|_| ());
        presence
    }

    /// A tool call begins, from `client`, on `job` where it names one.
    pub fn begin(&self, client: Option<&str>, tool: &str, job: Option<&str>) {
        self.update(|seen| {
            if let Some(client) = client {
                seen.client = Some(client.to_owned());
            }
            seen.working = Some(Working {
                tool: tool.to_owned(),
                job: job.map(str::to_owned),
                since_s: seen.seen_s,
            });
        });
    }

    /// The call has been answered, about `job` where it was about one.
    pub fn end(&self, job: Option<&str>) {
        self.update(|seen| {
            seen.working = None;
            if let Some(job) = job {
                seen.jobs.retain(|kept| kept != job);
                seen.jobs.insert(0, job.to_owned());
                seen.jobs.truncate(MOST_JOBS);
            }
        });
    }

    /// The server is still there.
    pub fn beat(&self) {
        self.update(|_| ());
    }

    /// The client has gone; the file stays, for the window to list what was done.
    pub fn close(&self) {
        self.update(|seen| {
            seen.working = None;
            seen.ended_s = Some(seen.seen_s);
        });
    }

    /// Changes what the file says, stamps it now and writes it whole. A file that cannot be
    /// written leaves the window without this agent, never the client without an answer.
    fn update(&self, change: impl FnOnce(&mut AgentFile)) {
        let mut seen = self.said.lock().unwrap_or_else(PoisonError::into_inner);
        seen.seen_s = now_s();
        change(&mut seen);
        let Some(file) = &self.file else {
            return;
        };
        let written = serde_json::to_vec(&*seen).map(|bytes| {
            // Renamed into place, so the window never reads half a file.
            let partial = file.with_extension("json.part");
            fs::write(&partial, bytes).and_then(|()| fs::rename(&partial, file))
        });
        if let Ok(Err(error)) | Err(error) = written.map_err(std::io::Error::from) {
            eprintln!("encrust-mcp: {} is not written: {error}", file.display());
        }
    }
}

/// Takes away the files of agents last seen more than `KEPT` before `now`.
fn prune(dir: &Path, now: u64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for path in entries.filter_map(|entry| Some(entry.ok()?.path())) {
        let seen = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|seen| seen["seen_s"].as_u64());
        if seen.is_some_and(|seen| now.saturating_sub(seen) > KEPT.as_secs()) {
            let _ = fs::remove_file(&path);
        }
    }
}

fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("encrust-mcp-agents-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn read(dir: &Path) -> Value {
        let file = fs::read_dir(dir)
            .expect("the folder was made")
            .filter_map(|entry| Some(entry.ok()?.path()))
            .find(|path| path.extension().is_some_and(|ext| ext == "json"))
            .expect("one agent's file");
        serde_json::from_slice(&fs::read(file).expect("readable")).expect("JSON")
    }

    #[test]
    fn an_agent_says_what_it_runs_and_keeps_the_jobs_it_touched_latest_first() {
        let dir = scratch("working");
        let presence = Presence::new(Some(dir.clone()));
        presence.begin(Some("claude-code"), "set_hollow", Some("cube-1"));
        let working = read(&dir);
        assert_eq!(working["client"], "claude-code");
        assert_eq!(working["working"]["tool"], "set_hollow");
        assert_eq!(working["working"]["job"], "cube-1");
        presence.end(Some("cube-1"));
        presence.begin(None, "start_job", None);
        presence.end(Some("bust-2"));
        presence.end(Some("cube-1"));
        let idle = read(&dir);
        assert!(idle.get("working").is_none(), "{idle}");
        assert_eq!(idle["jobs"], serde_json::json!(["cube-1", "bust-2"]));
        assert_eq!(idle["client"], "claude-code", "kept from the first call");
        presence.close();
        assert!(read(&dir)["ended_s"].is_u64());
    }

    #[test]
    fn the_file_of_an_agent_gone_a_week_is_taken_away() {
        let dir = scratch("prune");
        fs::create_dir_all(&dir).expect("writable");
        let old = dir.join("1-1.json");
        fs::write(&old, r#"{"seen_s": 1}"#).expect("writable");
        let recent = dir.join("2-2.json");
        fs::write(&recent, format!(r#"{{"seen_s": {}}}"#, now_s())).expect("writable");
        Presence::new(Some(dir.clone()));
        assert!(!old.exists());
        assert!(recent.exists());
    }
}
