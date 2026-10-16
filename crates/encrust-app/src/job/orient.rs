use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_engine::{Trial, orient_for_print};
use core_geometry::{Mesh, Quat, Transform, Vec3, transform_mesh};
use core_plate::{OrientSettings, Printed};
use core_volume::ModelHollow;
use printer_profiles::{MaterialProfile, PrinterProfile, SupportProfile};
use rayon::prelude::*;

use crate::job::pipeline::{thread_pool, worker_threads};
use crate::scene::{ObjectId, Scene, SceneObject};

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
    /// The support profile this model is built to, which the search counts overhangs and
    /// contacts by and a proof prints it on.
    pub support: SupportProfile,
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
            support: SupportProfile::medium(),
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
    }

    /// `mesh` as it measures on the plate. A model nobody resized is not copied.
    fn sized(&self, mesh: &Arc<Mesh>) -> Arc<Mesh> {
        if self.sizing == Transform::default() {
            Arc::clone(mesh)
        } else {
            Arc::new(transform_mesh(mesh, self.sizing))
        }
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
    /// How far it is stood off the plate onto its supports when it is not on it,
    /// millimetres: its support profile's own lift.
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

/// An orientation run on its own thread.
///
/// Every candidate is measured over the whole mesh and the best few are sliced, which is
/// seconds on a detailed model and far too much for a frame.
#[derive(Debug)]
pub struct OrientJob {
    outcome: Receiver<OrientOutcome>,
}

impl OrientJob {
    /// Starts the run. The thread is detached: it ends on its own.
    pub fn spawn(request: OrientRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(run(&request));
        });
        Self { outcome }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<OrientOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(OrientOutcome::Failed(
                "the orientation thread stopped without finishing".to_owned(),
            )),
        }
    }
}

/// The models a run covers, each in its own space: one when `only` names it, everything
/// on the plate being edited otherwise. `support_of` is the profile each is built to.
pub fn orient_tasks(
    scene: &Scene,
    only: Option<ObjectId>,
    support_of: impl Fn(&SceneObject) -> SupportProfile,
) -> Vec<OrientTask> {
    scene
        .targets()
        .filter(|object| only.is_none_or(|id| object.id == id))
        .map(|object| OrientTask {
            id: object.id,
            mesh: Arc::clone(&object.mesh),
            shell: object.hollow.shell().map(Arc::clone),
            cuts: object.hollow.cut_bodies().map(Arc::clone),
            vents: vents_of(&object.hollow),
            sizing: object.transform.sizing(),
            down_now: object.transform.rotation.inverse() * Vec3::NEG_Z,
            support: lifted(support_of(object), object.lift_mm),
        })
        .collect()
}

/// `support` standing a model `lift_mm` clear of the plate where the user asked for that.
fn lifted(support: SupportProfile, lift_mm: Option<f32>) -> SupportProfile {
    SupportProfile {
        z_lift_mm: lift_mm.unwrap_or(support.z_lift_mm),
        ..support
    }
}

/// The good ways to stand one model, best first: proven by printing where there is a
/// machine to print with, and the search's own answer otherwise.
fn choose(
    printed: &Printed,
    settings: &OrientSettings,
    support: &SupportProfile,
    proof: Option<&Proof>,
) -> Result<Vec<Pose>, String> {
    let trial = proof.map(|proof| Trial {
        printer: &proof.printer,
        material: &proof.material,
        support,
        raster_window: worker_threads(),
    });
    let chosen =
        orient_for_print(printed, settings, trial.as_ref()).map_err(|error| error.to_string())?;
    Ok(offered(chosen.iter().map(|choice| {
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
    })))
}

/// The first of `ranked` and every other one offered beside it.
fn offered(ranked: impl Iterator<Item = (Pose, bool)>) -> Vec<Pose> {
    ranked
        .enumerate()
        .filter(|(at, (_, offered))| *at == 0 || *offered)
        .map(|(_, (pose, _))| pose)
        .collect()
}

/// The way out of every hole and of both ends of every channel cut into a model.
fn vents_of(hollow: &ModelHollow) -> Vec<Vec3> {
    let (holes, channels) = hollow.cut();
    let ends = channels.iter().flat_map(|channel| {
        let points = &channel.points;
        let first = points
            .first()
            .zip(points.get(1))
            .map(|(end, next)| *end - *next);
        let last = points
            .last()
            .zip(points.iter().rev().nth(1))
            .map(|(end, next)| *end - *next);
        first.into_iter().chain(last)
    });
    holes.iter().map(|hole| -hole.axis).chain(ends).collect()
}

fn run(request: &OrientRequest) -> OrientOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return OrientOutcome::Failed(error.to_string()),
    };

    pool.install(|| {
        let turned: Result<Vec<Turn>, String> = request
            .tasks
            .par_iter()
            .map(|task| {
                let settings = OrientSettings {
                    down_now: task.down_now,
                    max_overhang_deg: task.support.max_overhang_deg,
                    contact_spacing_mm: task.support.contact_spacing_mm,
                    lift_mm: task.support.lift_mm(),
                    ..request.settings
                };
                let outer = task.sized(&task.mesh);
                let shell = task.shell.as_ref().map(|shell| task.sized(shell));
                let cuts = task.cuts.as_ref().map(|cuts| task.sized(cuts));
                let sizing = task.sizing.linear();
                let vents: Vec<Vec3> = task.vents.iter().map(|out| sizing * *out).collect();
                let printed = Printed {
                    outer: &outer,
                    shell: shell.as_deref(),
                    cuts: cuts.as_deref(),
                    vents: &vents,
                };
                let poses = choose(&printed, &settings, &task.support, request.proof.as_ref())?;
                if poses.is_empty() {
                    return Err("the search found no orientation".to_owned());
                }
                Ok(Turn {
                    id: task.id,
                    poses,
                    lift_mm: task.support.z_lift_mm,
                })
            })
            .collect();

        match turned {
            Ok(turns) => OrientOutcome::Turned(turns),
            Err(message) => OrientOutcome::Failed(message),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
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

    #[test]
    fn a_run_reports_a_turn_for_every_model_it_was_given() {
        let request = OrientRequest {
            tasks: vec![
                OrientTask::solid(
                    ObjectId::for_test(1),
                    Arc::new(cuboid(Vec3::new(40.0, 40.0, 4.0))),
                ),
                OrientTask::solid(ObjectId::for_test(2), Arc::new(cuboid(Vec3::splat(10.0)))),
            ],
            settings: OrientSettings::default(),
            proof: None,
        };

        let OrientOutcome::Turned(turns) = run(&request) else {
            panic!("two sound models can be oriented");
        };
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].id, ObjectId::for_test(1));
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

        let tasks = orient_tasks(&scene, None, |_| SupportProfile::medium());
        let judged = tasks[0].shell.as_ref().expect("the task carries the shell");
        assert!(Arc::ptr_eq(judged, &shell));
    }

    #[test]
    fn a_model_with_no_geometry_fails_the_run() {
        let request = OrientRequest {
            tasks: vec![OrientTask::solid(
                ObjectId::for_test(1),
                Arc::new(Mesh::default()),
            )],
            settings: OrientSettings::default(),
            proof: None,
        };

        let OrientOutcome::Failed(message) = run(&request) else {
            panic!("an empty mesh has no orientation");
        };
        assert!(message.contains("orientation"), "got {message}");
    }
}
