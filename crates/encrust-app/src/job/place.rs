use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_supports::{Blocked, Region, generate_supports};
use printer_profiles::SupportProfile;

use crate::job::pipeline::{slice_mesh, thread_pool, worker_threads};
use crate::scene::{ObjectId, Scene};

/// How much of the run has to pass before the worker says so again. A layer is a
/// microsecond and a frame is sixteen thousand of them, so a message a layer would only
/// fill the channel.
const REPORT_STEP: f32 = 0.005;

/// One model to stand supports under, owned so that it can cross to the worker thread.
pub struct SupportTask {
    pub id: ObjectId,
    /// The model in its own space, and where it stands.
    pub mesh: Arc<Mesh>,
    pub transform: Transform,
    /// Contacts of the supports already under this model, in plate coordinates. They
    /// hold up what is near them, and the run never returns or moves them.
    pub seeds: Vec<Vec3>,
    /// The faces painted out of bounds, which the run puts nothing on or near.
    pub blocked: Region,
}

/// Everything one automatic placement run needs.
pub struct SupportRequest {
    pub tasks: Vec<SupportTask>,
    pub layer_height_mm: Scalar,
    pub profile: SupportProfile,
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
    pub contacts: Vec<Vec3>,
}

/// How an automatic placement run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum SupportOutcome {
    Placed(Vec<Placement>),
    Cancelled,
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

