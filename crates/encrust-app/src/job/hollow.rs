use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Bvh, Mat3, Mesh, Scalar};
use core_volume::{Cancel, HollowSettings, Shell, VolumeError, carve, cavity_field};

use crate::i18n::tr;
use crate::job::pipeline::{thread_pool, worker_threads};

mod fields;

use crate::scene::{ObjectId, Scene, SceneObject};
pub use fields::{SharedFields, lock};

/// One model to hollow, owned so that it can cross to the worker thread.
pub struct HollowTask {
    pub id: ObjectId,
    /// The model and its hierarchy, both in the model's own space, which is the space the
    /// cavity is worked out and kept in.
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    /// The placement with its turn taken out (`Transform::unturned`). The wall is measured
    /// on the plate under it, so a model resized afterwards has to be hollowed again.
    pub shape: Mat3,
    pub settings: HollowSettings,
}

/// Everything one hollowing run needs.
pub struct HollowRequest {
    pub tasks: Vec<HollowTask>,
    /// The fields the window keeps, read for a model built before and grown by each new one.
    pub fields: SharedFields,
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
    /// Models whose shell is built, of the models the run was given.
    Progress {
        done: usize,
        total: usize,
    },
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
    done: usize,
    total: usize,
}

impl HollowJob {
    /// Starts the run. The thread is detached: it ends on its own, and dropping the
    /// handle cancels it.
    pub fn spawn(request: HollowRequest) -> Self {
        let (sender, reports) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);

        crate::job::spawn(move || {
            let outcome = run(&request, &worker_cancel, &mut |done, total| {
                let _ = sender.send(Report::Progress { done, total });
            });
            let _ = sender.send(Report::Finished(outcome));
        });

