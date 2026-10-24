use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Aabb, Mat4, Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_supports::{TrapScan, Trapped};

use core_engine::{Baked, Cutting, cut};

use crate::i18n::tr;
use crate::job::pipeline::{thread_pool, worker_threads};
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

/// A drainage check on its own thread.
///
/// It cuts each model window by window and keeps none of it, so a plate the window could
/// not hold as a stack can still be checked; see ADR 0072.
#[derive(Debug)]
pub struct TrapJob {
    result: Receiver<TrapOutcome>,
}

impl TrapJob {
    pub fn spawn(request: TrapRequest) -> Self {
        let (sender, result) = mpsc::channel();
        crate::job::spawn(move || {
            let _ = sender.send(check(&request));
        });
        Self { result }
    }

    /// The outcome on the frame the check ends, and `None` while it is still running.
    pub fn poll(&mut self) -> Option<TrapOutcome> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(TrapOutcome::Failed(tr!("job-thread-stopped-drainage")))
            }
        }
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

fn check(request: &TrapRequest) -> TrapOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return TrapOutcome::Failed(error.to_string()),
    };

    pool.install(|| {
        let mut checked: Vec<Pockets> = Vec::with_capacity(request.tasks.len());
        for (index, task) in request.tasks.iter().enumerate() {
            let trapped = match twin_of(&request.tasks[..index], task) {
                Some(twin) => checked[twin].trapped.clone(),
                None => match look_through(task, request.layer_height_mm) {
                    Ok(trapped) => trapped,
                    Err(message) => return TrapOutcome::Failed(message),
                },
            };
            checked.push(Pockets {
                id: task.id,
                trapped,
            });
        }
        TrapOutcome::Checked(checked)
    })
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
fn look_through(task: &TrapTask, layer_height_mm: Scalar) -> Result<Vec<Trapped>, String> {
    let mut placed = transform_mesh(&task.mesh, task.transform);
    let Some(material) = placed.aabb() else {
        return Ok(Vec::new());
    };
    if let Some(cuts) = &task.cuts {
        append(&mut placed, &transform_mesh(cuts, task.transform));
    }
    let bounds = placed.aabb().unwrap_or(material);
    let baked = Baked {
        solid: placed.into(),
        ceiling_mm: material.maxs.z,
        floor_mm: material.mins.z,
    };
    let windows = cut(&baked, &Cutting::uniform(layer_height_mm)).map_err(|error| {
        anyhow::Error::new(error)
            .chain()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(": ")
    })?;

    let mut scan = TrapScan::new(
        bounds.mins.truncate(),
        bounds.maxs.truncate(),
        layer_height_mm,
    );
    windows
        .stream(&baked.solid, |sliced| {
            for layer in &sliced.layers {
                scan.push(layer);
            }
            Ok::<(), core_slicer::SliceError>(())
        })
        .map_err(|error| error.to_string())?;

    let matrix = task.transform.to_matrix();
    if matrix.determinant().abs() < f32::EPSILON {
        return Ok(Vec::new());
    }
    let inverse = matrix.inverse();
    let volume = task
        .transform
        .linear()
        .determinant()
        .abs()
        .max(f32::EPSILON);

    Ok(scan
        .finish()
        .into_iter()
        .map(|found| Trapped {
            at: inverse.transform_point3(found.at),
            volume_mm3: found.volume_mm3 / volume,
            bounds: carried(&found.bounds, inverse),
        })
        .collect())
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
