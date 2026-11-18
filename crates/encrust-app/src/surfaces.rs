//! The hierarchy of every surface supports stand against, built off the drawing thread: a
//! dense shell takes a while, and a frame waiting on it freezes the window (ADR 0332).

use std::sync::{Arc, Weak};

use core_geometry::{Bvh, Mesh};

use crate::i18n::tr;
use crate::job::{Worker, pooled};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

/// The hierarchies being built, one run a surface, and the surfaces one failed on.
#[derive(Debug, Default)]
pub struct SupportSurfaces {
    runs: Vec<Run>,
    /// Not built again while the model prints as them; it stands on itself meanwhile.
    failed: Vec<Weak<Mesh>>,
}

/// A run for one model's surface, held weakly so a replaced one is let go.
#[derive(Debug)]
struct Run {
    model: ObjectId,
    mesh: Weak<Mesh>,
    /// The hierarchy, or why not: `None` when the run stopped without a word.
    job: Worker<Result<Arc<Bvh>, Option<String>>>,
}

impl SupportSurfaces {
    /// Lets go of every surface a model no longer prints as, and starts building the
    /// hierarchy of each one that has none yet, no run under way and no failed one.
    pub fn start(&mut self, scene: &mut Scene) {
        for object in scene.objects_mut() {
            if object.forget_stale_support_surface() {
                object.supports.surface_changed();
            }
            let Some(mesh) = object.unbuilt_support_surface() else {
                continue;
            };
            let busy = self.runs.iter().any(|run| {
                run.model == object.id
                    && run.mesh.upgrade().is_some_and(|it| Arc::ptr_eq(&it, mesh))
            });
            let failed = self
                .failed
                .iter()
                .any(|it| it.upgrade().is_some_and(|it| Arc::ptr_eq(&it, mesh)));
            if busy || failed {
                continue;
            }
            let built = Arc::clone(mesh);
            self.runs.push(Run {
                model: object.id,
                mesh: Arc::downgrade(mesh),
                job: Worker::spawn(
                    move |_, _| pooled(|| Ok(Arc::new(Bvh::build(&built))), |why| Err(Some(why))),
                    || Err(None),
                ),
            });
        }
    }

    pub fn is_running(&self) -> bool {
        !self.runs.is_empty()
    }

    /// Keeps each built hierarchy whose model still prints as its surface, tells `status` of
    /// a failed run, and answers whether any is still going.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let failed = &mut self.failed;
        self.runs.retain_mut(|run| {
            let Some(bvh) = run.job.poll() else {
                return true;
            };
            let bvh = match bvh {
                Ok(bvh) => bvh,
                Err(why) => {
                    *status = Status::Error(why.map_or_else(
                        || tr!("surfaces-build-stopped"),
                        |reason| tr!("surfaces-cannot-build", reason = reason),
                    ));
                    failed.push(run.mesh.clone());
                    return false;
                }
            };
            let Some(mesh) = run.mesh.upgrade() else {
                return false;
            };
            if let Some(object) = scene.get_mut(run.model)
                && object.keep_support_bvh(&mesh, bvh)
            {
                object.supports.surface_changed();
            }
            false
        });
        self.failed.retain(|mesh| mesh.strong_count() > 0);
        !self.runs.is_empty()
    }
}

#[cfg(test)]
mod tests;