        Self {
            reports,
            cancel,
            done: 0,
            total: 0,
        }
    }

    /// Takes everything the worker reported since the last frame, and returns the outcome
    /// on the frame the run ends.
    pub fn poll(&mut self) -> Option<HollowOutcome> {
        loop {
            match self.reports.try_recv() {
                Ok(Report::Progress { done, total }) => {
                    self.done = done;
                    self.total = total;
                }
                Ok(Report::Finished(outcome)) => return Some(outcome),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(HollowOutcome::Failed(tr!("job-thread-stopped-hollowing")));
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

    /// Share of the models shelled, or `None` where there is nothing honest to show: a
    /// cavity is built in one call, so a single model is either going or done.
    pub fn fraction(&self) -> Option<f32> {
        (self.total > 1).then(|| self.done as f32 / self.total as f32)
    }

    pub fn label(&self) -> String {
        if self.is_cancelling() {
            return tr!("job-hollow-stopping");
        }
        match self.fraction() {
            Some(_) => tr!(
                "job-hollow-progress",
                index = self.done + 1,
                total = self.total
            ),
            None => tr!("job-hollow-working"),
        }
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
pub fn hollow_tasks(
    scene: &Scene,
    settings: impl Fn(&SceneObject) -> HollowSettings,
) -> Vec<HollowTask> {
    scene
        .targets()
        .filter(|object| !object.mesh.is_empty())
        .map(|object| HollowTask {
            id: object.id,
            mesh: Arc::clone(&object.mesh),
            bvh: Arc::clone(&object.bvh),
            shape: object.transform.unturned(),
            settings: object.hollow.asking(&settings(object)),
        })
        .collect()
}

/// Every visible model whose shell no longer matches the cuts it carries, to be built
/// again at the numbers it was built at.
pub fn rebuild_tasks(scene: &Scene) -> Vec<HollowTask> {
    scene
        .targets()
        .filter_map(|object| {
            Some(HollowTask {
                id: object.id,
                mesh: Arc::clone(&object.mesh),
                bvh: Arc::clone(&object.bvh),
                shape: object.transform.unturned(),
                settings: object.hollow.rebuild()?,
            })
        })
        .collect()
}

/// The models of `models` still on the plate, each to be built at the settings beside it
/// with the blockers and channels it carries now.
pub fn again_tasks(scene: &Scene, models: &[(ObjectId, HollowSettings)]) -> Vec<HollowTask> {
    models
        .iter()
        .filter_map(|(id, asked)| {
            let object = scene.get(*id).filter(|object| !object.mesh.is_empty())?;
            Some(HollowTask {
                id: object.id,
                mesh: Arc::clone(&object.mesh),
                bvh: Arc::clone(&object.bvh),
                shape: object.transform.unturned(),
                settings: object.hollow.asking(asked),
            })
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
    report: &mut (dyn FnMut(usize, usize) + Send),
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
    report: &mut (dyn FnMut(usize, usize) + Send),
) -> HollowOutcome {
    let models = request.tasks.len();
    let mut shells: Vec<Shelled> = Vec::with_capacity(models);
    let asked = || cancel.load(Ordering::Relaxed);

    for (index, task) in request.tasks.iter().enumerate() {
        if asked() {
            return HollowOutcome::Cancelled;
        }
        report(index, models);

        let shell = match twin_of(&request.tasks[..index], task) {
            Some(twin) => shells[twin].shell.clone(),
            None => match shell_of(task, &request.fields, Cancel::when(&asked)) {
                Ok(shell) => shell,
                Err(VolumeError::Cancelled) => return HollowOutcome::Cancelled,
                Err(error) => return HollowOutcome::Failed(error.to_string()),
            },
        };
        shells.push(Shelled { id: task.id, shell });
    }

    report(models, models);
    HollowOutcome::Hollowed(shells)
}

/// An earlier task `task` is a copy of — one mesh, one shape, one set of numbers — whose
/// shell it shares rather than building the same field again.
fn twin_of(earlier: &[HollowTask], task: &HollowTask) -> Option<usize> {
    earlier.iter().position(|other| {
        Arc::ptr_eq(&other.mesh, &task.mesh)
            && other.shape == task.shape
            && other.settings == task.settings
    })
}

/// The shell of one model, cut from the field kept for it where there is one.
fn shell_of(
    task: &HollowTask,
    fields: &SharedFields,
    cancel: Cancel<'_>,
) -> Result<Shell, VolumeError> {
    let kept = lock(fields).get(&task.mesh, task.shape, &task.settings);
    let field = match kept {
        Some(field) => field,
        None => {
            lock(fields).make_way();
            let field = Arc::new(cavity_field(
                &task.mesh,
                &task.bvh,
                &task.settings,
                task.shape,
                cancel,
            )?);
            lock(fields).keep(&task.mesh, task.shape, &task.settings, Arc::clone(&field));
            field
        }
    };
    let hollowed = carve(&task.mesh, &field, &task.settings, cancel)?;
    Ok(Shell {
        mesh: Arc::new(hollowed.mesh),
        cavity: hollowed.cavity,
        cavity_mm3: hollowed.cavity_mm3,
        voxel_mm: hollowed.voxel_mm,
        coarsened: hollowed.coarsened,
        shape: task.shape,
        settings: task.settings.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A handle with no worker behind it, so that the counters can be driven by hand.
    fn job(done: usize, total: usize) -> HollowJob {
        let (_sender, reports) = mpsc::channel();
        HollowJob {
            reports,
            cancel: Arc::new(AtomicBool::new(false)),
            done,
            total,
        }
    }

    fn tetrahedron_task(id: u64, mesh: &Arc<Mesh>, thickness_mm: Scalar) -> HollowTask {
        HollowTask {
            id: ObjectId::for_test(id),
            mesh: Arc::clone(mesh),
            bvh: Arc::new(Bvh::build(mesh)),
            shape: Mat3::IDENTITY,
            settings: HollowSettings {
                thickness_mm,
                ..HollowSettings::default()
            },
        }
    }

    #[test]
    fn a_copy_shares_the_shell_of_its_original_and_a_different_wall_does_not() {
        let mesh = Arc::new(Mesh::new(
            vec![
                core_geometry::Vec3::new(0.0, 0.0, 0.0),
                core_geometry::Vec3::new(20.0, 0.0, 0.0),
                core_geometry::Vec3::new(0.0, 20.0, 0.0),
                core_geometry::Vec3::new(0.0, 0.0, 20.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        ));
        let request = HollowRequest {
            fields: SharedFields::default(),
            tasks: vec![
                tetrahedron_task(0, &mesh, 1.5),
                tetrahedron_task(1, &mesh, 1.5),
                tetrahedron_task(2, &mesh, 2.0),
            ],
        };
        let HollowOutcome::Hollowed(shells) =
            shell_each(&request, &AtomicBool::new(false), &mut |_, _| {})
        else {
            panic!("a 20 mm tetrahedron hollows at a 1.5 mm and a 2 mm wall");
        };

        let ids: Vec<ObjectId> = shells.iter().map(|shelled| shelled.id).collect();
        assert_eq!(
            ids,
            [0, 1, 2].map(ObjectId::for_test),
            "every object gets its own shell back, in order"
        );
        assert!(
            Arc::ptr_eq(&shells[0].shell.mesh, &shells[1].shell.mesh),
            "a copy at the same numbers is not built again"
        );
        assert!(
            !Arc::ptr_eq(&shells[0].shell.mesh, &shells[2].shell.mesh),
            "a wall of its own is a field of its own"
        );
    }

    #[test]
    fn a_model_asked_to_stop_stops_inside_its_field_and_keeps_nothing() {
        let mesh = Arc::new(Mesh::new(
            vec![
                core_geometry::Vec3::new(0.0, 0.0, 0.0),
                core_geometry::Vec3::new(20.0, 0.0, 0.0),
                core_geometry::Vec3::new(0.0, 20.0, 0.0),
                core_geometry::Vec3::new(0.0, 0.0, 20.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        ));
        let fields = SharedFields::default();
        let stopped = || true;

        let shelled = shell_of(
            &tetrahedron_task(0, &mesh, 1.5),
            &fields,
            Cancel::when(&stopped),
        );

        assert!(matches!(shelled, Err(VolumeError::Cancelled)));
        assert_eq!(
            lock(&fields).bytes(),
            0,
            "a field left unfinished is not kept"
        );
    }

    /// The bar runs rather than standing at nought for the whole of a run it cannot
    /// honestly take a share of.
    #[test]
    fn one_model_has_no_share_to_show() {
        let job = job(0, 1);
        assert_eq!(job.fraction(), None);
        assert_eq!(job.label(), "Hollowing...");
    }

    #[test]
    fn a_plate_of_models_counts_the_one_in_hand() {
        let job = job(1, 4);
        assert_eq!(job.fraction(), Some(0.25));
        assert_eq!(job.label(), "Hollowing model 2 of 4");
    }

    #[test]
    fn cancelling_shows_in_the_label() {
        let job = job(1, 4);
        job.cancel();
        assert_eq!(job.label(), "Stopping after this model");
    }
}
