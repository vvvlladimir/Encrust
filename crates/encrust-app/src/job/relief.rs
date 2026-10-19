use std::sync::Arc;

use core_geometry::{Bvh, Heightmap, Mesh, Transform, Vec3, transform_mesh};
use core_volume::{
    AngleMask, ReliefBasis, ReliefSettings, Tiling, press_reusing, press_tiled_reusing,
};
use rayon::prelude::*;

use crate::i18n::tr;
use crate::job::{Worker, pooled};
use core_geometry::UvMap;

use crate::scene::{Mapped, Scene, Shaped, Taken};

/// One model to press a texture into, owned so that it can cross to the worker thread.
pub struct ReliefTask {
    /// The model as the run took it, so that the press lands only on that model.
    pub taken: Taken,
    /// The model and its hierarchy, both in its own space, which is where the map stands.
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    /// The texture the model's file came with, if any; a tiled picture needs none.
    pub mapped: Option<Arc<Mapped>>,
    /// The model's placement but for its turn (`Transform::sizing`). The relief is pressed
    /// at the size the model prints at, so the mesh is taken through this first; see ADR 0122.
    pub sizing: Transform,
    /// The plate's up in the model's own space, which is where its faces are held.
    pub up: Vec3,
    /// The basis a run before this one kept for this very mesh, if any; the press checks
    /// that it still fits and builds a new one where it does not.
    pub basis: Option<Arc<ReliefBasis>>,
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
#[derive(Debug, Clone)]
pub struct Pressed {
    pub taken: Taken,
    pub shaped: Shaped,
    /// Whether the memory budget made the lattice coarser than precision asked for.
    pub coarsened: bool,
    /// What the press stood on, for the window to keep while another picture may follow.
    pub basis: Arc<ReliefBasis>,
}

/// How a relief run ended.
#[derive(Debug, Clone)]
pub enum ReliefOutcome {
    Pressed(Vec<Pressed>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// A relief run on its own thread.
///
/// A relief is a field over the whole model with a nearest-point query per lattice point,
/// which is seconds of work on a real part, the way hollowing is.
pub type ReliefJob = Worker<ReliefOutcome>;

/// Starts the run. The thread is detached: it ends on its own.
pub fn spawn_relief(request: ReliefRequest) -> ReliefJob {
    // A press cannot be given up on part way; a run given up on is thrown away as it ends.
    Worker::spawn(
        move |_| pooled(|| run(&request), ReliefOutcome::Failed),
        || ReliefOutcome::Failed(tr!("job-thread-stopped-relief")),
    )
}

/// The models a run covers: everything on the plate being edited, or with `tiled` false
/// only what still carries the texture its file came with.
pub fn relief_tasks(scene: &Scene, tiled: bool) -> Vec<ReliefTask> {
    scene
        .targets()
        .filter(|object| tiled || object.mapped.is_some())
        .map(|object| ReliefTask {
            taken: Taken::of(object),
            mesh: Arc::clone(&object.mesh),
            bvh: Arc::clone(&object.bvh),
            mapped: object.mapped.clone(),
            sizing: object.transform.sizing(),
            up: object.transform.rotation.inverse() * Vec3::Z,
            basis: None,
        })
        .collect()
}

fn run(request: &ReliefRequest) -> ReliefOutcome {
    let pressed: Result<Vec<Pressed>, _> = request
        .tasks
        .par_iter()
        .map(|task| {
            let (mesh, bvh, uvs) = printed(task);
            let settings = ReliefSettings {
                mask: AngleMask {
                    up: task.up,
                    ..request.settings.mask
                },
                ..request.settings
            };
            let relief = match (&request.source, &task.mapped, uvs) {
                (ReliefSource::Tiled { image, tiling }, _, _) => {
                    let basis = task.basis.clone();
                    press_tiled_reusing(&mesh, &bvh, image, *tiling, &settings, basis)
                }
                (ReliefSource::Mapped, Some(mapped), Some(uvs)) => {
                    let basis = task.basis.clone();
                    press_reusing(&mesh, &bvh, &uvs, &mapped.heights, &settings, basis)
                }
                (ReliefSource::Mapped, ..) => {
                    return Err(anyhow::anyhow!(tr!("job-relief-no-texture")));
                }
            };
            relief
                .map_err(anyhow::Error::new)
                .map(|(relief, basis)| Pressed {
                    taken: task.taken.clone(),
                    shaped: Shaped::of(Arc::new(relief.mesh)),
                    coarsened: relief.coarsened,
                    basis,
                })
        })
        .collect();

    match pressed {
        Ok(pressed) => ReliefOutcome::Pressed(pressed),
        Err(error) => ReliefOutcome::Failed(format!("{error:#}")),
    }
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
