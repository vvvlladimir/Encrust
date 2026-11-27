//! Jobs on disk, a directory per job and a numbered recipe per revision with its check, its
//! flags and the steps it runs beside it; a call names its job, so no state lives in the
//! process but a lock per job.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_link::{RevisionFile, is_job_name, revision_of};
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
    /// The `core-recipe` file its check ran, the flags folded in (ADR 0461); what `save_recipe`
    /// keeps.
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
        let path = self.dir(job)?.join(RevisionFile::Recipe.of(number));
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
        (mut recipe, steps): (Recipe, Option<String>),
        report: &Value,
    ) -> Result<Revision> {
        recipe.pin_pose(report);
        let dir = self.dir(job)?;
        let number = if dir.is_dir() {
            self.latest(job)? + 1
        } else {
            1
        };
        fs::create_dir_all(&dir).with_context(|| format!("creating job {job}"))?;
        let path = dir.join(RevisionFile::Recipe.of(number));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("revision {number} of job {job} was taken"))?;
        file.write_all(toml::to_string(&recipe)?.as_bytes())
            .with_context(|| format!("writing {}", path.display()))?;
        let checked = dir.join(RevisionFile::Check.of(number));
        fs::write(&checked, report.to_string())
            .with_context(|| format!("writing {}", checked.display()))?;
        // The flags beside it, the model first, so a bench and the window read the revision
        // without reading a recipe.
        let args = dir.join(RevisionFile::Args.of(number));
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
        let file = self.dir(job).ok()?.join(RevisionFile::Check.of(number));
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

    /// A file no other run writes, for the command line to write the steps of a check into; one
    /// left behind is overwritten by no other run, its name being this run's.
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

    /// Revision `revision`'s plate as its check wrote it, for the window to show: refused for
    /// one made by edits on the window's plate, which holds it already, or one with none.
    pub fn shown_plate(&self, revision: &Revision) -> Result<PathBuf> {
        let dir = self.dir(&revision.job)?;
        let number = revision.number;
        if dir.join(RevisionFile::Edits.of(number)).is_file() {
            bail!("revision {number} was made on the window's plate, which holds it already");
        }
        let plate = dir.join(RevisionFile::Plate.of(number));
        if !plate.is_file() {
            bail!("revision {number} keeps no plate for the window to show");
        }
        Ok(plate)
    }

    /// Revision `revision`'s plate where it was made by edits on one: what it slices and draws,
    /// with the user's changes there, which its recipe does not hold.
    pub fn made_on_plate(&self, revision: &Revision) -> Option<PathBuf> {
        let dir = self.dir(&revision.job).ok()?;
        let number = revision.number;
        let plate = dir.join(RevisionFile::Plate.of(number));
        let made = dir.join(RevisionFile::Edits.of(number)).is_file() && plate.is_file();
        made.then_some(plate)
    }

    /// The plate revision `revision` stands as: the project it opened, else the plate its check
    /// wrote.
    pub fn plate_of(&self, revision: &Revision) -> Result<PathBuf> {
        if revision.recipe.is_project() {
            return Ok(revision.recipe.model.clone());
        }
        let plate = self
            .dir(&revision.job)?
            .join(RevisionFile::Plate.of(revision.number));
        if !plate.is_file() {
            bail!(
                "revision {} of {} keeps no plate",
                revision.number,
                revision.job
            );
        }
        Ok(plate)
    }

    /// Revision `revision`'s plate and the job's one model on it, the base of the next revision
    /// with no window answering; `None` without a plate, or where the model is now in pieces.
    pub fn kept_base(&self, revision: &Revision) -> Option<(PathBuf, ObjectId)> {
        let dir = self.dir(&revision.job).ok()?;
        let number = revision.number;
        let plate = dir.join(RevisionFile::Plate.of(number));
        // A plate of pieces is edited piece by piece (`kept_pieces`), never as one model.
        if !plate.is_file() || revision.recipe.is_project() || revision.recipe.makes_pieces() {
            return None;
        }
        let object = match fs::read_to_string(dir.join(RevisionFile::Edits.of(number))) {
            Ok(text) => {
                let edits: Vec<Edit> = serde_json::from_str(&text).ok()?;
                if edits.iter().any(makes_pieces) {
                    return None;
                }
                edits.iter().find_map(Edit::object)?
            }
            // A check of the model alone stands it as the plate's first object.
            Err(_) if revision.recipe.makes_pieces() => return None,
            Err(_) => ObjectId(1),
        };
        Some((plate, object))
    }

    /// The plate `revision` kept where its recipe left the model in pieces, which each later
    /// edit is made on.
    pub fn kept_pieces(&self, revision: &Revision) -> Option<PathBuf> {
        let plate = (self.dir(&revision.job).ok()?).join(RevisionFile::Plate.of(revision.number));
        let pieces = revision.recipe.makes_pieces() && !revision.recipe.is_project();
        (pieces && plate.is_file()).then_some(plate)
    }

    /// Keeps the plate a check wrote at `from` as revision `number`'s `N.encrust`.
    pub fn keep_plate(&self, job: &str, number: u32, from: &Path) -> Result<()> {
        let path = self.dir(job)?.join(RevisionFile::Plate.of(number));
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
            .filter_map(|entry| revision_of(&entry.ok()?.path()))
            .max()
            .with_context(|| format!("job {job} has no revision"))
    }

    /// The edits revision `number` handed the open window, kept beside it as `N.edits.json`.
    pub fn keep_edits(&self, job: &str, number: u32, edits: &[Edit]) -> Result<()> {
        let path = self.dir(job)?.join(RevisionFile::Edits.of(number));
        fs::write(&path, serde_json::to_string(edits)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Forgets the edits revision `number` handed the open window, which did not take them: the
    /// revision stands by its own plate again.
    pub fn forget_edits(&self, job: &str, number: u32) -> Result<()> {
        let path = self.dir(job)?.join(RevisionFile::Edits.of(number));
        fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))
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
        if !is_job_name(job) {
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

/// Whether `edit` leaves its model as pieces or joined into another, no longer one object.
fn makes_pieces(edit: &Edit) -> bool {
    matches!(
        edit,
        Edit::Cut { .. } | Edit::Split { .. } | Edit::PartOff { .. } | Edit::Combine { .. }
    )
}

/// The file the window's object of revision `number`'s cut is kept in, to follow its undo.
fn window_name(number: u32) -> String {
    format!("{number}.window.json")
}

/// The file the steps of revision `number` are kept in.
fn steps_name(number: u32) -> String {
    format!("{number}.recipe.toml")
}

#[cfg(test)]
mod tests;
