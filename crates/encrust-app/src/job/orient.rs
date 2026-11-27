use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, LazyLock};

use core_document::Body;
use core_engine::{KeepClear, SupportPlan, Trial, kept_poses_folder, orient_for_print_kept};
use core_geometry::{Mesh, Quat, Region, Transform, Vec3};
use core_plate::{OrientSettings, Phase, Printed, Watch};
#[cfg(test)]
use printer_profiles::SupportProfile;
use printer_profiles::{MaterialProfile, PrinterProfile};
use rayon::prelude::*;

use crate::i18n::tr;
use crate::job::pipeline::worker_threads;
use crate::job::{Worker, pooled};
use crate::scene::{ObjectId, Scene, SceneObject};

mod seen;

use seen::Seen;

/// Where searches are kept, shared with the command line built beside the window (ADR 0466).
static KEPT_POSES: LazyLock<Option<PathBuf>> =
    LazyLock::new(|| kept_poses_folder(agent_link::cache_dir().as_deref()));

/// One model to find an orientation for, owned so that it can cross to the worker thread.
#[derive(Clone)]
pub struct OrientTask {
    pub id: ObjectId,
    /// The model in its own space. Orientation is measured there, so where it currently
    /// stands does not enter into it.
    pub mesh: Arc<Mesh>,
    /// What will actually be printed where the model has been hollowed, and the holes and
    /// channels cut into it, wound inward: what it pulls and seals is read off these.
    pub shell: Option<Arc<Mesh>>,
    pub cuts: Option<Arc<Mesh>>,
    /// The way out of each drain hole and each end of a channel, in the model's own space.
    pub vents: Vec<Vec3>,
    /// The model's scale and stretch with its turn taken out. Overhangs, pulls and hole
    /// sizes are measured in plate millimetres, so the meshes are judged through it.
    pub sizing: Transform,
    /// The way down the model stands now, in the sized model's space, which a tilt is
    /// measured from.
    pub down_now: Vec3,
    /// What this model's supports are built to, as a run in the window builds them: the
    /// search counts overhangs and contacts by its placing profile, and a proof prints it.
    pub support: SupportPlan,
    /// The faces of `mesh` painted as what shows, which a clean surface keeps supports off.
    pub shown: Region,
    /// Places no support should touch, in the sized mesh's frame, which ranks poses first.
    pub keep_clear: Option<KeepClear>,
}

impl OrientTask {
    /// A model as its file has it: solid, at its own size, standing the way it was drawn.
    #[cfg(test)]
    pub fn solid(id: ObjectId, mesh: Arc<Mesh>) -> Self {
        Self {
            id,
            mesh,
            shell: None,
            cuts: None,
            vents: Vec::new(),
            sizing: Transform::default(),
            down_now: Vec3::NEG_Z,
            support: SupportPlan::of(
                SupportProfile::shipped("medium").expect("the medium preset ships"),
            ),
            shown: Region::default(),
            keep_clear: None,
        }
    }

    /// Whether `other` asks the search the same of the same model, however it now stands.
    pub fn asks_as(&self, other: &Self) -> bool {
        let same = |one: &Option<Arc<Mesh>>, two: &Option<Arc<Mesh>>| match (one, two) {
            (Some(one), Some(two)) => Arc::ptr_eq(one, two),
            (one, two) => one.is_none() && two.is_none(),
        };
        self.id == other.id
            && Arc::ptr_eq(&self.mesh, &other.mesh)
            && same(&self.shell, &other.shell)
            && same(&self.cuts, &other.cuts)
            && self.vents == other.vents
            && self.sizing == other.sizing
            && self.support == other.support
            && self.shown == other.shown
            && self.keep_clear == other.keep_clear
    }
}

/// Everything one orientation run needs.
pub struct OrientRequest {
    pub tasks: Vec<OrientTask>,
    pub settings: OrientSettings,
    /// What the search's best few are printed with to choose between them, or `None`
    /// to take the search at its word, as a window with no printer chosen has to.
    pub proof: Option<Proof>,
}

/// The machine and the resin a pose is printed with to be judged.
#[derive(Clone, PartialEq)]
pub struct Proof {
    pub printer: PrinterProfile,
    pub material: MaterialProfile,
}

/// How a model should be turned, and the other good ways to stand it, best first.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub id: ObjectId,
    /// Never empty: the first is the choice, the rest are offered in turn beside it.
    pub poses: Vec<Pose>,
    /// Every way the search ranked, best first, offered or not: what an edit's pick counts.
    pub ranked: Vec<Pose>,
    /// How far it is stood off the plate onto its supports when it is not on it,
    /// millimetres: the lift the user gave the model, or its support profile's.
    pub lift_mm: f32,
}

/// One way to stand a model, and what a proof read it to cost, where one printed it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub rotation: Quat,
    /// Whether it stands straight on the plate, supported only where it hangs over it.
    pub on_plate: bool,
    pub costs: Option<Costs>,
}

