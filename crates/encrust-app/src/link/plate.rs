//! Schema 3 of the window link: the plate handed to a server as it stands, and the edits it
//! built on it applied while the plate is still that version (ADR 0466).

use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::Sender;

use core_document::CutAsked;
use core_edit::{Edit, ObjectId};
use serde_json::{Value, json};

use crate::panels::Window;

/// Edits being applied one after another, each a job of the tool that carries it out.
#[derive(Debug)]
pub struct Applying {
    edits: VecDeque<Edit>,
    reply: Sender<Value>,
    /// The object the edit under way replaces, there still if the edit did not land.
    under_way: Option<ObjectId>,
}

fn refused(why: String) -> Value {
    json!({"applied": false, "why_not": why})
}

/// Writes the plate as it stands into `job`'s folder under `jobs` off the drawing thread,
/// and answers with its path, the plate's version, the ids on it and `job_object`, the
/// job's model where it stands on the plate whole.
pub fn hand_plate(
    jobs: &Path,
    (job, job_object): (&str, Option<ObjectId>),
    window: &Window,
    reply: Sender<Value>,
) {
    let Some(dir) = job_dir(jobs, job) else {
        let _ = reply.send(json!({"plate": null, "why_not": format!("{job:?} names no job")}));
        return;
    };
    let version = window.doc.history.plate_version();
    let project = crate::project::captured(window);
    let objects: Vec<ObjectId> = (project.manifest.objects.iter()).map(|o| o.id).collect();
    let path = dir.join(format!("base-{version}.encrust"));
    std::thread::spawn(move || {
        let written = std::fs::create_dir_all(&dir)
            .map_err(|error| error.to_string())
            .and_then(|()| core_engine::project::save(&path, &project).map_err(|e| e.to_string()));
        let answer = match written {
            Ok(()) => json!({"plate": path, "version": version, "objects": objects,
                             "job_object": job_object}),
            Err(why) => json!({"plate": null, "why_not": why}),
        };
        let _ = reply.send(answer);
    });
}

/// `job`'s folder under `jobs`, for a job named by one plain path segment alone.
fn job_dir(jobs: &Path, job: &str) -> Option<PathBuf> {
    let mut parts = Path::new(job).components();
    match (parts.next(), parts.next()) {
        (Some(Component::Normal(_)), None) => Some(jobs.join(job)),
        _ => None,
    }
}

/// Starts applying `edits` if the plate is still at `base_version`; otherwise answers
/// `stale` with the version it is at, for the server to build again.
pub fn start(
    edits: Vec<Edit>,
    base_version: u64,
    window: &Window,
    reply: Sender<Value>,
) -> Option<Applying> {
    let version = window.doc.history.plate_version();
    if base_version != version {
        let _ = reply.send(json!({"applied": false, "stale": true, "version": version}));
        return None;
    }
    Some(Applying {
        edits: edits.into(),
        reply,
        under_way: None,
    })
}

/// Moves the edits along once the tool carrying the last one is done, answering when all
/// have landed or one did not. Answers whether edits are still being applied.
pub fn advance(applying: &mut Option<Applying>, window: &mut Window) -> bool {
    let Some(work) = applying.as_mut() else {
        return false;
    };
    if window.tools.cut.is_busy() {
        return true;
    }
    if let Some(id) = work.under_way.take()
        && window.doc.scene.get(id).is_some()
    {
        let why = window.machine.status.text();
        return finish(applying, refused(why));
    }
    let Some(edit) = work.edits.pop_front() else {
        return finish(applying, json!({"applied": true}));
    };
    match start_edit(&edit, window) {
        Ok(id) => {
            work.under_way = Some(id);
            true
        }
        Err(why) => finish(applying, refused(why)),
    }
}

fn finish(applying: &mut Option<Applying>, answer: Value) -> bool {
    if let Some(work) = applying.take() {
        let _ = work.reply.send(answer);
    }
    false
}

/// Starts the tool job that carries `edit` out, answering the object it replaces.
fn start_edit(edit: &Edit, window: &mut Window) -> Result<ObjectId, String> {
    match *edit {
        Edit::Cut {
            object,
            axis,
            at_mm,
            keep,
            joints,
            lay_flat,
        } => {
            let asked = CutAsked {
                axis,
                at_mm,
                keep,
                joints,
                lay_flat,
            };
            (window.tools.cut)
                .start_asked(&window.doc.scene, object, asked)
                .map_err(|error| format!("{error:#}"))?;
            Ok(object)
        }
    }
}
