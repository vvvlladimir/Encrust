//! A revision of a job to show, as `encrust-mcp` hands it over or the job's folder holds it,
//! and its plate read (`docs/design/window-link.md`).

use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use agent_link::{RevisionFile, Show, is_job_name};
use anyhow::{Context, Result, anyhow, bail};

use crate::i18n::tr;
use core_engine::project::{Project, load};

/// A revision with its plate read, ready to stand on the window's.
#[derive(Debug)]
pub struct Shown {
    pub request: Show,
    pub project: Project,
}

/// Revision `revision` of `job` under `jobs`, shown from its flags and the plate its check wrote;
/// one made by edits is refused while the job's plate, which holds it, stands (`held`).
pub fn of_job(jobs: &Path, job: &str, revision: u32, held: bool) -> Result<Show> {
    if !is_job_name(job) {
        bail!(tr!("link-no-job", job = job.to_owned()));
    }
    let dir = jobs.join(job);
    if held && dir.join(RevisionFile::Edits.of(revision)).is_file() {
        bail!(tr!(
            "link-revision-on-plate",
            job = job.to_owned(),
            revision = revision
        ));
    }
    let path = dir.join(RevisionFile::Args.of(revision));
    let text = std::fs::read_to_string(&path)
        .with_context(|| tr!("error-cannot-read", path = path.display().to_string()))?;
    let unreadable = || {
        tr!(
            "link-revision-unreadable",
            job = job.to_owned(),
            revision = revision
        )
    };
    let flags: Vec<String> = serde_json::from_str(&text).with_context(unreadable)?;
    let flag = |names: [&str; 2]| {
        let value = flags.iter().find_map(|flag| {
            let (name, value) = flag.split_once('=')?;
            names.contains(&name).then_some(value)
        });
        value.unwrap_or_default().to_owned()
    };
    Ok(Show {
        job: job.to_owned(),
        revision,
        model: flags.first().with_context(unreadable)?.into(),
        printer: flag(["--printer", "--profile"]),
        resin: flag(["--resin", "--material"]),
        plate: dir.join(RevisionFile::Plate.of(revision)),
        open: true,
    })
}

/// `request` with its plate read.
pub fn read(request: Show) -> Result<Shown> {
    let project = load(&request.plate).with_context(|| {
        tr!(
            "error-cannot-read",
            path = request.plate.display().to_string()
        )
    })?;
    Ok(Shown { request, project })
}

/// A revision to show: its plate being read off the drawing thread, then read (ADR 0462).
#[derive(Debug)]
pub enum Waiting {
    Reading(Receiver<Result<Shown>>),
    Read(Box<Shown>),
}

impl Waiting {
    /// Starts reading `request`'s plate on a thread of its own.
    pub fn read_aside(request: Show) -> Self {
        let (send, reading) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("link-plate".into())
            .spawn(move || drop(send.send(read(request))));
        if let Err(error) = spawned {
            let (send, failed) = mpsc::channel();
            let _ = send.send(Err(anyhow!(error).context(tr!("link-plate-unread"))));
            return Self::Reading(failed);
        }
        Self::Reading(reading)
    }

    /// Whether the plate is read, `false` while it is still being read; why where it did not.
    pub fn settle(&mut self) -> Result<bool> {
        let Self::Reading(reading) = self else {
            return Ok(true);
        };
        match reading.try_recv() {
            Ok(read) => *self = Self::Read(Box::new(read?)),
            Err(TryRecvError::Empty) => return Ok(false),
            Err(TryRecvError::Disconnected) => bail!(tr!("link-plate-unread")),
        }
        Ok(true)
    }
}
