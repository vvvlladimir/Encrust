//! The line `encrust-mcp` sends for each revision it keeps, and the window's answer
//! (`docs/design/window-link.md`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::project::{Project, load};
use serde::{Deserialize, Serialize};

/// The schema of a revision handed over as its plate (ADR 0473).
pub const SCHEMA: u64 = 4;

/// One revision of a chat client's job: its model, its machine and the plate its check wrote.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    pub job: String,
    pub revision: u32,
    pub model: PathBuf,
    /// A printer id of the catalogue, or the path of a printer TOML.
    pub printer: String,
    /// A resin id, or the path of a resin TOML.
    pub resin: String,
    /// The `.encrust` its check wrote, which the window puts on its plate.
    pub plate: PathBuf,
    /// Shown whether the window follows its agents or not: the client asked for it, or the
    /// user did (ADR 0465).
    #[serde(default)]
    pub open: bool,
}

/// A revision with its plate read, ready to stand on the window's.
#[derive(Debug)]
pub struct Shown {
    pub request: Request,
    pub project: Project,
}

impl Request {
    /// Revision `revision` of `job` as the server keeps it under `jobs`, to be shown: the
    /// model and the machine from the flags it stages with, and the plate its check wrote.
    pub fn of_job(jobs: &Path, job: &str, revision: u32) -> Result<Self> {
        let dir = jobs.join(job);
        if dir.join(format!("{revision}.edits.json")).is_file() {
            bail!("revision {revision} of {job} was made on this plate, which holds it already");
        }
        let path = dir.join(format!("{revision}.args.json"));
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let flags: Vec<String> = serde_json::from_str(&text)
            .with_context(|| format!("revision {revision} of {job} does not read"))?;
        let flag = |names: [&str; 2]| {
            let value = flags.iter().find_map(|flag| {
                let (name, value) = flag.split_once('=')?;
                names.contains(&name).then_some(value)
            });
            value.unwrap_or_default().to_owned()
        };
        Ok(Self {
            job: job.to_owned(),
            revision,
            model: flags
                .first()
                .with_context(|| format!("revision {revision} of {job} names no model"))?
                .into(),
            printer: flag(["--printer", "--profile"]),
            resin: flag(["--resin", "--material"]),
            plate: dir.join(format!("{revision}.encrust")),
            open: true,
        })
    }

    /// The revision with its plate read.
    pub fn read(self) -> Result<Shown> {
        let project = load(&self.plate)
            .with_context(|| format!("the revision's plate {}", self.plate.display()))?;
        Ok(Shown {
            request: self,
            project,
        })
    }
}

/// What the window says of a revision.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reply {
    pub shown: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why_not: Option<String>,
}

impl Reply {
    pub fn taken() -> Self {
        Self {
            shown: true,
            why_not: None,
        }
    }

    pub fn refused(why: String) -> Self {
        Self {
            shown: false,
            why_not: Some(why),
        }
    }
}
