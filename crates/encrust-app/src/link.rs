//! The window as `encrust-mcp`'s viewport: each revision a chat client's job keeps is replayed
//! on the open plate, one undo entry a revision (ADR 0401, `docs/design/window-link.md`).

mod listen;
mod machine;
mod recipe;
mod shaping;
mod unshown;

use std::path::PathBuf;

use anyhow::Result;

use crate::files::Handed;
use crate::i18n::tr;
use crate::panels::Window;
use crate::prepare::Plan;
use crate::scene::{ObjectId, SceneObject};
use crate::status::Status;

use listen::{Listening, default_file};
use recipe::{Reply, Request};

/// The port listened on and the jobs followed on the plate.
#[derive(Debug)]
pub struct Link {
    listening: Listening,
    followed: Vec<Followed>,
    /// The job whose replay the Prepare run going is, and the plate's models as it began,
    /// to tell what it left.
    replaying: Option<(usize, Vec<ObjectId>)>,
}

/// One job on the plate: its model and the others a combine joins, each as it came in once
/// read, what the last replay left besides, and the revision waiting to be replayed.
#[derive(Debug)]
struct Followed {
    job: String,
    models: Vec<(PathBuf, Option<SceneObject>)>,
    left: Vec<ObjectId>,
    waiting: Option<Waiting>,
}

/// A revision to replay: its plan and the machine it is printed on.
#[derive(Debug)]
struct Waiting {
    revision: u32,
    plan: Plan,
    printer: String,
    resin: String,
    slicing: Option<machine::Slicing>,
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
            replaying: None,
        })
    }

    /// Answers what the server asked since the last frame and replays what is waiting.
    /// Called every frame after the jobs are polled; returns whether a revision waits or is
    /// being replayed.
    pub fn poll(&mut self, window: &mut Window) -> bool {
        for (path, id) in window.doc.imports.take_landed() {
            let mut reading = self
                .followed
                .iter_mut()
                .flat_map(|followed| &mut followed.models);
            if let Some((_, model)) = reading.find(|(at, model)| model.is_none() && *at == path) {
                *model = window.doc.scene.get(id).cloned();
            }
        }
        while let Ok((request, reply)) = self.listening.asked.try_recv() {
            let _ = reply.send(self.take(request, window));
        }
        self.note_what_was_left(window);
        for index in 0..self.followed.len() {
            self.replay(index, window);
        }
        self.replaying.is_some()
            || self
                .followed
                .iter()
                .any(|followed| followed.waiting.is_some())
    }

    /// Notes a revision to replay, reading in the models the plate does not have yet.
    fn take(&mut self, request: Request, window: &mut Window) -> Reply {
        let (plan, unshown) = request.recipe.plan();
        let mut paths = vec![request.recipe.model.clone()];
        let combine = request
            .recipe
            .modelling
            .as_ref()
            .and_then(|modelling| modelling.combine.as_ref());
        paths.extend(
            combine
                .iter()
                .flat_map(|combine| combine.with.iter().cloned()),
        );
        let index = match self
            .followed
            .iter()
            .position(|followed| followed.job == request.job)
        {
            Some(index) => index,
            None => {
                self.followed.push(Followed {
                    job: request.job.clone(),
                    models: Vec::new(),
                    left: Vec::new(),
                    waiting: None,
                });
                self.followed.len() - 1
            }
        };
        let followed = &mut self.followed[index];
        followed.models.retain(|(path, _)| paths.contains(path));
        for path in paths {
            if !followed.models.iter().any(|(at, _)| *at == path) {
                let (doc, status) = (&mut window.doc, &mut window.machine.status);
                doc.imports
                    .open(Handed::Path(path.clone()), &doc.plate, status);
                followed.models.push((path, None));
            }
        }
        let first = &request.recipe.model;
        followed.models.sort_by_key(|(path, _)| path != first);
        followed.waiting = Some(Waiting {
            revision: request.revision,
            plan,
            printer: request.recipe.printer.clone(),
            resin: request.recipe.resin.clone(),
            slicing: request.recipe.slicing.clone(),
        });
        Reply {
            shown: true,
            unshown,
            why_not: None,
        }
    }

    /// Once the replay going has ended, notes what it left on the plate besides its models,
    /// for the next revision to take away.
    fn note_what_was_left(&mut self, window: &Window) {
        let Some((index, before)) = &self.replaying else {
            return;
        };
        if window.tools.prepare.is_running() {
            return;
        }
        if let Some(followed) = self.followed.get_mut(*index) {
            let models: Vec<ObjectId> = followed
                .models
                .iter()
                .filter_map(|(_, model)| Some(model.as_ref()?.id))
                .collect();
            followed.left = window
                .doc
                .scene
                .objects()
                .iter()
                .map(|object| object.id)
                .filter(|id| !before.contains(id) && !models.contains(id))
                .collect();
        }
        self.replaying = None;
    }

    /// Starts the job's waiting revision once its models are in and nothing else runs on the
    /// plate: on the job's machine, from the models as they came in, what the last replay left
    /// taken away. A newer revision stops the replay of an older one first.
    fn replay(&mut self, index: usize, window: &mut Window) {
        let followed = &self.followed[index];
        let ready = followed.models.iter().all(|(_, model)| model.is_some());
        if !ready || followed.waiting.is_none() {
            return;
        }
        if self.replaying.is_some() && window.tools.prepare.is_running() {
            crate::prepare::cancel(window.doc, window.tools, window.machine);
        }
        self.note_what_was_left(window);
        if crate::prepare::busy(window.doc, window.tools, window.machine).is_some() {
            return;
        }
        let followed = &mut self.followed[index];
        let Some(waiting) = followed.waiting.take() else {
            return;
        };
        let fresh: Vec<SceneObject> = followed
            .models
            .iter()
            .filter_map(|(_, model)| model.clone())
            .collect();
        let machine = (waiting.printer.as_str(), waiting.resin.as_str());
        let started =
            machine::set_machine(window, machine, waiting.slicing.as_ref()).and_then(|()| {
                let (doc, tools, held) =
                    (&mut *window.doc, &mut *window.tools, &mut *window.machine);
                crate::prepare::start_planned(
                    doc,
                    tools,
                    held,
                    waiting.plan,
                    (&fresh, &followed.left),
                )
            });
        let before = window
            .doc
            .scene
            .objects()
            .iter()
            .map(|object| object.id)
            .collect();
        self.replaying =
            (started.is_ok() && window.tools.prepare.is_running()).then_some((index, before));
        window.machine.status = match started {
            Ok(()) => Status::Info(tr!(
                "link-replaying",
                job = followed.job.clone(),
                revision = waiting.revision
            )),
            Err(error) => Status::failed(&error),
        };
    }
}

#[cfg(test)]
mod tests;
