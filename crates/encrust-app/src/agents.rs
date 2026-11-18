//! The agents the window lists: each `encrust-mcp` that keeps its file in `agents/`, with its
//! latest jobs read from `mcp-jobs/` (ADR 0465, `docs/design/window-link.md`).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

#[cfg(test)]
mod tests;

/// How often the folders are read again.
const EVERY: Duration = Duration::from_secs(2);
/// How long an agent may say nothing before it is taken as ended: three of its beats.
const SILENT_S: u64 = 60;
/// The most jobs read of one agent.
pub const MOST_JOBS: usize = 3;

/// What the window knows of its agents, and what the user asked of them this frame.
#[derive(Debug, Default)]
pub struct Agents {
    pub list: Vec<Agent>,
    read: Option<Receiver<Vec<Agent>>>,
    /// The revisions the user asked to see on the plate, for the link to take.
    pub asked: Vec<Asked>,
    /// The jobs whose model the link keeps on the plate.
    pub on_plate: Vec<String>,
}

/// A revision of a job, to be shown on the plate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub job: String,
    pub revision: u32,
}

/// One server, by the client that drives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    /// The client's own name for itself: `claude-code`, say.
    pub client: Option<String>,
    pub started_s: u64,
    pub state: State,
    /// Its latest jobs, at most `MOST_JOBS`, latest first.
    pub jobs: Vec<Job>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// A tool runs, on the job it names.
    Working {
        tool: String,
        job: Option<String>,
        /// When it started, a Unix second.
        since_s: u64,
    },
    Idle,
    Ended,
}

/// A job's latest revision, as its folder holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: String,
    pub model: PathBuf,
    pub revision: u32,
    /// Whether its check found nothing; `None` where no check was kept.
    pub clean: Option<bool>,
}

impl Job {
    /// Whether the job is of a project the window saved, which opens from File.
    pub fn is_project(&self) -> bool {
        crate::files::has_extension(&self.model, &crate::files::PROJECTS)
    }

    /// The model's file name.
    pub fn name(&self) -> String {
        self.model.file_name().map_or_else(
            || self.id.clone(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

impl Agents {
    /// Reads the folders under the user's cache every `EVERY` on a thread of its own, a frame
    /// asked of `ctx` whenever what it read changed.
    pub fn start(ctx: &egui::Context) -> Self {
        let Some(cache) = directories::ProjectDirs::from("", "", "Encrust")
            .map(|dirs| dirs.cache_dir().to_owned())
        else {
            return Self::default();
        };
        let (send, read) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("agents".into())
            .spawn(move || {
                let mut last = None;
                loop {
                    let now = now_s();
                    let listed = scan(&cache.join("agents"), &cache.join("mcp-jobs"), now);
                    if last.as_ref() != Some(&listed) {
                        if send.send(listed.clone()).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                        last = Some(listed);
                    }
                    std::thread::sleep(EVERY);
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("not reading the agents: {error}");
        }
        Self {
            read: Some(read),
            ..Self::default()
        }
    }

    /// Takes what the thread read last.
    pub fn poll(&mut self) {
        if let Some(read) = &self.read {
            while let Ok(list) = read.try_recv() {
                self.list = list;
            }
        }
    }

    /// How many agents are there now, and whether one of them works.
    pub fn at_work(&self) -> (usize, bool) {
        let live = self.list.iter().filter(|a| a.state != State::Ended);
        let working = self
            .list
            .iter()
            .any(|a| matches!(a.state, State::Working { .. }));
        (live.count(), working)
    }
}

#[derive(Deserialize)]
struct Said {
    client: Option<String>,
    started_s: u64,
    seen_s: u64,
    ended_s: Option<u64>,
    working: Option<Working>,
    #[serde(default)]
    jobs: Vec<String>,
}

#[derive(Deserialize)]
struct Working {
    tool: String,
    job: Option<String>,
    since_s: u64,
}

/// The agents whose files are in `agents`, those at work first, then the idle, then the
/// ended, each the latest seen first; an ended one with no job left is not listed.
pub fn scan(agents: &Path, jobs: &Path, now_s: u64) -> Vec<Agent> {
    let Ok(entries) = std::fs::read_dir(agents) else {
        return Vec::new();
    };
    let mut read: Vec<(u64, Agent)> = entries
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| serde_json::from_slice::<Said>(&std::fs::read(path).ok()?).ok())
        .filter_map(|said| {
            let agent = agent_of(&said, jobs, now_s);
            let listed = agent.state != State::Ended || !agent.jobs.is_empty();
            listed.then_some((said.seen_s, agent))
        })
        .collect();
    let rank = |state: &State| match state {
        State::Working { .. } => 0,
        State::Idle => 1,
        State::Ended => 2,
    };
    read.sort_by(|(a_seen, a), (b_seen, b)| {
        rank(&a.state).cmp(&rank(&b.state)).then(b_seen.cmp(a_seen))
    });
    read.into_iter().map(|(_, agent)| agent).collect()
}

fn agent_of(said: &Said, jobs: &Path, now_s: u64) -> Agent {
    let silent = now_s.saturating_sub(said.seen_s) > SILENT_S;
    let state = match (&said.working, said.ended_s.is_some() || silent) {
        (_, true) => State::Ended,
        (Some(working), false) => State::Working {
            tool: working.tool.clone(),
            job: working.job.clone(),
            since_s: working.since_s,
        },
        (None, false) => State::Idle,
    };
    Agent {
        client: said.client.clone(),
        started_s: said.started_s,
        state,
        jobs: said
            .jobs
            .iter()
            .filter_map(|id| job(jobs, id))
            .take(MOST_JOBS)
            .collect(),
    }
}

/// A job's latest revision, or `None` where its folder holds none.
fn job(jobs: &Path, id: &str) -> Option<Job> {
    let plain = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !plain {
        return None;
    }
    let dir = jobs.join(id);
    let revision = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "toml").then_some(())?;
            path.file_stem()?.to_str()?.parse::<u32>().ok()
        })
        .max()?;
    // The flags the revision stages with, the model first.
    let flags: Vec<PathBuf> =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{revision}.args.json"))).ok()?)
            .ok()?;
    let clean = std::fs::read(dir.join(format!("{revision}.json")))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|report| report["status"].as_str().map(|status| status == "ok"));
    Some(Job {
        id: id.to_owned(),
        model: flags.into_iter().next()?,
        revision,
        clean,
    })
}

/// Seconds since the Unix epoch.
pub fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}
