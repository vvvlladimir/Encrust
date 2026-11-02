use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::Relaxed};

use core_engine::{DrainSearch, Drained, suggest_drains_heard};
use core_geometry::{Mesh, Scalar, Transform, transform_mesh};
use core_supports::Trapped;
use core_volume::{DrainHole, HoleSize};

use super::trapped::{flattened, into_model};
use crate::i18n::tr;
use crate::job::{Highest, Worker, pooled};
use crate::scene::{ObjectId, SceneObject};

/// Nearer the plate than this a model's lowest point rests on it, millimetres: no hole may
/// open where the first layers spread (ADR 0396).
const ON_PLATE_MM: Scalar = 1e-2;

/// One model the drain search looks through, owned so it can cross to the worker.
pub struct DrainsTask {
    pub id: ObjectId,
    /// What prints, in the model's own space: its shell once hollowed.
    pub mesh: Arc<Mesh>,
    /// Its holes and channels already cut, wound inward, in the same space.
    pub cuts: Option<Arc<Mesh>>,
    pub transform: Transform,
    /// The hole as the Drain tool drills it, measured on the plate.
    pub size: HoleSize,
    /// The wall the cavity was cut to and its lattice, millimetres; `None` while solid.
    pub wall: Option<(Scalar, Scalar)>,
}

impl DrainsTask {
    /// `object` as it stands, drilled with `size`.
    pub fn of(object: &SceneObject, size: HoleSize) -> Self {
        let hollow = &object.hollow;
        let wall = hollow.asked().map(|asked| asked.thickness_mm);
        Self {
            id: object.id,
            mesh: Arc::clone(object.uncut()),
            cuts: hollow.cut_bodies().map(Arc::clone),
            transform: object.transform,
            size,
            wall: wall.zip(hollow.lattice().map(|(voxel_mm, _)| voxel_mm)),
        }
    }
}

/// Everything one drain search needs.
pub struct DrainsRequest {
    pub tasks: Vec<DrainsTask>,
    pub layer_height_mm: Scalar,
    /// Whether each drained pocket gets a vent too.
    pub vents: bool,
}

/// What the search chose for one model, in its own space, and what it was looked for in.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposed {
    pub id: ObjectId,
    pub mesh: Arc<Mesh>,
    pub transform: Transform,
    pub holes: Vec<DrainHole>,
    /// What no hole the search found could drain.
    pub trapped: Vec<Trapped>,
}

/// How a drain search ended.
#[derive(Debug, Clone, PartialEq)]
pub enum DrainsOutcome {
    Found(Vec<Proposed>),
    Cancelled,
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// A drain search over the picked models on its own thread; dropping it asks it to stop
/// between windows of a scan, and nothing it found lands.
#[derive(Debug)]
pub struct DrainsJob {
    worker: Worker<DrainsOutcome>,
    heard: Arc<Heard>,
    models: usize,
    shown: Highest,
}

/// How far a search has got: the models done, and the share of the one in hand as bits.
#[derive(Debug, Default)]
struct Heard {
    models: AtomicUsize,
    share: AtomicU32,
}

impl DrainsJob {
    pub fn spawn(request: DrainsRequest) -> Self {
        let heard = Arc::new(Heard::default());
        let models = request.tasks.len();
        let told = Arc::clone(&heard);
        let worker = Worker::spawn(
            move |stop, _| pooled(|| search(&request, stop, &told), DrainsOutcome::Failed),
            || DrainsOutcome::Failed(tr!("job-thread-stopped-drain-search")),
        );
        Self {
            worker,
            heard,
            models,
            shown: Highest::default(),
        }
    }

    /// The outcome on the frame the search ends, and `None` while it is still running.
    pub fn poll(&mut self) -> Option<DrainsOutcome> {
        self.worker.poll()
    }

    /// The share of the most the search can take that is done; it may end sooner.
    pub fn fraction(&self) -> f32 {
        let done = self.heard.models.load(Relaxed) as f32;
        let share = f32::from_bits(self.heard.share.load(Relaxed));
        self.shown
            .keep(((done + share) / self.models.max(1) as f32).min(1.0))
    }
}

fn search(request: &DrainsRequest, stop: &AtomicBool, heard: &Heard) -> DrainsOutcome {
    let mut found = Vec::with_capacity(request.tasks.len());
    for task in &request.tasks {
        heard.share.store(0.0_f32.to_bits(), Relaxed);
        match look_for_holes(task, request, (stop, heard)) {
            Ok(Some(proposed)) => found.push(proposed),
            Ok(None) => return DrainsOutcome::Cancelled,
            Err(message) => return DrainsOutcome::Failed(message),
        }
        heard.models.fetch_add(1, Relaxed);
    }
    DrainsOutcome::Found(found)
}

/// The holes for one model as it stands on the plate, brought back into its own space;
/// `None` once asked to stop.
fn look_for_holes(
    task: &DrainsTask,
    request: &DrainsRequest,
    (stop, heard): (&AtomicBool, &Heard),
) -> Result<Option<Proposed>, String> {
    let placed = transform_mesh(&task.mesh, task.transform);
    let cuts = task
        .cuts
        .as_ref()
        .map(|cuts| transform_mesh(cuts, task.transform));
    let plate_z = placed
        .aabb()
        .map(|bounds| bounds.mins.z)
        .filter(|lowest| lowest.abs() <= ON_PLATE_MM);
    let search = DrainSearch {
        mesh: &placed,
        cuts: cuts.as_ref(),
        size: task.size,
        wall: task.wall,
        vents: request.vents,
        plate_z,
        layer_height_mm: request.layer_height_mm,
    };
    let drained = suggest_drains_heard(&search, |scanning| {
        heard.share.store(scanning.share().to_bits(), Relaxed);
        !stop.load(Relaxed)
    })
    .map_err(flattened)?;
    Ok(drained.map(|drained| proposed(task, drained)))
}

fn proposed(task: &DrainsTask, drained: Drained) -> Proposed {
    let transform = task.transform;
    Proposed {
        id: task.id,
        mesh: Arc::clone(&task.mesh),
        transform,
        holes: drained
            .drain_holes()
            .filter_map(|hole| hole.into_model(transform))
            .collect(),
        trapped: into_model(drained.trapped, transform),
    }
}

#[cfg(test)]
mod tests;
