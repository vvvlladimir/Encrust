//! The window as `encrust-mcp`'s viewport: each job stands on a plate of its own as the plate its
//! check wrote, in place of its last, one undo each (ADR 0473, 0477, `window-link.md`).

mod line;
mod listen;
mod machine;
mod plate;
mod request;

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use serde_json::Value;

use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

use line::Line;
use listen::{Listening, default_file};
use request::{Reply, Request, Shown};

/// The port listened on and the jobs followed on the plate.
#[derive(Debug)]
pub struct Link {
    listening: Listening,
    followed: Vec<Followed>,
    /// Where the server keeps its jobs, beside the window's file.
    jobs: PathBuf,
    /// Edits a server handed over, being applied one after another (ADR 0466).
    applying: Option<plate::Applying>,
}

/// What the server is told of edits handed over while others are still being applied.
const EDITS_UNDER_WAY: &str = "the window is still applying the edits handed over before";

/// What the server is told of a revision handed over while the window does not follow.
const NOT_FOLLOWING: &str = "the window is not following its agents; it lists the revision, \
    which the user can show, or show_in_window shows it";

/// One job in the window: the name of the plate its revisions stand on, which nothing else
/// shares, and the revision waiting for the window to be still.
#[derive(Debug)]
struct Followed {
    job: String,
    model: PathBuf,
    plate: Option<String>,
    waiting: Option<Box<Shown>>,
}

impl Followed {
    /// The job's plate, while one of the window's plates still bears its name.
    fn plate_in(&self, scene: &Scene) -> Option<u32> {
        let name = self.plate.as_ref()?;
        let index = scene.plates().iter().position(|plate| plate == name)?;
        u32::try_from(index).ok()
    }
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
    pub fn start(file: PathBuf, wake: impl Fn() + Send + 'static) -> anyhow::Result<Self> {
        let jobs = file.with_file_name("mcp-jobs");
        Ok(Self {
            listening: Listening::start(file, wake)?,
            followed: Vec::new(),
            jobs,
            applying: None,
        })
    }

    /// Answers what the server asked since the last frame and shows what is waiting. Called
    /// every frame after the jobs are polled; returns whether a revision waits.
    pub fn poll(&mut self, window: &mut Window) -> bool {
        let follow = window.machine.general.follow_agents;
        while let Ok((line, reply)) = self.listening.asked.try_recv() {
            self.answer(line, reply, follow, window);
        }
        let applying = plate::advance(&mut self.applying, window);
        self.take_asked(window);
        for index in 0..self.followed.len() {
            self.show(index, window);
        }
        let scene = &window.doc.scene;
        window.machine.agents.on_plate = (self.followed.iter())
            .filter(|f| {
                f.plate_in(scene)
                    .is_some_and(|p| scene.on_plate(p).next().is_some())
            })
            .map(|f| f.job.clone())
            .collect();
        applying || self.followed.iter().any(|f| f.waiting.is_some())
    }

    /// Answers one line: a revision is taken to show, the plate handed over or edits applied
    /// as ADR 0466 does, all only while the window follows its agents.
    fn answer(&mut self, line: Line, reply: Sender<Value>, follow: bool, window: &mut Window) {
        let refused = |why: &str| serde_json::to_value(Reply::refused(why.to_owned()));
        match line {
            Line::Revision(shown) => {
                let answer = if shown.request.open || follow {
                    self.take(shown);
                    serde_json::to_value(Reply::taken()).unwrap_or_default()
                } else {
                    refused(NOT_FOLLOWING).unwrap_or_default()
                };
                let _ = reply.send(answer);
            }
            _ if !follow => {
                let _ = reply.send(refused(NOT_FOLLOWING).unwrap_or_default());
            }
            Line::Plate { job } => {
                let plate = self.plate_of(&job, &window.doc.scene);
                let object = plate.and_then(|plate| job_object(&window.doc.scene, plate));
                plate::hand_plate(&self.jobs, (&job, plate, object), window, reply);
            }
            Line::Apply { .. } if self.applying.is_some() => {
                let _ = reply.send(refused(EDITS_UNDER_WAY).unwrap_or_default());
            }
            Line::Apply {
                job,
                edits,
                base_version,
            } => {
                let plate = self.plate_of(&job, &window.doc.scene);
                self.applying = plate::start((edits, plate), base_version, window, reply);
            }
        }
    }

    /// Takes the revisions the user asked to see from the agents' list.
    fn take_asked(&mut self, window: &mut Window) {
        for asked in std::mem::take(&mut window.machine.agents.asked) {
            let read =
                Request::of_job(&self.jobs, &asked.job, asked.revision).and_then(Request::read);
            match read {
                Ok(shown) => self.take(Box::new(shown)),
                Err(error) => window.machine.status = Status::failed(&error),
            }
        }
    }

    /// Notes a revision to show, in place of one of its job still waiting.
    fn take(&mut self, shown: Box<Shown>) {
        let request = &shown.request;
        let index = self.followed_index(&request.job, &request.model);
        self.followed[index].waiting = Some(shown);
    }

    /// The plate `job` stands on in the window, `None` before one of its revisions is shown.
    fn plate_of(&self, job: &str, scene: &Scene) -> Option<u32> {
        let followed = self.followed.iter().find(|followed| followed.job == job)?;
        followed.plate_in(scene)
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
            plate: None,
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

    /// Stands the job's waiting revision on its own plate once nothing runs, a new plate
    /// where it has none: on the job's machine, its plate's objects in place of all there.
    fn show(&mut self, index: usize, window: &mut Window) {
        let followed = &mut self.followed[index];
        let (doc, tools, held) = (&*window.doc, &*window.tools, &*window.machine);
        if followed.waiting.is_none() || crate::prepare::busy(doc, tools, held) {
            return;
        }
        let Some(shown) = followed.waiting.take() else {
            return;
        };
        let Shown { request, project } = *shown;
        if let Err(error) = machine::set_machine(window, (&request.printer, &request.resin)) {
            window.machine.status = Status::failed(&error);
            return;
        }
        let scene = &mut window.doc.scene;
        match followed.plate_in(scene) {
            Some(plate) => scene.show_plate(plate),
            None => followed.plate = Some(new_plate(scene, &request)),
        }
        let standing: Vec<ObjectId> = scene.here().map(|object| object.id).collect();
        for id in standing {
            scene.remove(id);
        }
        crate::project::inserted::insert(project, scene, &mut window.tools.supports);
        window.machine.status = Status::Info(tr!(
            "link-replaying",
            job = request.job,
            revision = request.revision
        ));
    }
}

/// The job's model where it stands on `plate` whole: its one object that is no plug, `None`
/// where it is in pieces or gone.
fn job_object(scene: &Scene, plate: u32) -> Option<ObjectId> {
    let mut whole = scene
        .on_plate(plate)
        .filter(|object| object.plug_of.is_none());
    let first = whole.next()?.id;
    whole.next().is_none().then_some(first)
}

/// Adds a plate for the job `request` names, called for its model and numbered past a name
/// taken, shown; answers its name.
fn new_plate(scene: &mut Scene, request: &request::Request) -> String {
    let stem = request
        .model
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned());
    let stem = stem.unwrap_or_else(|| request.job.clone());
    let name = (1..)
        .map(|number| match number {
            1 => stem.clone(),
            number => tr!("link-plate-numbered", model = stem.clone(), number = number),
        })
        .find(|name| !scene.plates().contains(name))
        .unwrap_or(stem);
    let plate = scene.add_plate();
    scene.rename_plate(plate, name.clone());
    name
}

#[cfg(test)]
mod tests;
