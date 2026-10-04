use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_volume::{ReliefSettings, press};
use rayon::prelude::*;

use crate::job::pipeline::{thread_pool, worker_threads};
use core_geometry::UvMap;

use crate::scene::{Mapped, ObjectId, Scene};

/// One model to press a texture into, owned so that it can cross to the worker thread.
pub struct ReliefTask {
    pub id: ObjectId,
    /// The model and its hierarchy, both in its own space, which is where the map stands.
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    pub mapped: Arc<Mapped>,
    /// What the model is scaled by where it stands. The relief is pressed at the size the
    /// model prints at, so the mesh is scaled by this first; see ADR 0122.
    pub scale: Vec3,
}

/// Everything one relief run needs.
pub struct ReliefRequest {
    pub tasks: Vec<ReliefTask>,
    pub settings: ReliefSettings,
}

/// One model with its texture pressed in, in its own space.
#[derive(Debug, Clone, PartialEq)]
pub struct Pressed {
    pub id: ObjectId,
    pub mesh: Arc<Mesh>,
    /// The lattice it came out on, in the model's own millimetres, and whether the memory
    /// budget made that coarser than precision asked for.
    pub voxel_mm: Scalar,
    pub coarsened: bool,
}

/// How a relief run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum ReliefOutcome {
    Pressed(Vec<Pressed>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// A relief run on its own thread.
///
/// A relief is a field over the whole model with a nearest-point query per lattice point,
/// which is seconds of work on a real part, the way hollowing is.
#[derive(Debug)]
pub struct ReliefJob {
    outcome: Receiver<ReliefOutcome>,
}

impl ReliefJob {
    /// Starts the run. The thread is detached: it ends on its own.
    pub fn spawn(request: ReliefRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(run(&request));
        });
        Self { outcome }
    }

    /// Returns the outcome on the frame the run ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<ReliefOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(ReliefOutcome::Failed(
                "the relief thread stopped without finishing".to_owned(),
            )),
        }
    }
}

/// The models a run covers: everything on the plate being edited that still carries the
/// texture its file came with.
pub fn relief_tasks(scene: &Scene) -> Vec<ReliefTask> {
    scene
        .targets()
        .filter_map(|object| {
            Some(ReliefTask {
                id: object.id,
                mesh: Arc::clone(&object.mesh),
                bvh: Arc::clone(&object.bvh),
                mapped: Arc::clone(object.mapped.as_ref()?),
                scale: object.transform.scale,
            })
        })
        .collect()
}

fn run(request: &ReliefRequest) -> ReliefOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return ReliefOutcome::Failed(error.to_string()),
    };

    pool.install(|| {
        let pressed: Result<Vec<Pressed>, _> = request
            .tasks
            .par_iter()
            .map(|task| {
                let (mesh, bvh, uvs) = printed(task);
                press(&mesh, &bvh, &uvs, &task.mapped.heights, &request.settings).map(|relief| {
                    Pressed {
                        id: task.id,
                        mesh: Arc::new(relief.mesh),
                        voxel_mm: relief.voxel_mm,
                        coarsened: relief.coarsened,
                    }
                })
            })
            .collect();

        match pressed {
            Ok(pressed) => ReliefOutcome::Pressed(pressed),
            Err(error) => ReliefOutcome::Failed(error.to_string()),
        }
    })
}

/// The model at the size it prints at, with its map still over it.
///
/// The lattice, the memory budget and the depth are all plate millimetres, and a file's
/// own unit is not: a statue drawn 1.33 units tall and printed at 140 mm would otherwise be
/// pressed on a lattice coarser than the model. A mirroring scale turns every face inside
/// out, so the map's corners follow the winding.
fn printed(task: &ReliefTask) -> (Arc<Mesh>, Arc<Bvh>, UvMap) {
    if task.scale.abs_diff_eq(Vec3::ONE, Scalar::EPSILON) {
        return (
            Arc::clone(&task.mesh),
            Arc::clone(&task.bvh),
            task.mapped.uvs.clone(),
        );
    }

    let sized = Transform {
        scale: task.scale,
        ..Transform::default()
    };
    let mesh = transform_mesh(&task.mesh, sized);
    let uvs = if task.scale.x * task.scale.y * task.scale.z < 0.0 {
        let all: Vec<u32> = (0..mesh.faces.len() as u32).collect();
        task.mapped.uvs.flipping(&all)
    } else {
        task.mapped.uvs.clone()
    };
    let bvh = Arc::new(Bvh::build(&mesh));
    (Arc::new(mesh), bvh, uvs)
}
