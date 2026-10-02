use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{Mesh, Scalar, Transform, transform_mesh};
use core_supports::{TrapScan, Trapped};

use core_engine::{Cutting, cut};

use crate::job::pipeline::{thread_pool, worker_threads};
use crate::scene::{ObjectId, Scene};

/// One model to look through, owned so that it can cross to the worker thread.
pub struct TrapTask {
    pub id: ObjectId,
    /// What will actually be printed — the shell, where the model has been hollowed — in
    /// the model's own space.
    pub mesh: Arc<Mesh>,
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
        std::thread::spawn(move || {
            let _ = sender.send(check(&request));
        });
        Self { result }
    }

    /// The outcome on the frame the check ends, and `None` while it is still running.
    pub fn poll(&mut self) -> Option<TrapOutcome> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(TrapOutcome::Failed(
                "the drainage thread stopped without finishing".to_owned(),
            )),
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
            // The cuts go in with the model: the check is about what the resin can get
            // out through, so it has to look at the holes as well as the cavity.
            mesh: Arc::new(drained(object)),
            transform: object.transform,
        })
        .collect()
}

/// One model as it will be sliced: its shell, or its own mesh, with its cuts appended.
fn drained(object: &crate::scene::SceneObject) -> Mesh {
    let mesh = object.hollow.shell().unwrap_or(&object.mesh);
    let Some(cuts) = object.hollow.cut_bodies() else {
        return mesh.as_ref().clone();
    };

    let mut drained = mesh.as_ref().clone();
    let offset = drained.vertices.len() as u32;
    drained.vertices.extend_from_slice(&cuts.vertices);
    drained.faces.extend(
        cuts.faces
            .iter()
            .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
    );
    drained
}

fn check(request: &TrapRequest) -> TrapOutcome {
    let pool = match thread_pool(worker_threads()) {
        Ok(pool) => pool,
        Err(error) => return TrapOutcome::Failed(error.to_string()),
    };

    pool.install(|| {
        let mut checked = Vec::with_capacity(request.tasks.len());
        for task in &request.tasks {
            match look_through(task, request.layer_height_mm) {
                Ok(trapped) => checked.push(Pockets {
                    id: task.id,
                    trapped,
                }),
                Err(message) => return TrapOutcome::Failed(message),
            }
        }
        TrapOutcome::Checked(checked)
    })
}

/// Cuts one model where it stands and answers with the pockets in it, brought back into
/// the model's own space.
fn look_through(task: &TrapTask, layer_height_mm: Scalar) -> Result<Vec<Trapped>, String> {
    let placed = transform_mesh(&task.mesh, task.transform);
    let Some(bounds) = placed.aabb() else {
        return Ok(Vec::new());
    };
    let windows = cut(&placed, &Cutting::uniform(layer_height_mm)).map_err(|error| {
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
        .stream(&placed, |sliced| {
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
    let scale = task.transform.scale.abs();
    let volume = (scale.x * scale.y * scale.z).max(f32::EPSILON);

    Ok(scan
        .finish()
        .into_iter()
        .map(|found| Trapped {
            at: inverse.transform_point3(found.at),
            volume_mm3: found.volume_mm3 / volume,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;

    /// An axis-aligned box spanning `min`..`max`, wound outward.
    fn box_mesh(min: Vec3, max: Vec3) -> Mesh {
        let corners = vec![
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
        Mesh::new(corners, faces)
    }

    /// A 20 mm cube with a 10 mm cube of air sealed inside it, the cavity wound inward
    /// the way a hollowed model carries it.
    fn sealed_box() -> Mesh {
        let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(20.0));
        let cavity = box_mesh(Vec3::splat(5.0), Vec3::splat(15.0));
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend_from_slice(&cavity.vertices);
        mesh.faces.extend(
            cavity
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
        );
        mesh
    }

    fn wait(job: &mut TrapJob) -> TrapOutcome {
        loop {
            if let Some(outcome) = job.poll() {
                return outcome;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_sealed_cavity_is_found_where_its_floor_is() {
        let mut job = TrapJob::spawn(TrapRequest {
            tasks: vec![TrapTask {
                id: ObjectId::for_test(1),
                mesh: Arc::new(sealed_box()),
                transform: Transform::default(),
            }],
            layer_height_mm: 0.5,
        });

        let TrapOutcome::Checked(models) = wait(&mut job) else {
            panic!("a closed mesh slices");
        };
        assert_eq!(models[0].trapped.len(), 1, "one sealed cavity");

        let pocket = models[0].trapped[0];
        assert!(
            (pocket.volume_mm3 - 1000.0).abs() < 60.0,
            "a 10 mm cube of air is 1000 mm3, got {}",
            pocket.volume_mm3
        );
        assert!(
            (pocket.at.z - 5.25).abs() < 0.3,
            "the hole goes at the cavity's floor, got {}",
            pocket.at
        );
    }

    #[test]
    fn a_solid_model_has_nothing_trapped_in_it() {
        let mut job = TrapJob::spawn(TrapRequest {
            tasks: vec![TrapTask {
                id: ObjectId::for_test(2),
                mesh: Arc::new(box_mesh(Vec3::ZERO, Vec3::splat(10.0))),
                transform: Transform::default(),
            }],
            layer_height_mm: 0.5,
        });

        let TrapOutcome::Checked(models) = wait(&mut job) else {
            panic!("a closed mesh slices");
        };
        assert!(models[0].trapped.is_empty());
    }
}