/// What a pose costs as a proof printed it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Costs {
    pub supports_ml: f32,
    pub pull_n: f64,
    /// How tall the stack stands, model and supports, millimetres.
    pub stack_mm: f32,
}

/// How an orientation run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum OrientOutcome {
    Turned(Vec<Turn>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// An orientation run on its own thread. Dropping the job asks the run to stop.
///
/// Every candidate is measured over the whole mesh and the best few are sliced, which is
/// seconds on a detailed model and far too much for a frame.
#[derive(Debug)]
pub struct OrientJob {
    worker: Worker<OrientOutcome>,
    seen: Arc<Seen>,
}

impl OrientJob {
    /// Starts the run on its own thread.
    pub fn spawn(request: OrientRequest) -> Self {
        let seen = Arc::new(Seen::new(request.tasks.len()));
        let told = Arc::clone(&seen);
        let worker = Worker::spawn(
            move |stop, _| pooled(|| run(&request, (stop, &told)), OrientOutcome::Failed),
            || OrientOutcome::Failed(tr!("job-thread-stopped-orientation")),
        );
        Self { worker, seen }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<OrientOutcome> {
        self.worker.poll()
    }

    /// What the run is at in words and the share of it done, or `None` before any model
    /// has begun.
    pub fn counted(&self) -> Option<(String, f32)> {
        let label = match self.seen.phase()? {
            Phase::Looking => tr!("orient-looking"),
            Phase::Proving => tr!("orient-proving"),
        };
        Some((label, self.seen.share()?))
    }
}

/// The models a run covers, each in its own space: one when `only` names it, everything
/// on the plate being edited otherwise. `plan_of` is what each one's supports are built to.
pub fn orient_tasks(
    scene: &Scene,
    only: Option<ObjectId>,
    plan_of: impl Fn(&SceneObject) -> SupportPlan,
) -> Vec<OrientTask> {
    scene
        .targets()
        .filter(|object| only.is_none_or(|id| object.id == id))
        .map(|object| OrientTask::of(object, plan_of(object)))
        .collect()
}

impl OrientTask {
    /// `object` as it stands, judged on supports built to `plan`.
    pub fn of(object: &SceneObject, plan: SupportPlan) -> Self {
        Self {
            id: object.id,
            mesh: Arc::clone(&object.mesh),
            shell: object.hollow.shell().map(Arc::clone),
            cuts: object.hollow.cut_bodies().map(Arc::clone),
            vents: core_document::vents_of(&object.hollow),
            sizing: object.transform.sizing(),
            down_now: object.transform.rotation.inverse() * Vec3::NEG_Z,
            support: core_document::lifted(plan, object.lift_mm),
            shown: object.shown_painted.clone(),
            keep_clear: None,
        }
    }
}

/// The good ways to stand one model, best first: proven by printing where there is a
/// machine to print with, and the search's own answer otherwise.
fn choose(
    printed: &Printed,
    settings: &OrientSettings,
    (support, keep_clear): (&SupportPlan, Option<&KeepClear>),
    proof: Option<&Proof>,
    watch: Watch<'_>,
) -> Result<Vec<(Pose, bool)>, String> {
    let trial = proof.map(|proof| Trial {
        printer: &proof.printer,
        material: &proof.material,
        support,
        raster_window: worker_threads(),
        keep_clear,
    });
    let kept = KEPT_POSES.as_deref();
    let chosen = orient_for_print_kept(kept, printed, settings, trial.as_ref(), watch)
        .map_err(|error| error.to_string())?;
    Ok(chosen
        .iter()
        .map(|choice| {
            let pose = Pose {
                rotation: choice.oriented.rotation,
                on_plate: choice.oriented.on_plate,
                costs: choice.printout.map(|printout| Costs {
                    supports_ml: printout.supports_ml,
                    pull_n: printout.pull_n,
                    stack_mm: printout.stack_mm,
                }),
            };
            (pose, choice.ruling.is_offered())
        })
        .collect())
}

/// The first of `ranked` and every other one offered beside it.
fn offered(ranked: &[(Pose, bool)]) -> Vec<Pose> {
    ranked
        .iter()
        .enumerate()
        .filter(|(at, (_, offered))| *at == 0 || *offered)
        .map(|(_, (pose, _))| *pose)
        .collect()
}

fn run(request: &OrientRequest, (stop, seen): (&AtomicBool, &Seen)) -> OrientOutcome {
    let turned: Result<Vec<Turn>, String> = request
        .tasks
        .par_iter()
        .enumerate()
        .map(|(at, task)| {
            let told = |phase, done, total| {
                seen.told(at, phase, done, total);
                !stop.load(Relaxed)
            };
            turn_of(task, request, Watch::by(&told))
        })
        .collect();

    match turned {
        Ok(turns) => OrientOutcome::Turned(turns),
        Err(message) => OrientOutcome::Failed(message),
    }
}

/// How `task`'s model should be turned, judged as the shell and cuts it will print.
fn turn_of(task: &OrientTask, request: &OrientRequest, watch: Watch<'_>) -> Result<Turn, String> {
    let placing = task.support.placing();
    let settings = core_document::placed_by(
        OrientSettings {
            down_now: task.down_now,
            ..request.settings
        },
        &task.support,
    );
    let body = Body {
        outer: Arc::clone(&task.mesh),
        shell: task.shell.clone(),
        cuts: task.cuts.clone(),
        vents: task.vents.clone(),
    }
    .sized(task.sizing);
    let printed = body.printed(&task.shown);
    let ranked = choose(
        &printed,
        &settings,
        (&task.support, task.keep_clear.as_ref()),
        request.proof.as_ref(),
        watch,
    )?;
    if ranked.is_empty() {
        return Err(tr!("job-orient-none"));
    }
    Ok(Turn {
        id: task.id,
        poses: offered(&ranked),
        ranked: ranked.into_iter().map(|(pose, _)| pose).collect(),
        lift_mm: placing.z_lift_mm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        core_geometry::cuboid(Vec3::ZERO, size)
    }

    /// `request` run with nobody watching.
    fn quietly(request: &OrientRequest) -> OrientOutcome {
        let seen = Seen::new(request.tasks.len());
        run(request, (&AtomicBool::new(false), &seen))
    }

    #[test]
    fn a_run_reports_a_turn_for_every_model_it_was_given() {
        let request = OrientRequest {
            tasks: vec![
                OrientTask::solid(ObjectId(1), Arc::new(cuboid(Vec3::new(40.0, 40.0, 4.0)))),
                OrientTask::solid(ObjectId(2), Arc::new(cuboid(Vec3::splat(10.0)))),
            ],
            settings: OrientSettings::default(),
            proof: None,
        };

        let OrientOutcome::Turned(turns) = quietly(&request) else {
            panic!("two sound models can be oriented");
        };
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].id, ObjectId(1));
    }

    #[test]
    fn a_hollowed_model_is_judged_as_the_shell_it_prints() {
        use crate::scene::{ImportSummary, Imported};
        use core_geometry::{Mat3, Orientation, diagnose};
        use core_volume::{HollowSettings, Shell};

        let mesh = cuboid(Vec3::splat(10.0));
        let mut scene = Scene::default();
        scene.insert(Imported::new(
            "cube".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        let shell = Arc::new(mesh);
        scene.objects_mut()[0].hollow.take(Shell {
            mesh: Arc::clone(&shell),
            cavity: 0..0,
            cavity_mm3: 0.0,
            voxel_mm: 0.5,
            coarsened: false,
            shape: Mat3::IDENTITY,
            settings: HollowSettings::default(),
        });

        let tasks = orient_tasks(&scene, None, |_| {
            SupportPlan::of(SupportProfile::shipped("medium").expect("the medium preset ships"))
        });
        let judged = tasks[0].shell.as_ref().expect("the task carries the shell");
        assert!(Arc::ptr_eq(judged, &shell));
    }

    #[test]
    fn a_model_painted_otherwise_is_not_asked_the_same() {
        let mesh = Arc::new(cuboid(Vec3::splat(10.0)));
        let plain = OrientTask::solid(ObjectId(1), Arc::clone(&mesh));
        let mut painted = OrientTask::solid(ObjectId(1), mesh);
        painted.shown.set(0, true);
        assert!(plain.asks_as(&plain), "the same task asks the same");
        assert!(
            !plain.asks_as(&painted),
            "paint changes what a clean surface avoids"
        );
    }

    #[test]
    fn a_model_with_no_geometry_fails_the_run() {
        let request = OrientRequest {
            tasks: vec![OrientTask::solid(ObjectId(1), Arc::new(Mesh::default()))],
            settings: OrientSettings::default(),
            proof: None,
        };

        let OrientOutcome::Failed(message) = quietly(&request) else {
            panic!("an empty mesh has no orientation");
        };
        assert!(message.contains("orientation"), "got {message}");
    }

    #[test]
    fn a_run_tells_how_far_each_model_has_got_and_stops_when_asked() {
        let request = OrientRequest {
            tasks: vec![OrientTask::solid(
                ObjectId(1),
                Arc::new(cuboid(Vec3::new(40.0, 40.0, 4.0))),
            )],
            settings: OrientSettings::default(),
            proof: None,
        };
        let seen = Seen::new(1);
        run(&request, (&AtomicBool::new(false), &seen));
        assert_eq!(seen.phase(), Some(Phase::Looking), "no machine, no proof");
        let share = seen.share().expect("the shortlist was cut");
        assert!(
            (share - 0.12).abs() < 1e-6,
            "the search ends at 12 %, got {share}"
        );

        let stopped = run(&request, (&AtomicBool::new(true), &Seen::new(1)));
        assert!(matches!(stopped, OrientOutcome::Failed(_)));
    }
}
