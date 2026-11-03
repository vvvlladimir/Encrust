//! Jobs on disk: a directory per job, a numbered TOML recipe per revision and the JSON report
//! of its check beside it. A call names its job, so no state lives in the process but the
//! lock that orders edits to one job.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::recipe::Recipe;

/// Where the jobs are kept, and a lock per job so two edits never take one number.
#[derive(Debug, Clone)]
pub struct Jobs {
    root: PathBuf,
    locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

/// One revision of a job.
#[derive(Debug, Clone)]
pub struct Revision {
    pub job: String,
    /// Counted from one.
    pub number: u32,
    pub recipe: Recipe,
}

impl Jobs {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            locks: Arc::default(),
        }
    }

    /// A new job's name, from its model's file name and the time; nothing is written yet.
    pub fn name_for(recipe: &Recipe) -> String {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or_default();
        let stem = recipe
            .model
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map_or_else(|| "model".into(), slug);
        format!("{stem}-{stamp:x}")
    }

    /// The lock an edit of `job` holds from reading its latest revision to writing the next.
    pub fn lock(&self, job: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(locks.entry(job.to_owned()).or_default())
    }

    /// The revision a call names, or the job's latest one.
    pub fn open(&self, job: &str, number: Option<u32>) -> Result<Revision> {
        let number = match number {
            Some(number) => number,
            None => self.latest(job)?,
        };
        let path = self.dir(job)?.join(format!("{number}.toml"));
        let text = fs::read_to_string(&path)
            .with_context(|| format!("job {job} has no revision {number}"))?;
        let recipe = toml::from_str(&text)
            .with_context(|| format!("revision {number} of job {job} does not read"))?;
        Ok(Revision {
            job: job.to_owned(),
            number,
            recipe,
        })
    }

    /// Writes `recipe` and the `report` of its check as the revision after the job's latest, or
    /// as revision one of a job not there yet. Hold the job's lock across reading the latest
    /// and this.
    pub fn commit(&self, job: &str, recipe: Recipe, report: &Value) -> Result<Revision> {
        let dir = self.dir(job)?;
        let number = if dir.is_dir() {
            self.latest(job)? + 1
        } else {
            1
        };
        fs::create_dir_all(&dir).with_context(|| format!("creating job {job}"))?;
        let path = dir.join(format!("{number}.toml"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("revision {number} of job {job} was taken"))?;
        file.write_all(toml::to_string(&recipe)?.as_bytes())
            .with_context(|| format!("writing {}", path.display()))?;
        let checked = dir.join(format!("{number}.json"));
        fs::write(&checked, report.to_string())
            .with_context(|| format!("writing {}", checked.display()))?;
        Ok(Revision {
            job: job.to_owned(),
            number,
            recipe,
        })
    }

    /// An empty directory of the job's for the pictures of one revision.
    pub fn pictures_dir(&self, revision: &Revision) -> Result<PathBuf> {
        let dir = self
            .dir(&revision.job)?
            .join(format!("pictures-{}", revision.number));
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("clearing {}", dir.display()))?;
        }
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(dir)
    }

    fn latest(&self, job: &str) -> Result<u32> {
        let dir = self.dir(job)?;
        if !dir.is_dir() {
            bail!("there is no job {job}; start one with start_job");
        }
        let entries = fs::read_dir(&dir).with_context(|| format!("reading job {job}"))?;
        entries
            .filter_map(|entry| revision_number(&entry.ok()?.path()))
            .max()
            .with_context(|| format!("job {job} has no revision"))
    }

    /// A job's directory under the root. A name of anything but letters, digits, `-` and `_`
    /// is refused, so no name reaches outside it.
    fn dir(&self, job: &str) -> Result<PathBuf> {
        let plain = !job.is_empty()
            && job
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !plain {
            bail!("{job:?} is not a job name; use the id start_job returned");
        }
        Ok(self.root.join(job))
    }
}

fn slug(stem: &str) -> String {
    let slug: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(32)
        .collect();
    if slug.trim_matches('-').is_empty() {
        "model".into()
    } else {
        slug
    }
}

fn revision_number(path: &Path) -> Option<u32> {
    if path.extension()? != "toml" {
        return None;
    }
    path.file_stem()?.to_str()?.parse().ok()
}

#[cfg(test)]
mod tests;
