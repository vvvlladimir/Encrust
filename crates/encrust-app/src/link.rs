//! The window as `encrust-mcp`'s viewport: each revision a chat client's job keeps is run on
//! the open plate as Prepare for print by the revision's recipe, one undo entry a revision
//! (ADR 0459, `docs/design/window-link.md`).

mod listen;
mod machine;
mod request;

use std::path::PathBuf;

use anyhow::Result;
use core_recipe::Plan;

use crate::files::Handed;
use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::{ObjectId, SceneObject};
use crate::status::Status;

use listen::{Listening, default_file};
use request::{Reply, Request};

/// The port listened on and the jobs followed on the plate.
#[derive(Debug)]
pub struct Link {
    listening: Listening,
    followed: Vec<Followed>,
    /// The job whose revision the Prepare run going is, and the plate's models as it began,
    /// to tell what it left.
    replaying: Option<(usize, Vec<ObjectId>)>,
    /// Where the server keeps its jobs, beside the window's file.
    jobs: PathBuf,
}

/// What the server is told of a revision handed over while the window does not follow.
const NOT_FOLLOWING: &str = "the window is not following its agents; it lists the revision, \
    which the user can show, or show_in_window shows it";

/// One job on the plate: its model as it came in once read, what the last run left besides,
/// and the revision waiting to be run.
#[derive(Debug)]
struct Followed {
    job: String,
    model: PathBuf,
    fresh: Option<SceneObject>,
    reading: bool,
    left: Vec<ObjectId>,
    waiting: Option<Waiting>,
}

