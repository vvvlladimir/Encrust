//! Jobs on disk: a directory per job, a numbered TOML recipe per revision beside the report
//! the command line gave for it. A call names its job, so no state lives in the process.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::recipe::Recipe;

/// Where the jobs are kept.
#[derive(Debug, Clone)]
pub struct Jobs {
    root: PathBuf,
}

/// One revision of a job: its recipe and what checking it found.
#[derive(Debug, Clone)]
pub struct Revision {
    pub job: String,
    /// Counted from one.
    pub number: u32,
    pub recipe: Recipe,
}

impl Jobs {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Starts a job at revision one.
    pub fn start(&self, recipe: Recipe) -> Result<Revision> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or_default();
        let stem = recipe
            .model
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map_or_else(|| "model".into(), slug);
        let job = format!("{stem}-{stamp:x}");
        fs::create_dir_all(self.dir(&job)).with_context(|| format!("creating job {job}"))?;
        self.write(job, 1, recipe)
    }

    /// The revision a call names, or the job's latest one.
    pub fn open(&self, job: &str, number: Option<u32>) -> Result<Revision> {
        let number = match number {
            Some(number) => number,
            None => self.latest(job)?,
        };
        let path = self.recipe_path(job, number);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("job {job} has no revision {number}"))?;
        let recipe =
            toml::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
        Ok(Revision {
            job: job.to_owned(),
            number,
            recipe,
        })
    }

    /// Writes `recipe` as the revision after the job's latest.
    pub fn next(&self, job: &str, recipe: Recipe) -> Result<Revision> {
        let number = self.latest(job)? + 1;
        self.write(job.to_owned(), number, recipe)
    }

    /// Keeps what checking a revision reported, for the next call to compare against.
    pub fn keep_report(&self, revision: &Revision, report: &Value) -> Result<()> {
        let path = self
            .dir(&revision.job)
            .join(format!("{}.json", revision.number));
        let text = serde_json::to_string_pretty(report)?;
        fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
    }

    fn latest(&self, job: &str) -> Result<u32> {
        let dir = self.dir(job);
        if !is_job_name(job) || !dir.is_dir() {
            bail!("there is no job {job}; start one with start_job");
        }
        let entries = fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?;
        entries
            .filter_map(|entry| revision_number(&entry.ok()?.path()))
            .max()
            .with_context(|| format!("job {job} has no revision"))
    }

    fn write(&self, job: String, number: u32, recipe: Recipe) -> Result<Revision> {
        let path = self.recipe_path(&job, number);
        let text = toml::to_string(&recipe)?;
        fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(Revision {
            job,
            number,
            recipe,
        })
    }

    fn dir(&self, job: &str) -> PathBuf {
        self.root.join(job)
    }

    fn recipe_path(&self, job: &str, number: u32) -> PathBuf {
        self.dir(job).join(format!("{number}.toml"))
    }
}

/// A job's name is made here, so one that holds anything else reaches outside the root.
fn is_job_name(job: &str) -> bool {
    !job.is_empty()
        && job
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
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
