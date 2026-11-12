use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_engine::{SupportPlan, SupportShapes, Underside, strengthen};
use core_geometry::Region;
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_supports::{Blocked, Need, Shown, WallSide, generate_supports, low_points};
use printer_profiles::SupportProfile;

use crate::i18n::tr;
use crate::job::pipeline::{slice_mesh, thread_pool, worker_threads};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::supports::Sizing;

/// How much of the run has to pass before the worker says so again. A layer is a
/// microsecond and a frame is sixteen thousand of them, so a message a layer would only
/// fill the channel.
const REPORT_STEP: f32 = 0.005;

/// One model to stand supports under, owned so that it can cross to the worker thread.
pub struct SupportTask {
    pub id: ObjectId,
    /// The model in its own space, which the blocked faces are painted on, and where it
    /// stands.
    pub mesh: Arc<Mesh>,
    /// What the model prints as, in the same space: what is cut and read for undersides.
    pub surface: Arc<Mesh>,
    pub transform: Transform,
    /// Contacts of the supports already under this model, in plate coordinates. They
    /// hold up what is near them, and the run never returns or moves them.
    pub seeds: Vec<Vec3>,
    /// The faces painted out of bounds, which the run puts nothing on or near.
    pub blocked: Region,
    /// The faces painted as the side meant to be seen, held sparser and on the lightest tips.
    pub shown: Region,
    /// What this model's supports are placed by: its own profile where it keeps one.
    pub profile: SupportProfile,
    /// The side of a hollow model's wall the run holds up, or `None` for a solid one.
    pub side: Option<WallSide>,
}

/// Everything one automatic placement run needs.
pub struct SupportRequest {
    pub tasks: Vec<SupportTask>,
    pub layer_height_mm: Scalar,
    /// The automatic profile the run sizes its supports by, which holds steps apart from
    /// overhangs, and its shapes; `None` for a run built to the group in hand.
    pub sizing: Option<Sizing>,
}

/// Where the run wants supports under one model, in the plate coordinates that model
/// stood in while the run went on.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub id: ObjectId,
    /// The placement the contacts were worked out against. The model may have been moved
    /// since, so the contacts are mapped back into its own space through this rather than
    /// through where it stands now.
    pub transform: Transform,
    pub needs: Vec<Need>,
}

/// How an automatic placement run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum SupportOutcome {
    Placed(Vec<Placement>),
    Cancelled,
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

#[derive(Debug)]
enum Report {
    /// Share of the work done, over every model of the run.
    Progress(f32),
    Finished(SupportOutcome),
}

/// An automatic placement run on its own thread, and how far it has got.
///
/// Placement cuts the plate again, which is the same work the Slice button does and far
/// too much for a frame; see `docs/decisions/0029`.
#[derive(Debug)]
pub struct SupportJob {
    reports: Receiver<Report>,
    cancel: Arc<AtomicBool>,
    /// `None` until the first layer is reported: the stack of the first model is cut
    /// before anything is placed, which is work no share can be taken of.
    fraction: Option<f32>,
}

impl SupportJob {
    /// Starts the run. The thread is detached: it ends on its own, and dropping the
    /// handle cancels it.
    pub fn spawn(request: SupportRequest) -> Self {
        let (sender, reports) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);

        crate::job::spawn(move || {
            let outcome = run(&request, &worker_cancel, &mut |fraction| {
                let _ = sender.send(Report::Progress(fraction));
            });
            let _ = sender.send(Report::Finished(outcome));
        });

        Self {
            reports,
            cancel,
            fraction: None,
        }
    }

    /// Takes everything the worker reported since the last frame, and returns the outcome
    /// on the frame the run ends.
    pub fn poll(&mut self) -> Option<SupportOutcome> {
        loop {
            match self.reports.try_recv() {
                Ok(Report::Progress(fraction)) => self.fraction = Some(fraction),
                Ok(Report::Finished(outcome)) => return Some(outcome),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(SupportOutcome::Failed(tr!("job-thread-stopped-support")));
                }
            }
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The run has been told to stop but has not reached its next layer yet.
    pub fn is_cancelling(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn fraction(&self) -> Option<f32> {
        self.fraction
    }

    pub fn label(&self) -> String {
        if self.is_cancelling() {
            return tr!("job-cancelling");
        }
        match self.fraction {
            Some(fraction) => tr!("job-placing", percent = format!("{:.0}", fraction * 100.0)),
            None => tr!("job-cutting-plate"),
        }
    }
}

/// A run whose handle is gone has nobody to hand its points to, so it is stopped rather
/// than left cutting a plate nothing will read.
impl Drop for SupportJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Every visible model with geometry on the plate being edited, and the supports already
/// under it.
pub fn tasks_of(
    scene: &Scene,
    profile_of: impl Fn(&SceneObject) -> SupportProfile,
) -> Vec<SupportTask> {
    scene
        .targets()
        .filter(|object| !object.mesh.is_empty())
        .map(|object| SupportTask::of(object, profile_of(object)))
        .collect()
}

impl SupportTask {
    /// `object` as it stands, with the supports already under it, placed to `profile`.
    pub fn of(object: &SceneObject, profile: SupportProfile) -> Self {
        Self {
            id: object.id,
            mesh: Arc::clone(&object.mesh),
            surface: Arc::clone(object.support_surface().0),
            transform: object.transform,
            seeds: object.supports.contacts(object.transform),
            blocked: object.supports.blocked().clone(),
            shown: object.shown_painted.clone(),
            profile,
            side: None,
        }
    }
}

/// Cuts every model of the request and works out where it needs holding up.
///
/// Every failure comes back as `SupportOutcome::Failed` rather than a `Result`: the
/// caller is a worker thread whose only channel to the user is `report`.
fn run(
    request: &SupportRequest,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(f32) + Send),
) -> SupportOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return SupportOutcome::Failed(error.to_string()),
    };
    pool.install(|| place(request, cancel, report))
}

