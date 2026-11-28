//! What the jobs folder keeps: a job left alone past `IDLE` goes, then the least recently
//! touched until the folder fits `BUDGET`, sparing a day's; a scratch file goes after a day.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use agent_link::is_job_name;

use super::Jobs;

/// How long a job nobody touched is kept.
const IDLE: Duration = Duration::from_hours(30 * 24);
/// How much the jobs may hold together, bytes: a revision of a large model keeps a plate of
/// tens of megabytes.
const BUDGET: u64 = 4 << 30;
/// How long a file a run left in the scratch folder is kept, and a job however large.
const SCRATCH_IDLE: Duration = Duration::from_hours(24);

/// What a pruning took away.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    pub jobs: usize,
    pub bytes: u64,
}

impl Jobs {
    /// Takes away the jobs left alone too long, then the least recently touched past the budget,
    /// and stale scratch files; a job touched in the day before `now` is never taken.
    pub fn prune(&self, now: SystemTime) -> Pruned {
        self.pruned_to(now, BUDGET)
    }

    fn pruned_to(&self, now: SystemTime, budget: u64) -> Pruned {
        let mut pruned = Pruned::default();
        let mut jobs: Vec<(SystemTime, u64, std::path::PathBuf)> = Vec::new();
        for entry in fs::read_dir(&self.root).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name == "scratch" {
                pruned.bytes += cleared(&path, now - SCRATCH_IDLE);
                continue;
            }
            let folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if folder && name.to_str().is_some_and(is_job_name) {
                let (touched, bytes) = touched_and_size(&path);
                jobs.push((touched, bytes, path));
            }
        }
        jobs.sort_by_key(|(touched, ..)| *touched);
        let mut held: u64 = jobs.iter().map(|(_, bytes, _)| bytes).sum();
        for (touched, bytes, path) in jobs {
            let idle = now.duration_since(touched).unwrap_or_default() > IDLE;
            let over =
                held > budget && now.duration_since(touched).unwrap_or_default() > SCRATCH_IDLE;
            if (idle || over) && fs::remove_dir_all(&path).is_ok() {
                held -= bytes;
                pruned.jobs += 1;
                pruned.bytes += bytes;
            }
        }
        pruned
    }
}

/// When anything in `dir` was last written, and what its files hold, bytes.
fn touched_and_size(dir: &Path) -> (SystemTime, u64) {
    let mut touched = modified(dir);
    let mut bytes = 0;
    let mut folders = vec![dir.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            // A link is not followed, so a loop of them cannot hold the walk.
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                folders.push(path);
                continue;
            }
            touched = touched.max(modified(&path));
            bytes += entry.metadata().map_or(0, |metadata| metadata.len());
        }
    }
    (touched, bytes)
}

/// Removes the files in `dir` last written before `before`; answers the bytes they held.
fn cleared(dir: &Path, before: SystemTime) -> u64 {
    let mut bytes = 0;
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let len = entry.metadata().map_or(0, |metadata| metadata.len());
        if path.is_file() && modified(&path) < before && fs::remove_file(&path).is_ok() {
            bytes += len;
        }
    }
    bytes
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("encrust-mcp-pruned-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the temp dir is writable");
        dir
    }

    #[test]
    fn a_job_left_alone_past_a_month_goes_and_a_recent_one_stays() {
        let dir = root("idle");
        for job in ["cube-1", "bust-2"] {
            fs::create_dir_all(dir.join(job)).expect("the temp dir is writable");
            fs::write(dir.join(job).join("1.toml"), b"model = 1").expect("writable");
        }
        let jobs = Jobs::new(dir.clone());
        let later = SystemTime::now() + IDLE + Duration::from_secs(60);
        assert_eq!(
            jobs.prune(SystemTime::now()).jobs,
            0,
            "nothing is a month old yet"
        );
        assert_eq!(jobs.prune(later).jobs, 2, "both are a month old then");
        assert!(!dir.join("cube-1").exists());
    }

    #[test]
    fn past_the_budget_the_least_recently_touched_job_goes_first() {
        let dir = root("budget");
        for (job, bytes) in [("old-1", 600), ("new-2", 600)] {
            fs::create_dir_all(dir.join(job)).expect("the temp dir is writable");
            fs::write(dir.join(job).join("1.toml"), vec![b'#'; bytes]).expect("writable");
        }
        let old = SystemTime::now() - Duration::from_hours(48);
        let file = fs::File::options()
            .write(true)
            .open(dir.join("old-1/1.toml"));
        (file.expect("just written").set_modified(old)).expect("the time can be set");
        fs::File::open(dir.join("old-1"))
            .and_then(|folder| folder.set_modified(old))
            .expect("the folder's time can be set");
        let later = SystemTime::now() + Duration::from_hours(25);
        let pruned = Jobs::new(dir.clone()).pruned_to(later, 1000);
        assert_eq!(pruned.jobs, 1, "one job brings 1200 bytes under 1000");
        assert!(!dir.join("old-1").exists(), "the older goes");
        assert!(dir.join("new-2").exists());
    }

    #[test]
    fn a_job_touched_within_a_day_stays_past_the_budget() {
        let dir = root("recent");
        fs::create_dir_all(dir.join("big-1")).expect("the temp dir is writable");
        fs::write(dir.join("big-1").join("1.toml"), vec![b'#'; 600]).expect("writable");
        let pruned = Jobs::new(dir.clone()).pruned_to(SystemTime::now(), 100);
        assert_eq!(pruned.jobs, 0);
    }

    #[test]
    fn a_folder_that_is_no_job_is_left_alone() {
        let dir = root("other");
        fs::create_dir_all(dir.join("Not A Job!")).expect("the temp dir is writable");
        let later = SystemTime::now() + IDLE + Duration::from_secs(60);
        Jobs::new(dir.clone()).prune(later);
        assert!(dir.join("Not A Job!").exists());
    }
}