/// A revision to run: its recipe and the machine it is printed on.
#[derive(Debug)]
struct Waiting {
    revision: u32,
    plan: Plan,
    printer: String,
    resin: String,
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
        let jobs = file.with_file_name("mcp-jobs");
        Ok(Self {
            listening: Listening::start(file, wake)?,
            followed: Vec::new(),
            replaying: None,
            jobs,
        })
    }

    /// Answers what the server asked since the last frame and runs what is waiting. Called
    /// every frame after the jobs are polled; returns whether a revision waits or runs.
    pub fn poll(&mut self, window: &mut Window) -> bool {
        for (path, id) in window.doc.imports.take_landed() {
            let reading = self
                .followed
                .iter_mut()
                .find(|f| f.reading && f.model == path);
            if let Some(followed) = reading {
                followed.fresh = window.doc.scene.get(id).cloned();
                followed.reading = false;
            }
        }
        let follow = window.machine.general.follow_agents;
        while let Ok((request, reply)) = self.listening.asked.try_recv() {
            let answer = if request.open || follow {
                self.take(request, window)
            } else {
                Reply::refused(NOT_FOLLOWING.to_owned())
            };
            let _ = reply.send(answer);
        }
        self.take_asked(window);
        self.note_what_was_left(window);
        for index in 0..self.followed.len() {
            self.replay(index, window);
        }
        self.replaying.is_some() || self.followed.iter().any(|f| f.waiting.is_some())
    }

    /// Takes the revisions the user asked to see from the agents' list.
    fn take_asked(&mut self, window: &mut Window) {
        for asked in std::mem::take(&mut window.machine.agents.asked) {
            let taken = Request::of_job(&self.jobs, &asked.job, asked.revision)
                .map(|request| self.take(request, window));
            match taken {
                Ok(Reply { why_not: None, .. }) => {}
                Ok(Reply {
                    why_not: Some(why), ..
                }) => {
                    window.machine.status = Status::Error(why);
                }
                Err(error) => window.machine.status = Status::failed(&error),
            }
        }
        window.machine.agents.on_plate = self
            .followed
            .iter()
            .filter(|f| {
                f.fresh
                    .as_ref()
                    .is_some_and(|fresh| window.doc.scene.get(fresh.id).is_some())
            })
            .map(|f| f.job.clone())
            .collect();
    }

    /// Notes a revision to run, reading its model in where the plate does not have it.
    fn take(&mut self, request: Request, window: &mut Window) -> Reply {
        let plan = match request.plan() {
            Ok(plan) => plan,
            Err(error) => return Reply::refused(format!("{error:#}")),
        };
        let index = self.followed_index(&request.job, &request.model);
        let followed = &mut self.followed[index];
        let on_plate = followed
            .fresh
            .as_ref()
            .is_some_and(|fresh| window.doc.scene.get(fresh.id).is_some());
        // A model taken off the plate is read in again for its job's next revision.
        if !on_plate && !followed.reading {
            followed.fresh = None;
            followed.reading = true;
            let (doc, status) = (&mut window.doc, &mut window.machine.status);
            let file = Handed::Path(followed.model.clone());
            doc.imports.open(file, &doc.plate, status);
        }
        followed.waiting = Some(Waiting {
            revision: request.revision,
            plan,
            printer: request.printer,
            resin: request.resin,
        });
        Reply::taken()
    }

    /// The job's place in the list, a new entry where it is new or names another model. A
    /// new job of a model already followed takes that model's entry over, so a model is on
    /// the plate once however many jobs an agent starts on it.
    fn followed_index(&mut self, job: &str, model: &PathBuf) -> usize {
        if let Some(index) = self.followed.iter().position(|f| f.model == *model) {
            job.clone_into(&mut self.followed[index].job);
            return index;
        }
        let found = self.followed.iter().position(|f| f.job == job);
        let entry = Followed {
            job: job.to_owned(),
            model: model.clone(),
            fresh: None,
            reading: false,
            left: Vec::new(),
            waiting: None,
        };
        match found {
            Some(index) => {
                self.followed[index] = entry;
                index
            }
            None => {
                self.followed.push(entry);
                self.followed.len() - 1
            }
        }
    }

    /// Once the run going has ended, notes what it left on the plate besides its model, for
    /// the next revision to take away.
    fn note_what_was_left(&mut self, window: &Window) {
        let Some((index, before)) = &self.replaying else {
            return;
        };
        if window.tools.prepare.is_running() {
            return;
        }
        if let Some(followed) = self.followed.get_mut(*index) {
            let model = followed.fresh.as_ref().map(|fresh| fresh.id);
            followed.left = window
                .doc
                .scene
                .objects()
                .iter()
                .map(|object| object.id)
                .filter(|id| !before.contains(id) && Some(*id) != model)
                .collect();
        }
        self.replaying = None;
    }

    /// Starts the job's waiting revision once its model is in and nothing else runs on the
    /// plate: on the job's machine, from the model as it came in, what the last run left
    /// taken away. A newer revision stops the run of an older one of its own job first.
    fn replay(&mut self, index: usize, window: &mut Window) {
        let followed = &self.followed[index];
        if followed.fresh.is_none() || followed.waiting.is_none() {
            return;
        }
        if let Some((replaying, _)) = self.replaying
            && window.tools.prepare.is_running()
        {
            // Another job's run is never undone: this one waits for it to land.
            if replaying != index {
                return;
            }
            crate::prepare::cancel(window.doc, window.tools, window.machine);
        }
        self.note_what_was_left(window);
        if crate::prepare::busy(window.doc, window.tools, window.machine)
            || window.tools.prepare.is_running()
        {
            return;
        }
        let followed = &mut self.followed[index];
        let (Some(waiting), Some(fresh)) = (followed.waiting.take(), followed.fresh.clone()) else {
            return;
        };
        let started =
            machine::set_machine(window, (&waiting.printer, &waiting.resin)).and_then(|()| {
                let (doc, tools, held) =
                    (&mut *window.doc, &mut *window.tools, &mut *window.machine);
                let models = (std::slice::from_ref(&fresh), followed.left.as_slice());
                crate::prepare::start_replay(waiting.plan, models, doc, tools, held)
            });
        let before = window.doc.scene.objects().iter().map(|o| o.id).collect();
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