fn place(
    request: &SupportRequest,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(f32) + Send),
) -> SupportOutcome {
    let models = request.tasks.len() as f32;
    let mut placements = Vec::with_capacity(request.tasks.len());

    for (index, task) in request.tasks.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return SupportOutcome::Cancelled;
        }
        let share = (index as f32 / models, 1.0 / models);
        let needs = match needs_of(task, request, share, cancel, report) {
            Ok(needs) => needs,
            Err(error) => {
                return SupportOutcome::Failed(
                    error
                        .chain()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(": "),
                );
            }
        };
        if cancel.load(Ordering::Relaxed) {
            return SupportOutcome::Cancelled;
        }
        placements.push(Placement {
            id: task.id,
            transform: task.transform,
            needs,
        });
    }

    report(1.0);
    SupportOutcome::Placed(placements)
}

/// Where `task`'s model needs holding up, strengthened where a tip carries too much; its
/// layers reported as the `(done, of)` share of the run they are.
fn needs_of(
    task: &SupportTask,
    request: &SupportRequest,
    (done, of): (f32, f32),
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(f32) + Send),
) -> anyhow::Result<Vec<Need>> {
    let placed = if task.transform == Transform::default() {
        task.surface.as_ref().clone()
    } else {
        transform_mesh(&task.surface, task.transform)
    };
    let stack = slice_mesh(&placed, request.layer_height_mm)?;
    let layers = stack.layers.len().max(1) as f32;
    let mut last = done;
    let painted = Blocked::new(&task.mesh, &task.blocked, task.transform);
    let blocked = Blocked::keeping(painted, task.side.clone());
    let shown = Shown::new(&task.mesh, &task.shown, task.transform);
    // A support on the far side of the wall holds up nothing on this one.
    let seeds: Vec<Vec3> = match &task.side {
        Some(side) => task
            .seeds
            .iter()
            .copied()
            .filter(|seed| side.keeps(*seed))
            .collect(),
        None => task.seeds.clone(),
    };
    let plan = plan_of(task, request.sizing.as_ref());
    let mut needs = generate_supports(
        &stack,
        request.layer_height_mm,
        &task.profile,
        plan.sizing(),
        &seeds,
        &low_points(&task.surface, task.transform),
        blocked.as_ref(),
        shown.as_ref(),
        &mut |layer| {
            let fraction = done + layer as f32 / layers * of;
            if fraction - last >= REPORT_STEP {
                last = fraction;
                report(fraction);
            }
            !cancel.load(Ordering::Relaxed)
        },
    );
    let underside = Underside {
        stack: &stack,
        layer_mm: request.layer_height_mm,
        surface: &placed,
        bvh: &Bvh::build(&placed),
        blocked: blocked.as_ref(),
    };
    // Tips beyond the wall hold the same shell, so they share its pull all the same.
    strengthen(underside, &mut needs, &task.seeds, &plan, &mut || {
        !cancel.load(Ordering::Relaxed)
    })?;
    Ok(needs)
}

/// What `task`'s supports are built to: `sizing`'s shapes, or its own profile.
fn plan_of(task: &SupportTask, sizing: Option<&Sizing>) -> SupportPlan {
    match sizing {
        Some((auto, shapes)) => SupportPlan {
            shapes: SupportShapes::Sized {
                auto: Box::new(auto.clone()),
                shapes: Box::new(shapes.clone()),
            },
            cavity: false,
        },
        None => SupportPlan::of(task.profile.clone()),
    }
}

#[cfg(test)]
mod tests;
