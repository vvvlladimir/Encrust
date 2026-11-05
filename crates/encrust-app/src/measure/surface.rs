use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Weak};
use std::time::Duration;

use core_geometry::{
    Adjacency, Mat4, Mesh, SurfacePath, SurfacePathError, SurfacePoint, Transform, Vec3, WalkWatch,
    surface_path,
};

use super::{EdgeCache, Measure};
use crate::i18n::tr;
use crate::job::Worker;
use crate::scene::{ObjectId, Scene, SceneObject};

/// The model, placement and face a picked point lies on, so a walk over the surface can
/// start there.
#[derive(Debug, Clone)]
pub struct Anchor {
    id: ObjectId,
    /// Held so a mesh replaced since cannot free its address for another to pass as it.
    mesh: Weak<Mesh>,
    transform: Transform,
    face: usize,
}

impl PartialEq for Anchor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.mesh.ptr_eq(&other.mesh)
            && self.transform == other.transform
            && self.face == other.face
    }
}

impl Anchor {
    /// The face of `object` nearest `point`, plate coordinates; `None` for a model scaled
    /// flat.
    pub fn of(object: &SceneObject, point: Vec3) -> Option<Self> {
        let inverse = object.transform.inverse_matrix()?;
        let closest = object.bvh.closest(inverse.transform_point3(point))?;
        Some(Self {
            id: object.id,
            mesh: Arc::downgrade(&object.mesh),
            transform: object.transform,
            face: closest.face,
        })
    }

    /// Whether this was taken on `object` as it is now, so its face still indexes its mesh
    /// and the point still lies on it.
    fn is_on(&self, object: &SceneObject) -> bool {
        self.id == object.id
            && std::ptr::eq(self.mesh.as_ptr(), Arc::as_ptr(&object.mesh))
            && self.transform == object.transform
    }
}

/// What a walk over the surface between the two picked points came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Walk {
    Found(SurfacePath),
    /// The points lie on two models, or on one changed since they were picked.
    Apart,
    /// The points lie on two shells of one model that no edge joins.
    Unreachable,
    /// Cancelled before it was found.
    Stopped,
    /// The thread finding it stopped, with why.
    Failed(String),
}

/// How long a walk is found before its notice shows at the stage's foot, so a short one
/// does not flash a card.
const NOTICE_AFTER: Duration = Duration::from_millis(300);

/// A walk being found on its own thread: A* over a model of a million faces can take
/// longer than a frame.
#[derive(Debug, Default)]
pub struct Walking {
    running: Option<Running>,
}

/// A walk on its thread, for the span it was asked for.
#[derive(Debug)]
struct Running {
    span: (Vec3, Vec3),
    job: Worker<Walk>,
    /// The share found so far, an `f32`'s bits, written by the thread.
    share: Arc<AtomicU32>,
    started: web_time::Instant,
}

impl Walking {
    /// The share of the walk found, once it has taken long enough to show at the stage's foot.
    pub fn notice(&self) -> Option<f32> {
        let running = self.running.as_ref()?;
        (running.started.elapsed() >= NOTICE_AFTER)
            .then(|| f32::from_bits(running.share.load(Relaxed)))
    }

    /// Gives up the walk being found, and says so in `measure`.
    pub fn cancel(&mut self, measure: &mut Measure) {
        if self.running.take().is_some() {
            measure.set_walk(Walk::Stopped);
        }
    }

    /// Starts finding the walk `measure` asks for, and hands it over once found. Answers
    /// whether one is still being found.
    pub fn poll(&mut self, measure: &mut Measure, edges: &mut EdgeCache, scene: &Scene) -> bool {
        if matches!(measure.walk(), Some(Walk::Found(_))) && on_one_model(measure, scene).is_none()
        {
            measure.set_walk(Walk::Apart);
        }
        let wanted = measure
            .span()
            .filter(|_| measure.along_surface && measure.walk().is_none());
        let Some(span) = wanted else {
            self.running = None;
            return false;
        };
        if let Some(running) = &mut self.running
            && running.span == span
        {
            let Some(walk) = running.job.poll() else {
                return true;
            };
            measure.set_walk(walk);
            self.running = None;
            return false;
        }
        let share = Arc::new(AtomicU32::new(0));
        match start(measure, edges, scene, Arc::clone(&share)) {
            Some(job) => {
                self.running = Some(Running {
                    span,
                    job,
                    share,
                    started: web_time::Instant::now(),
                });
                true
            }
            None => {
                measure.set_walk(Walk::Apart);
                self.running = None;
                false
            }
        }
    }
}

/// The model both picked points lie on, as it was when they were picked.
fn on_one_model<'a>(measure: &Measure, scene: &'a Scene) -> Option<&'a SceneObject> {
    let [(_, from_on), (_, to_on)] = measure.anchored()?;
    let object = scene.get(from_on.id)?;
    (from_on.is_on(object) && to_on.is_on(object)).then_some(object)
}

/// Starts the walk between the picked points, when both are on one model as it is now.
fn start(
    measure: &Measure,
    edges: &mut EdgeCache,
    scene: &Scene,
    share: Arc<AtomicU32>,
) -> Option<Worker<Walk>> {
    let object = on_one_model(measure, scene)?;
    let (mesh, adjacency) = (Arc::clone(&object.mesh), edges.of(&object.mesh));
    let matrix = object.transform.to_matrix();
    let ends = measure.anchored()?.map(|(point, on)| SurfacePoint {
        face: on.face,
        point,
    });
    Some(Worker::spawn(
        move |stop, _| {
            let mut watch = |found: f32| {
                share.store(found.to_bits(), Relaxed);
                stopped(stop)
            };
            walk((&mesh, &adjacency, matrix), ends, &mut watch)
        },
        || Walk::Failed(tr!("job-thread-stopped-measure-walk")),
    ))
}

fn stopped(stop: &AtomicBool) -> ControlFlow<()> {
    if stop.load(Relaxed) {
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
}

fn walk(
    (mesh, adjacency, matrix): (&Mesh, &Adjacency, Mat4),
    [from, to]: [SurfacePoint; 2],
    watch: WalkWatch,
) -> Walk {
    match surface_path(mesh, adjacency, matrix, from, to, watch) {
        Ok(path) => Walk::Found(path),
        Err(SurfacePathError::Unreachable { .. }) => Walk::Unreachable,
        Err(SurfacePathError::FaceOutOfRange { .. }) => Walk::Apart,
        Err(SurfacePathError::Stopped) => Walk::Stopped,
    }
}

#[cfg(test)]
mod tests;
