use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

use core_geometry::{Aabb, Mat4, Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_supports::Trapped;

use core_engine::{Baked, trapped_in};

use crate::i18n::tr;
use crate::job::{Highest, Worker, pooled};
use crate::scene::{ObjectId, Scene};

/// One model to look through, owned so that it can cross to the worker thread.
pub struct TrapTask {
    pub id: ObjectId,
    /// What will actually be printed — the shell, where the model has been hollowed — in
    /// the model's own space.
    pub mesh: Arc<Mesh>,
    /// Its holes and channels, wound inward, in the same space: the check is about what
    /// the resin can get out through, so it looks at the cuts as well as the cavity.
    pub cuts: Option<Arc<Mesh>>,
    pub transform: Transform,
}

/// Everything one drainage check needs.
pub struct TrapRequest {
    pub tasks: Vec<TrapTask>,
    pub layer_height_mm: Scalar,
}

/// What one model's stack said, in the model's own space so that the pockets travel with
/// it the way the holes do.
#[derive(Debug, Clone, PartialEq)]
pub struct Pockets {
    pub id: ObjectId,
    pub trapped: Vec<Trapped>,
}

/// How a drainage check ended.
#[derive(Debug, Clone, PartialEq)]
pub enum TrapOutcome {
    Checked(Vec<Pockets>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// A drainage check on its own thread. Dropping the job asks the check to stop.
///
/// It cuts each model window by window and keeps none of it, so a plate the window could
/// not hold as a stack can still be checked; see ADR 0072.
#[derive(Debug)]
pub struct TrapJob {
    worker: Worker<TrapOutcome>,
    looked: Arc<Looked>,
    models: usize,
    shown: Highest,
}

/// How far a check has got: the models done, and the layers cut of the one in hand.
#[derive(Debug, Default)]
struct Looked {
    models: AtomicUsize,
    layers: AtomicUsize,
    layers_total: AtomicUsize,
}

impl TrapJob {
    pub fn spawn(request: TrapRequest) -> Self {
        let looked = Arc::new(Looked::default());
        let models = request.tasks.len();
        let told = Arc::clone(&looked);
        let worker = Worker::spawn(
            move |stop, _| pooled(|| check(&request, (stop, &told)), TrapOutcome::Failed),
            || TrapOutcome::Failed(tr!("job-thread-stopped-drainage")),
        );
        Self {
            worker,
            looked,
            models,
            shown: Highest::default(),
        }
    }

    /// The outcome on the frame the check ends, and `None` while it is still running.
    pub fn poll(&mut self) -> Option<TrapOutcome> {
        self.worker.poll()
    }

    /// The share of the plate looked through, or `None` before the first model's layers
    /// are counted.
    pub fn fraction(&self) -> Option<f32> {
        let layers_total = self.looked.layers_total.load(Relaxed);
        (layers_total > 0).then(|| {
            let layer = self.looked.layers.load(Relaxed).min(layers_total);
            let done = self.looked.models.load(Relaxed);
            let share = (done as f32 + layer as f32 / layers_total as f32) / self.models as f32;
            self.shown.keep(share.min(1.0))
        })
    }
}

/// Every visible model with geometry on the plate being edited, as it will be printed.
pub fn trap_tasks(scene: &Scene) -> Vec<TrapTask> {
    scene
        .targets()
        .filter(|object| !object.mesh.is_empty())
        .map(|object| TrapTask {
            id: object.id,
            mesh: Arc::clone(object.hollow.shell().unwrap_or(&object.mesh)),
            cuts: object.hollow.cut_bodies().map(Arc::clone),
            transform: object.transform,
        })
        .collect()
}

fn check(request: &TrapRequest, (stop, looked): (&AtomicBool, &Looked)) -> TrapOutcome {
    let mut checked: Vec<Pockets> = Vec::with_capacity(request.tasks.len());
    for (index, task) in request.tasks.iter().enumerate() {
        let trapped = match twin_of(&request.tasks[..index], task) {
            Some(twin) => checked[twin].trapped.clone(),
            None => match look_through(task, request.layer_height_mm, (stop, looked)) {
                Ok(trapped) => trapped,
                Err(message) => return TrapOutcome::Failed(message),
            },
        };
        checked.push(Pockets {
            id: task.id,
            trapped,
        });
        looked.layers.store(0, Relaxed);
        looked.models.fetch_add(1, Relaxed);
    }
    TrapOutcome::Checked(checked)
}

/// An earlier task `task` is a copy of: one mesh and one set of cuts, standing the same
/// way at the same height and only somewhere else on the plate. Its pockets are kept in
/// the model's own space and the scan's grid starts at the model's corner, so the copy's
/// are the same and are not looked for again.
fn twin_of(earlier: &[TrapTask], task: &TrapTask) -> Option<usize> {
    let same_cuts = |other: &TrapTask| match (&other.cuts, &task.cuts) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    };
    let (placed, at) = (task.transform, task.transform.translation);
    earlier.iter().position(|other| {
        let (turned, from) = (other.transform, other.transform.translation);
        Arc::ptr_eq(&other.mesh, &task.mesh)
            && same_cuts(other)
            && (turned.rotation, turned.scale, turned.stretch)
                == (placed.rotation, placed.scale, placed.stretch)
            && from.z == at.z
    })
}

/// The bodies a cut is made of are appended to whatever they cut; see ADR 0075.
fn append(whole: &mut Mesh, part: &Mesh) {
    let offset = whole.vertices.len() as u32;
    whole.vertices.extend_from_slice(&part.vertices);
    whole.faces.extend(
        part.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
}

/// Cuts one model where it stands and answers with the pockets in it, brought back into
/// the model's own space.
fn look_through(
    task: &TrapTask,
    layer_height_mm: Scalar,
    (stop, looked): (&AtomicBool, &Looked),
) -> Result<Vec<Trapped>, String> {
    let mut placed = transform_mesh(&task.mesh, task.transform);
    let Some(material) = placed.aabb() else {
        return Ok(Vec::new());
    };
    if let Some(cuts) = &task.cuts {
        append(&mut placed, &transform_mesh(cuts, task.transform));
    }
    let baked = Baked {
        solid: placed.into(),
        ceiling_mm: material.maxs.z,
        floor_mm: material.mins.z,
    };
    let read = trapped_in(&baked, layer_height_mm, |layers, total| {
        looked.layers_total.store(total, Relaxed);
        looked.layers.store(layers, Relaxed);
        !stop.load(Relaxed)
    })
    .map_err(flattened)?;
    let Some(found) = read else {
        return Err(tr!("drain-cancelled"));
    };
    Ok(into_model(found, task.transform))
}

/// `error` with its causes in one line, as the window has no terminal.
pub(super) fn flattened(error: core_engine::EngineError) -> String {
    anyhow::Error::new(error)
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

/// Pockets found on the plate brought back into the own space of a model placed by
/// `transform`, so they travel with it; none for a placement flat to nothing.
pub(super) fn into_model(found: Vec<Trapped>, transform: Transform) -> Vec<Trapped> {
    let matrix = transform.to_matrix();
    if matrix.determinant().abs() < f32::EPSILON {
        return Vec::new();
    }
    let inverse = matrix.inverse();
    let volume = transform.linear().determinant().abs().max(f32::EPSILON);
    found
        .into_iter()
        .map(|found| Trapped {
            at: inverse.transform_point3(found.at),
            volume_mm3: found.volume_mm3 / volume,
            bounds: carried(&found.bounds, inverse),
        })
        .collect()
}

/// `bounds` taken through `matrix` as the box of its own eight corners, so a pocket of a
/// model that was turned still covers the pocket.
fn carried(bounds: &Aabb, matrix: Mat4) -> Aabb {
    let corner = |index: usize| {
        let pick = |axis: usize| match index >> axis & 1 {
            0 => bounds.mins[axis],
            _ => bounds.maxs[axis],
        };
        matrix.transform_point3(Vec3::new(pick(0), pick(1), pick(2)))
    };
    let mut box_of = Aabb::new(corner(0), corner(0));
    for index in 1..8 {
        box_of.take_point(corner(index));
    }
    box_of
}

#[cfg(test)]
mod tests;
