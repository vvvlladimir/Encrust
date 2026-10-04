use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Bvh, Mesh, Scalar, Vec3};
use core_volume::{HollowSettings, Shell, hollow};

use crate::job::pipeline::{thread_pool, worker_threads};
use crate::scene::{ObjectId, Scene};

/// One model to hollow, owned so that it can cross to the worker thread.
pub struct HollowTask {
    pub id: ObjectId,
    /// The model and its hierarchy, both in the model's own space, which is the space the
    /// cavity is worked out and kept in.
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    /// What the model is scaled by where it stands. The wall is measured in the model's
    /// own space, so a model rescaled afterwards has to be hollowed again.
    pub scale: Vec3,
    pub settings: HollowSettings,
}

/// Everything one hollowing run needs.
pub struct HollowRequest {
    pub tasks: Vec<HollowTask>,
}

/// One model hollowed, in its own space, and which object it was.
#[derive(Debug, Clone, PartialEq)]
pub struct Shelled {
    pub id: ObjectId,
    pub shell: Shell,
}

/// How a hollowing run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum HollowOutcome {
    Hollowed(Vec<Shelled>),
    Cancelled,
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

impl HollowOutcome {
    /// Resin the run takes out of the plate, in cubic millimetres.
    pub fn saved_mm3(&self) -> Scalar {
        match self {
            Self::Hollowed(shells) => shells.iter().map(|shelled| shelled.shell.cavity_mm3).sum(),
            Self::Cancelled | Self::Failed(_) => 0.0,
        }
    }
}

#[derive(Debug)]
enum Report {
    /// Share of the models done, and the name of the one in hand.
    Progress(f32),
    Finished(HollowOutcome),
}

/// A hollowing run on its own thread, and how far it has got.
///
/// Building the field is seconds of work on a real model, which is far too much for a
/// frame; the window keeps drawing while it goes, the way slicing and placement do.
#[derive(Debug)]
pub struct HollowJob {
    reports: Receiver<Report>,
    cancel: Arc<AtomicBool>,
    fraction: f32,
}

impl HollowJob {
    /// Starts the run. The thread is detached: it ends on its own, and dropping the
    /// handle cancels it.
    pub fn spawn(request: HollowRequest) -> Self {
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
            fraction: 0.0,
        }
    }

    /// Takes everything the worker reported since the last frame, and returns the outcome
    /// on the frame the run ends.
    pub fn poll(&mut self) -> Option<HollowOutcome> {
        loop {
            match self.reports.try_recv() {
                Ok(Report::Progress(fraction)) => self.fraction = fraction,
                Ok(Report::Finished(outcome)) => return Some(outcome),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(HollowOutcome::Failed(
                        "the hollowing thread stopped without finishing".to_owned(),
                    ));
                }
            }
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The run has been told to stop but has not finished the model in hand yet.
    pub fn is_cancelling(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn fraction(&self) -> f32 {
        self.fraction
    }

    pub fn label(&self) -> String {
        if self.is_cancelling() {
            return "Stopping after this model".to_owned();
        }
        format!("Hollowing, {:.0} %", self.fraction * 100.0)
    }
}

/// A run whose handle is gone has nobody to hand its shells to, so it is stopped rather
/// than left building a field nothing will read.
impl Drop for HollowJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Every visible model with geometry on the plate being edited, with the blockers it
/// carries.
pub fn hollow_tasks(scene: &Scene, settings: &HollowSettings) -> Vec<HollowTask> {
    scene
        .targets()
        .filter(|object| !object.mesh.is_empty())
        .map(|object| HollowTask {
            id: object.id,
            mesh: Arc::clone(&object.mesh),
            bvh: Arc::clone(&object.bvh),
            scale: object.transform.scale,
            settings: object.hollow.asking(settings),
        })
        .collect()
}

/// Hollows every model of the request.
///
/// Every failure comes back as `HollowOutcome::Failed` rather than a `Result`: the caller
/// is a worker thread whose only channel to the user is `report`.
fn run(
    request: &HollowRequest,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(f32) + Send),
) -> HollowOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return HollowOutcome::Failed(error.to_string()),
    };
    pool.install(|| shell_each(request, cancel, report))
}

fn shell_each(
    request: &HollowRequest,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(f32) + Send),
) -> HollowOutcome {
    let models = request.tasks.len().max(1) as f32;
    let mut shells = Vec::with_capacity(request.tasks.len());

    for (index, task) in request.tasks.iter().enumerate() {
        // The field is built in one call, so a run can only be stopped between models.
        if cancel.load(Ordering::Relaxed) {
            return HollowOutcome::Cancelled;
        }
        report(index as f32 / models);

        match hollow(&task.mesh, &task.bvh, &task.settings) {
            Ok(hollowed) => shells.push(Shelled {
                id: task.id,
                shell: Shell {
                    mesh: Arc::new(hollowed.mesh),
                    cavity_mm3: hollowed.cavity_mm3,
                    voxel_mm: hollowed.voxel_mm,
                    coarsened: hollowed.coarsened,
                    scale: task.scale,
                    settings: task.settings.clone(),
                },
            }),
            Err(error) => return HollowOutcome::Failed(error.to_string()),
        }
    }

    report(1.0);
    HollowOutcome::Hollowed(shells)
}
