use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Bvh, Heightmap, Mesh, Scalar, Transform, transform_mesh};
use core_volume::{ReliefSettings, Tiling, press, press_tiled};
use rayon::prelude::*;

use crate::job::pipeline::{thread_pool, worker_threads};
use core_geometry::UvMap;

use crate::scene::{Mapped, ObjectId, Scene, Shaped};

/// One model to press a texture into, owned so that it can cross to the worker thread.
pub struct ReliefTask {
    pub id: ObjectId,
    /// The model and its hierarchy, both in its own space, which is where the map stands.
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    /// The texture the model's file came with, if any; a tiled picture needs none.
    pub mapped: Option<Arc<Mapped>>,
    /// The model's placement but for its turn (`Transform::sizing`). The relief is pressed
    /// at the size the model prints at, so the mesh is taken through this first; see ADR 0122.
    pub sizing: Transform,
}

/// Everything one relief run needs.
pub struct ReliefRequest {
    pub tasks: Vec<ReliefTask>,
    pub settings: ReliefSettings,
    pub source: ReliefSource,
}

/// What the heights are read from.
pub enum ReliefSource {
    /// Each model's own texture, through the map its file came with.
    Mapped,
    /// One picture laid over every model as tiles, mapped or not.
    Tiled {
        image: Arc<Heightmap>,
        tiling: Tiling,
    },
}

/// One model with its texture pressed in, in its own space.
#[derive(Debug, Clone, PartialEq)]
pub struct Pressed {
    pub id: ObjectId,
    pub shaped: Shaped,
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

/// The models a run covers: everything on the plate being edited, or with `tiled` false
/// only what still carries the texture its file came with.
pub fn relief_tasks(scene: &Scene, tiled: bool) -> Vec<ReliefTask> {
    scene
        .targets()
        .filter(|object| tiled || object.mapped.is_some())
        .map(|object| ReliefTask {
            id: object.id,
            mesh: Arc::clone(&object.mesh),
            bvh: Arc::clone(&object.bvh),
            mapped: object.mapped.clone(),
            sizing: object.transform.sizing(),
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
                let relief = match (&request.source, &task.mapped, uvs) {
                    (ReliefSource::Tiled { image, tiling }, _, _) => {
                        press_tiled(&mesh, &bvh, image, *tiling, &request.settings)
                    }
                    (ReliefSource::Mapped, Some(mapped), Some(uvs)) => {
                        press(&mesh, &bvh, &uvs, &mapped.heights, &request.settings)
                    }
                    (ReliefSource::Mapped, ..) => {
                        return Err(anyhow::anyhow!("a model has no texture to press"));
                    }
                };
                relief.map_err(anyhow::Error::new).map(|relief| Pressed {
                    id: task.id,
                    shaped: Shaped::of(Arc::new(relief.mesh)),
                    voxel_mm: relief.voxel_mm,
                    coarsened: relief.coarsened,
                })
            })
            .collect();

        match pressed {
            Ok(pressed) => ReliefOutcome::Pressed(pressed),
            Err(error) => ReliefOutcome::Failed(format!("{error:#}")),
        }
    })
}

/// The model at the size it prints at, with its map still over it.
///
/// The lattice, the memory budget and the depth are all plate millimetres, and a file's
/// own unit is not: a statue drawn 1.33 units tall and printed at 140 mm would otherwise be
/// pressed on a lattice coarser than the model. A mirroring scale turns every face inside
/// out, so the map's corners follow the winding.
fn printed(task: &ReliefTask) -> (Arc<Mesh>, Arc<Bvh>, Option<UvMap>) {
    if task.sizing == Transform::default() {
        return (
            Arc::clone(&task.mesh),
            Arc::clone(&task.bvh),
            task.mapped.as_ref().map(|mapped| mapped.uvs.clone()),
        );
    }

    let mesh = transform_mesh(&task.mesh, task.sizing);
    let uvs = task.mapped.as_ref().map(|mapped| {
        if task.sizing.is_mirrored() {
            let all: Vec<u32> = (0..mesh.faces.len() as u32).collect();
            mapped.uvs.flipping(&all)
        } else {
            mapped.uvs.clone()
        }
    });
    let bvh = Arc::new(Bvh::build(&mesh));
    (Arc::new(mesh), bvh, uvs)
}
