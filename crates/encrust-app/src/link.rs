//! The window as `encrust-mcp`'s viewport: each revision a chat client's job keeps is replayed
//! on the open plate, one undo entry a revision (ADR 0401, `docs/design/window-link.md`).

mod listen;
mod recipe;
mod unshown;

use std::path::PathBuf;

use anyhow::Result;

use crate::files::Handed;
use crate::i18n::tr;
use crate::prepare::Plan;
use crate::scene::SceneObject;
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;

use listen::{Listening, default_file};
use recipe::{Reply, Request};

/// The port listened on and the jobs followed on the plate.
#[derive(Debug)]
pub struct Link {
    listening: Listening,
    followed: Vec<Followed>,
    /// Whether the Prepare run going is a replay, which a newer revision may stop.
    replaying: bool,
}

/// One job on the plate: its model as it came in, and the revision waiting to be replayed.
#[derive(Debug)]
struct Followed {
    job: String,
    path: PathBuf,
    /// `None` while the model is being read.
    fresh: Option<SceneObject>,
    waiting: Option<(u32, Plan)>,
}

impl Link {
    /// The window listening for `encrust-mcp`, a frame asked of `ctx` when it is spoken to;
    /// a window that cannot listen still works alone.
    pub fn for_window(ctx: &egui::Context) -> Option<Self> {
        let file = default_file()?;
        let ctx = ctx.clone();
        Self::start(file, move || ctx.request_repaint())
            .inspect_err(|error| tracing::warn!("not listening for encrust-mcp: {error:#}"))
            .ok()
    }

    /// Listens for the server, the file that says where written to `file`.
    pub fn start(file: PathBuf, wake: impl Fn() + Send + 'static) -> Result<Self> {
        Ok(Self {
            listening: Listening::start(file, wake)?,
            followed: Vec::new(),
            replaying: false,
        })
    }

    /// Answers what the server asked since the last frame and replays what is waiting.
    /// Called every frame after the jobs are polled; returns whether a revision waits or is
    /// being replayed.
    pub fn poll(&mut self, doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) -> bool {
        for (path, id) in doc.imports.take_landed() {
            let reading = self
                .followed
                .iter_mut()
                .find(|followed| followed.fresh.is_none() && followed.path == path);
            if let Some(followed) = reading {
                followed.fresh = doc.scene.get(id).cloned();
            }
        }
        while let Ok((request, reply)) = self.listening.asked.try_recv() {
            let _ = reply.send(self.take(request, doc, machine));
        }
        self.replaying &= tools.prepare.is_running();
        for index in 0..self.followed.len() {
            self.replay(index, doc, tools, machine);
        }
        self.replaying
            || self
                .followed
                .iter()
                .any(|followed| followed.waiting.is_some())
    }

    /// Notes a revision to replay, reading its model in first if the plate does not have it.
    fn take(&mut self, request: Request, doc: &mut Doc, machine: &mut Machine) -> Reply {
        let (plan, unshown) = request.recipe.plan();
        // A model taken off the plate is read in again for its job's next revision.
        self.followed.retain(|followed| {
            (followed.fresh.as_ref()).is_none_or(|fresh| doc.scene.get(fresh.id).is_some())
        });
        let known = self.followed.iter().position(|followed| {
            followed.job == request.job && followed.path == request.recipe.model
        });
        let followed = match known {
            Some(index) => &mut self.followed[index],
            None => {
                let file = Handed::Path(request.recipe.model.clone());
                doc.imports.open(file, &doc.plate, &mut machine.status);
                self.followed.push(Followed {
                    job: request.job.clone(),
                    path: request.recipe.model.clone(),
                    fresh: None,
                    waiting: None,
                });
                let last = self.followed.len() - 1;
                &mut self.followed[last]
            }
        };
        followed.waiting = Some((request.revision, plan));
        Reply {
            shown: true,
            unshown,
            why_not: None,
        }
    }
}

impl Link {
    /// Starts the job's waiting revision once its model is in and nothing else runs on the
    /// plate; a newer revision stops the replay of an older one first.
    fn replay(&mut self, index: usize, doc: &mut Doc, tools: &mut Tools, machine: &mut Machine) {
        let followed = &mut self.followed[index];
        let (Some(fresh), Some((revision, _))) = (&followed.fresh, &followed.waiting) else {
            return;
        };
        let revision = *revision;
        if self.replaying {
            crate::prepare::cancel(doc, tools, machine);
            self.replaying = false;
        }
        if crate::prepare::busy(doc, tools, machine).is_some() {
            return;
        }
        let Some((_, plan)) = followed.waiting.take() else {
            return;
        };
        let fresh = std::slice::from_ref(fresh);
        let started = crate::prepare::start_planned(doc, tools, machine, plan, fresh);
        self.replaying = started.is_ok() && tools.prepare.is_running();
        machine.status = match started {
            Ok(()) => Status::Info(tr!(
                "link-replaying",
                job = followed.job.clone(),
                revision = revision
            )),
            Err(error) => Status::failed(&error),
        };
    }
}

#[cfg(test)]
mod tests;
