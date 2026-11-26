//! A revision of a job to show, as `encrust-mcp` hands it over or the job's folder holds it,
//! and its plate read (`docs/design/window-link.md`).

use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use agent_link::Show;
use anyhow::{Context, Result, anyhow, bail};
use core_engine::project::{Project, load};

/// A revision with its plate read, ready to stand on the window's.
#[derive(Debug)]
pub struct Shown {
    pub request: Show,
    pub project: Project,
}

/// Revision `revision` of `job` as the server keeps it under `jobs`, to be shown: the model and
/// the machine from the flags it stages with, and the plate its check wrote.
pub fn of_job(jobs: &Path, job: &str, revision: u32) -> Result<Show> {
    let dir = jobs.join(job);
    if dir.join(format!("{revision}.edits.json")).is_file() {
        bail!("revision {revision} of {job} was made on this plate, which holds it already");
    }
    let path = dir.join(format!("{revision}.args.json"));
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let flags: Vec<String> = serde_json::from_str(&text)
        .with_context(|| format!("revision {revision} of {job} does not read"))?;
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

/// `request` with its plate read.
pub fn read(request: Show) -> Result<Shown> {
    let project = load(&request.plate)
        .with_context(|| format!("the revision's plate {}", request.plate.display()))?;
    Ok(Shown { request, project })
}

/// A revision to show: its plate being read off the drawing thread, then read (ADR 0483).
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
            let _ = send.send(Err(anyhow!("the plate is not read: {error}")));
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
            Err(TryRecvError::Disconnected) => bail!("the plate's reading stopped"),
        }
        Ok(true)
    }
}
