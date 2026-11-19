//! Jobs on disk, a directory per job and a numbered recipe per revision with its check, its
//! flags and the steps it runs beside it; a call names its job, so no state lives in the
//! process but a lock per job.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use core_edit::{Edit, ObjectId};
use serde_json::{Value, json};

use crate::recipe::Recipe;

/// The file in a job's directory that keeps the autopilot's check.
const AUTOPILOT: &str = "autopilot.json";
/// The file that keeps the number of the revision last written to a printer file.
const WRITTEN: &str = "written";
/// The file each print reported of the job is a line of.
const PRINTS: &str = "prints.jsonl";

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
    /// The `core-recipe` file its check ran, the flags folded in (ADR 0459); what the window
    /// runs.
    pub steps: Option<String>,
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
        let steps = fs::read_to_string(self.dir(job)?.join(steps_name(number))).ok();
        Ok(Revision {
            job: job.to_owned(),
            number,
            recipe,
            steps,
        })
    }

    /// Writes `recipe`, the `report` of its check and the `steps` it ran as the revision after
    /// the job's latest, or as revision one of a job not there yet. Hold the job's lock across
    /// reading the latest and this.
    pub fn commit(
        &self,
        job: &str,
        (recipe, steps): (Recipe, Option<String>),
        report: &Value,
    ) -> Result<Revision> {
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
        // The flags beside it, so a bench draws the revision without reading a recipe.
        let args = dir.join(format!("{number}.args.json"));
        fs::write(&args, serde_json::to_string(&recipe.flags())?)
            .with_context(|| format!("writing {}", args.display()))?;
        if let Some(steps) = &steps {
            let file = dir.join(steps_name(number));
            fs::write(&file, steps).with_context(|| format!("writing {}", file.display()))?;
        }
        Ok(Revision {
            job: job.to_owned(),
            number,
            recipe,
            steps,
        })
    }

    /// The file the check of revision `number` is kept in, if it is there.
    pub fn report_file(&self, job: &str, number: u32) -> Option<PathBuf> {
        let file = self.dir(job).ok()?.join(format!("{number}.json"));
        file.is_file().then_some(file)
    }

    /// The check of a revision as it was kept.
    pub fn report(&self, revision: &Revision) -> Result<Value> {
        let file = self
            .report_file(&revision.job, revision.number)
            .with_context(|| format!("revision {} keeps no check", revision.number))?;
        let text =
            fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        serde_json::from_str(&text).with_context(|| format!("{} does not read", file.display()))
    }

    /// Notes revision `number` as the one last written to a printer file.
    pub fn mark_written(&self, job: &str, number: u32) -> Result<()> {
        let file = self.dir(job)?.join(WRITTEN);
        fs::write(&file, number.to_string()).with_context(|| format!("writing {}", file.display()))
    }

    /// The revision last written to a printer file, if one was.
    pub fn written(&self, job: &str) -> Option<u32> {
        let text = fs::read_to_string(self.dir(job).ok()?.join(WRITTEN)).ok()?;
        text.trim().parse().ok()
    }

    /// Adds what a print of the job did to the ones kept; answers how many there are now.
    pub fn record_print(&self, job: &str, print: &Value) -> Result<usize> {
        let file = self.dir(job)?.join(PRINTS);
        let mut prints = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .with_context(|| format!("opening {}", file.display()))?;
        writeln!(prints, "{print}").with_context(|| format!("writing {}", file.display()))?;
        let kept =
            fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        Ok(kept.lines().count())
    }

    /// The file the autopilot's own check of the job's model is kept in, if it is there.
    pub fn autopilot_file(&self, job: &str) -> Option<PathBuf> {
        let file = self.dir(job).ok()?.join(AUTOPILOT);
        file.is_file().then_some(file)
    }

    /// Keeps `report` as the autopilot's check of the job's model, what every revision is
    /// set against.
    pub fn keep_autopilot(&self, job: &str, report: &Value) -> Result<()> {
        let file = self.dir(job)?.join(AUTOPILOT);
        fs::write(&file, report.to_string()).with_context(|| format!("writing {}", file.display()))
    }

    /// The `--against` flags that set a check of `job` beside the revision `previous` and
    /// the autopilot's, where each was kept.
    pub fn against(&self, job: &str, previous: Option<u32>) -> Vec<String> {
        let previous = previous.and_then(|number| self.report_file(job, number));
        let kept = [
            ("previous", previous),
            ("autopilot", self.autopilot_file(job)),
        ];
        kept.into_iter()
            .filter_map(|(name, file)| Some(format!("--against={name}={}", file?.display())))
            .collect()
    }

    /// A file no other run writes, for the command line to write the steps of a check into.
    pub fn scratch_steps(&self) -> Result<PathBuf> {
        static TAKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = self.root.join("scratch");
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let taken = TAKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(dir.join(format!("{}-{taken}.recipe.toml", std::process::id())))
    }

    /// A scratch path for a plate one check writes, kept as a revision's once it is kept.
    pub fn scratch_plate(&self) -> Result<PathBuf> {
        Ok(self.scratch_steps()?.with_extension("encrust"))
    }

    /// Keeps the plate a check wrote at `from` as revision `number`'s `N.encrust`.
    pub fn keep_plate(&self, job: &str, number: u32, from: &Path) -> Result<()> {
        let path = self.dir(job)?.join(format!("{number}.encrust"));
        fs::rename(from, &path).with_context(|| format!("keeping {}", path.display()))
    }

    /// An empty directory of the job's for the pictures of one revision.
    pub fn pictures_dir(&self, revision: &Revision) -> Result<PathBuf> {
        self.empty_dir(&revision.job, &format!("pictures-{}", revision.number))
    }

    /// An empty directory of the job's for the pictures of the `pick`th pose offered for a
    /// revision.
    pub fn poses_dir(&self, revision: &Revision, pick: u64) -> Result<PathBuf> {
        self.empty_dir(&revision.job, &format!("poses-{}-{pick}", revision.number))
    }

    fn empty_dir(&self, job: &str, name: &str) -> Result<PathBuf> {
        let dir = self.dir(job)?.join(name);
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

    /// The edits revision `number` handed the open window, kept beside it as `N.edits.json`.
    pub fn keep_edits(&self, job: &str, number: u32, edits: &[Edit]) -> Result<()> {
        let path = self.dir(job)?.join(format!("{number}.edits.json"));
        fs::write(&path, serde_json::to_string(edits)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Notes that the open window holds revision `number`'s cut, which replaced `object`.
    pub fn keep_window_cut(&self, job: &str, number: u32, object: ObjectId) -> Result<()> {
        let path = self.dir(job)?.join(window_name(number));
        fs::write(&path, json!({"cut_object": object}).to_string())
            .with_context(|| format!("writing {}", path.display()))
    }

    /// The object revision `number`'s cut replaced on the open window's plate, if the window
    /// was handed the cut.
    pub fn window_cut(&self, job: &str, number: u32) -> Option<ObjectId> {
        let text = fs::read_to_string(self.dir(job).ok()?.join(window_name(number))).ok()?;
        let kept: Value = serde_json::from_str(&text).ok()?;
        serde_json::from_value(kept["cut_object"].clone()).ok()
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

/// The file the steps of revision `number` are kept in.
fn window_name(number: u32) -> String {
    format!("{number}.window.json")
}

fn steps_name(number: u32) -> String {
    format!("{number}.recipe.toml")
}

fn revision_number(path: &Path) -> Option<u32> {
    if path.extension()? != "toml" {
        return None;
    }
    path.file_stem()?.to_str()?.parse().ok()
}

#[cfg(test)]
mod tests;