impl SupportOutcome {
    /// How many supports the run wants to add.
    pub fn count(&self) -> usize {
        match self {
            Self::Placed(placements) => placements
                .iter()
                .map(|placement| placement.contacts.len())
                .sum(),
            Self::Cancelled | Self::Failed(_) => 0,
        }
    }
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
    fraction: f32,
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
            fraction: 0.0,
        }
    }

    /// Takes everything the worker reported since the last frame, and returns the outcome
    /// on the frame the run ends.
    pub fn poll(&mut self) -> Option<SupportOutcome> {
        loop {
            match self.reports.try_recv() {
                Ok(Report::Progress(fraction)) => self.fraction = fraction,
                Ok(Report::Finished(outcome)) => return Some(outcome),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    return Some(SupportOutcome::Failed(
                        "the support thread stopped without finishing".to_owned(),
                    ));
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

    pub fn fraction(&self) -> f32 {
        self.fraction
    }

    pub fn label(&self) -> String {
        if self.is_cancelling() {
            return "Cancelling".to_owned();
        }
        format!("Placing supports, {:.0} %", self.fraction * 100.0)
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
pub fn tasks_of(scene: &Scene) -> Vec<SupportTask> {
    scene
        .targets()
        .filter(|object| !object.mesh.is_empty())
        .map(|object| SupportTask {
            id: object.id,
            mesh: Arc::clone(&object.mesh),
            transform: object.transform,
            seeds: object.supports.contacts(object.transform),
            blocked: object.supports.blocked().clone(),
        })
        .collect()
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

        let placed = if task.transform == Transform::default() {
            task.mesh.as_ref().clone()
        } else {
            transform_mesh(&task.mesh, task.transform)
        };
        let stack = match slice_mesh(&placed, request.layer_height_mm) {
            Ok(stack) => stack,
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

        let layers = stack.layers.len().max(1) as f32;
        let done = index as f32 / models;
        let mut last = done;
        let blocked = Blocked::new(&task.mesh, &task.blocked, task.transform);
        let contacts = generate_supports(
            &stack,
            request.layer_height_mm,
            &request.profile,
            &task.seeds,
            blocked.as_ref(),
            &mut |layer| {
                let fraction = done + layer as f32 / layers / models;
                if fraction - last >= REPORT_STEP {
                    last = fraction;
                    report(fraction);
                }
                !cancel.load(Ordering::Relaxed)
            },
        );

        if cancel.load(Ordering::Relaxed) {
            return SupportOutcome::Cancelled;
        }
        placements.push(Placement {
            id: task.id,
            transform: task.transform,
            contacts,
        });
    }

    report(1.0);
    SupportOutcome::Placed(placements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ImportSummary;
    use core_geometry::{Orientation, diagnose};

    /// An axis-aligned box spanning `min`..`max`, wound outwards.
    fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    fn summary(mesh: &Mesh) -> ImportSummary {
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(mesh),
        }
    }

    /// A scene holding one 10 mm cube, standing wherever `transform` puts it.
    fn scene_with_a_cube(transform: Transform) -> Scene {
        let mesh = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let mut scene = Scene::default();
        scene.insert(crate::scene::Imported::new(
            "cube".to_owned(),
            Arc::new(mesh.clone()),
            transform,
            summary(&mesh),
        ));
        scene
    }

    fn request(scene: &Scene) -> SupportRequest {
        SupportRequest {
            tasks: tasks_of(scene),
            layer_height_mm: 0.1,
            profile: SupportProfile::medium(),
        }
    }

    /// Runs a request to its end on this thread, with nothing cancelling it.
    fn placed(request: &SupportRequest) -> SupportOutcome {
        place(request, &AtomicBool::new(false), &mut |_| {})
    }

    #[test]
    fn a_cube_on_the_plate_needs_nothing_standing_under_it() {
        let scene = scene_with_a_cube(Transform::default());
        let outcome = placed(&request(&scene));

        assert_eq!(outcome.count(), 0);
        let SupportOutcome::Placed(placements) = outcome else {
            panic!("a sound cube places");
        };
        assert_eq!(placements.len(), 1, "the model was still looked at");
    }

    #[test]
    fn a_cube_lifted_off_the_plate_is_held_up_from_underneath() {
        let lift = Transform::from_translation(Vec3::new(0.0, 0.0, 10.0));
        let scene = scene_with_a_cube(lift);
        let outcome = placed(&request(&scene));

        assert!(outcome.count() > 0, "a floating cube is an island");
        let SupportOutcome::Placed(placements) = outcome else {
            panic!("a sound cube places");
        };
        assert_eq!(placements[0].transform, lift, "the placement it was cut in");
        for contact in &placements[0].contacts {
            assert!(
                (contact.z - 10.0).abs() < 0.2,
                "{contact} is not under the lifted cube"
            );
        }
    }

    #[test]
    fn a_hidden_model_is_not_worked_on() {
        let mut scene = scene_with_a_cube(Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)));
        scene.objects_mut()[0].visible = false;
        assert!(tasks_of(&scene).is_empty());
    }

    #[test]
    fn the_patch_painted_out_of_bounds_is_handed_over_with_the_model() {
        let mut scene = scene_with_a_cube(Transform::default());
        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let adjacency = core_geometry::Adjacency::of(&mesh);
        scene.objects_mut()[0]
            .supports
            .paint_face(&mesh, &adjacency, 0, 30.0, true, true);

        let tasks = tasks_of(&scene);
        assert!(
            !tasks[0].blocked.is_empty(),
            "the run has to know what it may not put a support on"
        );
    }

    #[test]
    fn the_supports_already_under_a_model_are_handed_over_as_seeds() {
        let lift = Transform::from_translation(Vec3::new(0.0, 0.0, 10.0));
        let mut scene = scene_with_a_cube(lift);
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(5.0, 5.0, 10.0), lift, 0);

        let tasks = tasks_of(&scene);
        assert_eq!(tasks.len(), 1);
        assert!(
            tasks[0].seeds[0].abs_diff_eq(Vec3::new(5.0, 5.0, 10.0), 1e-4),
            "a seed stands in plate coordinates, got {}",
            tasks[0].seeds[0]
        );
    }

    #[test]
    fn a_cancelled_run_hands_back_no_points() {
        let scene = scene_with_a_cube(Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)));
        let cancel = AtomicBool::new(true);
        assert_eq!(
            place(&request(&scene), &cancel, &mut |_| {}),
            SupportOutcome::Cancelled
        );
    }

    #[test]
    fn a_layer_height_of_zero_comes_back_as_a_failure() {
        let scene = scene_with_a_cube(Transform::default());
        let broken = SupportRequest {
            layer_height_mm: 0.0,
            ..request(&scene)
        };
        let SupportOutcome::Failed(message) = placed(&broken) else {
            panic!("a layer height of zero cannot cut a stack");
        };
        assert!(message.contains("cannot slice the model"), "got {message}");
    }

    #[test]
    fn a_run_reports_its_way_to_the_end() {
        let scene = scene_with_a_cube(Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)));
        let mut fractions = Vec::new();
        place(&request(&scene), &AtomicBool::new(false), &mut |fraction| {
            fractions.push(fraction);
        });

        assert_eq!(fractions.last(), Some(&1.0), "the run finishes at the end");
        assert!(
            fractions.windows(2).all(|pair| pair[0] <= pair[1]),
            "progress never runs backwards"
        );
    }

    #[test]
    fn a_worker_that_dies_without_an_outcome_is_a_failure() {
        let (sender, reports) = mpsc::channel();
        let mut job = SupportJob {
            reports,
            cancel: Arc::new(AtomicBool::new(false)),
            fraction: 0.0,
        };
        assert_eq!(job.poll(), None, "an empty channel means still running");

        drop(sender);
        let Some(SupportOutcome::Failed(message)) = job.poll() else {
            panic!("a dead worker must end the run");
        };
        assert!(message.contains("stopped"), "got {message}");
    }

    #[test]
    fn a_cancelling_job_says_so_instead_of_a_percentage() {
        let (_sender, reports) = mpsc::channel();
        let job = SupportJob {
            reports,
            cancel: Arc::new(AtomicBool::new(false)),
            fraction: 0.5,
        };
        assert_eq!(job.label(), "Placing supports, 50 %");

        job.cancel();
        assert!(job.is_cancelling());
        assert_eq!(job.label(), "Cancelling");
    }
}
