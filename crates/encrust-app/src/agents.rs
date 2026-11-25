//! The agents the window lists: each live `encrust-mcp` that keeps its file in `agents/` and
//! has a model in hand, read from `mcp-jobs/` (ADR 0465, 0476, `docs/design/window-link.md`).

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

/// What the window knows of its agents, and what the user asked of them this frame.
#[derive(Debug, Default)]
pub struct Agents {
    pub list: Vec<Agent>,
    read: Option<Receiver<Vec<Agent>>>,
    /// The revisions the user asked to see on the plate, for the link to take.
    pub asked: Vec<Asked>,
    /// The jobs the user took off the list, kept off it while their agent holds them.
    hidden: Vec<String>,
    /// The servers' job folders, where a deleted job is taken away.
    jobs: Option<PathBuf>,
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
    /// The job in hand: the one its running tool names, else the latest it touched.
    pub job: Option<Job>,
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

    /// The model's name: its file's, without the extension.
    pub fn name(&self) -> String {
        self.model.file_stem().map_or_else(
            || self.id.clone(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

impl Agent {
    /// Whether it starts a job that has no revision yet.
    pub fn opening(&self) -> bool {
        matches!(&self.state, State::Working { tool, job: None, .. } if tool == "start_job")
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
        let jobs = cache.join("mcp-jobs");
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
            jobs: Some(jobs),
            ..Self::default()
        }
    }

    /// Takes what the thread read last.
    pub fn poll(&mut self) {
        if let Some(read) = &self.read {
            while let Ok(list) = read.try_recv() {
                self.list = list;
                self.list.retain(|agent| !hidden(agent, &self.hidden));
            }
        }
    }

    /// Takes `job` off the list until its agent takes another in hand.
    pub fn hide(&mut self, job: &str) {
        self.hidden.push(job.to_owned());
        self.list.retain(|agent| !hidden(agent, &self.hidden));
    }

    /// Takes `job` off the list and its folder off the disk, on a thread of its own.
    pub fn delete(&mut self, job: &str) {
        self.hide(job);
        let Some(jobs) = self.jobs.clone() else {
            return;
        };
        let id = job.to_owned();
        let spawned = std::thread::Builder::new()
            .name("agents-delete".into())
            .spawn(move || {
                if let Err(error) = remove_job(&jobs, &id) {
                    tracing::warn!("job {id} is not deleted: {error}");
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("not deleting job {job}: {error}");
        }
    }

    /// Whether one of the agents runs a tool now.
    pub fn working(&self) -> bool {
        self.list
            .iter()
            .any(|a| matches!(a.state, State::Working { .. }))
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

/// The live agents in `agents` with a model in hand or opening one, those at work first,
/// each the latest seen first.
pub fn scan(agents: &Path, jobs: &Path, now_s: u64) -> Vec<Agent> {
    let mut read: Vec<(u64, Agent)> = std::fs::read_dir(agents)
        .into_iter()
        .flatten()
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| serde_json::from_slice::<Said>(&std::fs::read(path).ok()?).ok())
        .filter_map(|said| Some((said.seen_s, agent_of(&said, jobs, now_s)?)))
        .filter(|(_, agent)| agent.job.is_some() || agent.opening())
        .collect();
    let idle = |agent: &Agent| agent.state == State::Idle;
    read.sort_by(|(a_seen, a), (b_seen, b)| idle(a).cmp(&idle(b)).then(b_seen.cmp(a_seen)));
    read.into_iter().map(|(_, agent)| agent).collect()
}

/// The agent a file tells of, `None` once it has ended or fallen silent.
fn agent_of(said: &Said, jobs: &Path, now_s: u64) -> Option<Agent> {
    let silent = now_s.saturating_sub(said.seen_s) > SILENT_S;
    if said.ended_s.is_some() || silent {
        return None;
    }
    let state = match &said.working {
        Some(working) => State::Working {
            tool: working.tool.clone(),
            job: working.job.clone(),
            since_s: working.since_s,
        },
        None => State::Idle,
    };
    let running = said
        .working
        .as_ref()
        .and_then(|working| working.job.as_ref());
    Some(Agent {
        client: said.client.clone(),
        started_s: said.started_s,
        state,
        job: running
            .into_iter()
            .chain(said.jobs.first())
            .find_map(|id| job(jobs, id)),
    })
}

/// Whether the user took the job `agent` holds off the list.
fn hidden(agent: &Agent, hidden: &[String]) -> bool {
    agent
        .job
        .as_ref()
        .is_some_and(|job| hidden.contains(&job.id))
}

/// Whether `id` names a job folder and nothing outside `mcp-jobs/`.
fn plain(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Takes the folder of job `id` under `jobs` away with every revision in it.
pub fn remove_job(jobs: &Path, id: &str) -> std::io::Result<()> {
    if !plain(id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{id:?} is not a job's name"),
        ));
    }
    std::fs::remove_dir_all(jobs.join(id))
}

/// A job's latest revision, or `None` where its folder holds none.
fn job(jobs: &Path, id: &str) -> Option<Job> {
    if !plain(id) {
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
