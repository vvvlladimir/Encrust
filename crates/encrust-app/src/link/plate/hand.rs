//! The job's plate written for a server to build its edits on (ADR 0466, 0477).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use agent_link::{HandedPlate, is_job_name, plate_refused};
use core_edit::ObjectId;
use serde_json::Value;

use super::reply_of;
use crate::panels::Window;

/// Writes the job's plate into its folder under `jobs` off the drawing thread, answering its
/// path, version, ids and `job_object`, the job's model where it stands whole.
pub fn hand_plate(
    jobs: &Path,
    (job, plate, job_object): (&str, Option<u32>, Option<ObjectId>),
    window: &Window,
    reply: Sender<Value>,
) {
    let Some(dir) = job_dir(jobs, job) else {
        let _ = reply.send(plate_refused(format!("{job:?} names no job")));
        return;
    };
    let version = window.doc.history.plate_version();
    let project = crate::project::captured_plate(window, plate);
    let objects: Vec<ObjectId> = (project.manifest.objects.iter()).map(|o| o.id).collect();
    let path = dir.join(format!("base-{version}.encrust"));
    std::thread::spawn(move || {
        let written = std::fs::create_dir_all(&dir)
            .map_err(|error| error.to_string())
            .and_then(|()| core_engine::project::save(&path, &project).map_err(|e| e.to_string()));
        let answer = match written {
            Ok(()) => reply_of(&HandedPlate {
                path,
                version,
                objects,
                job_object,
            }),
            Err(why) => plate_refused(why),
        };
        let _ = reply.send(answer);
    });
}

/// `job`'s folder under `jobs`, for a name that reaches no other.
fn job_dir(jobs: &Path, job: &str) -> Option<PathBuf> {
    is_job_name(job).then(|| jobs.join(job))
}
