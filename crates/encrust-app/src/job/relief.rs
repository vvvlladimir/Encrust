use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use core_geometry::{Bvh, Heightmap, Mesh, Region, Transform, Vec3, transform_mesh};
use core_volume::{
    AngleMask, Cancel, Progress, ReliefBasis, ReliefSettings, Tiling, press_painted,
    press_painted_tiled, press_reusing, press_tiled_reusing,
};
use rayon::prelude::*;

use crate::i18n::tr;
use crate::job::{Worker, pooled};
use core_geometry::UvMap;

use crate::scene::{Mapped, Scene, SceneObject, Shaped, Taken};

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
    /// The only faces pressed, where the brush has painted any.
    pub painted: Option<Region>,
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
    /// What the press stood on, for the window to keep while another picture may follow;
    /// a painted press stands on none.
    pub basis: Option<Arc<ReliefBasis>>,
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
    // A run given up on stops inside its field; what it then answers is thrown away.
    Worker::spawn(
        move |stop, progress| pooled(|| run(&request, stop, progress), ReliefOutcome::Failed),
        || ReliefOutcome::Failed(tr!("job-thread-stopped-relief")),
    )
}

/// The faces of `object` a press is held to: what is painted, once anything is or while
/// the brush is in hand, when nothing painted means nowhere. `None` presses it whole.
pub fn painting(object: &SceneObject, brushing: bool) -> Option<&Region> {
    (brushing || !object.relief_painted.is_empty()).then_some(&object.relief_painted)
}

/// The models a run covers: everything on the plate being edited, or with `tiled` false
/// only what still carries the texture its file came with, less those `brushing` leaves
/// with nothing painted.
pub fn relief_tasks(scene: &Scene, tiled: bool, brushing: bool) -> Vec<ReliefTask> {
    scene
        .targets()
        .filter(|object| tiled || object.mapped.is_some())
        .filter(|object| painting(object, brushing).is_none_or(|painted| !painted.is_empty()))
        .map(|object| ReliefTask {
            taken: Taken::of(object),
            mesh: Arc::clone(&object.mesh),
            bvh: Arc::clone(&object.bvh),
            mapped: object.mapped.clone(),
            sizing: object.transform.sizing(),
            up: object.transform.rotation.inverse() * Vec3::Z,
            painted: painting(object, brushing).cloned(),
            basis: None,
        })
        .collect()
}

fn run(request: &ReliefRequest, stop: &AtomicBool, progress: &Progress) -> ReliefOutcome {
    let asked = || stop.load(Ordering::Relaxed);
    // Models are pressed side by side, and one count for several would jump between them.
    let cancel = match request.tasks.len() {
        1 => Cancel::when(&asked).reporting(progress),
        _ => Cancel::when(&asked),
    };
    let done: Result<Vec<Pressed>, _> = request
        .tasks
        .par_iter()
        .map(|task| {
            let settings = ReliefSettings {
                mask: AngleMask {
                    up: task.up,
                    ..request.settings.mask
                },
                ..request.settings
            };
            pressed(request, task, &settings, cancel)
        })
        .collect();

    match done {
        Ok(pressed) => ReliefOutcome::Pressed(pressed),
        Err(error) => ReliefOutcome::Failed(format!("{error:#}")),
    }
}

/// One model pressed: over a field where nothing is painted, or by moving the painted
/// faces alone, which keeps a basis for no other press.
fn pressed(
    request: &ReliefRequest,
    task: &ReliefTask,
    settings: &ReliefSettings,
    cancel: Cancel<'_>,
) -> anyhow::Result<Pressed> {
    let (mesh, bvh, uvs) = printed(task);
    let basis = task.basis.clone();
    let (relief, basis) = match (&request.source, &task.mapped, uvs, &task.painted) {
        (ReliefSource::Tiled { image, tiling }, _, _, Some(painted)) => (
            press_painted_tiled(&mesh, image, *tiling, settings, painted, cancel)?,
            None,
        ),
        (ReliefSource::Tiled { image, tiling }, _, _, None) => {
            let (relief, basis) =
                press_tiled_reusing(&mesh, &bvh, image, *tiling, settings, basis, cancel)?;
            (relief, Some(basis))
        }
        (ReliefSource::Mapped, Some(mapped), Some(uvs), Some(painted)) => (
            press_painted(&mesh, &uvs, &mapped.heights, settings, painted, cancel)?,
            None,
        ),
        (ReliefSource::Mapped, Some(mapped), Some(uvs), None) => {
            let heights = &mapped.heights;
            let (relief, basis) =
                press_reusing(&mesh, &bvh, &uvs, heights, settings, basis, cancel)?;
            (relief, Some(basis))
        }
        (ReliefSource::Mapped, ..) => anyhow::bail!(tr!("job-relief-no-texture")),
    };
    Ok(Pressed {
        taken: task.taken.clone(),
        shaped: Shaped::of(Arc::new(relief.mesh)),
        coarsened: relief.coarsened,
        basis,
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
