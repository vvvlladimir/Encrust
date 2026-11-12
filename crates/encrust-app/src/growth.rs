//! The support trees of every model, grown off the drawing thread: merging a thousand tips
//! into trunks takes a second or more on a dense model (ADR 0342).

use std::sync::Arc;

use core_geometry::Transform;
use core_supports::Grown;
use printer_profiles::SupportProfile;

use crate::i18n::tr;
use crate::job::{Worker, pooled};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::state::Doc;
use crate::status::Status;
use crate::supports::{SupportTool, with_own};

/// The trees being grown, one run a model at most, and the runs that failed.
#[derive(Debug, Default)]
pub struct SupportGrowth {
    runs: Vec<Run>,
    /// What a run failed on, not tried again until the model's supports change.
    failed: Vec<Asked>,
}

#[derive(Debug)]
struct Run {
    asked: Asked,
    /// The trees, or why not: `None` when the run stopped without a word.
    job: Worker<Result<Grown, Option<String>>>,
}

/// What one model's trees were asked to grow for.
#[derive(Debug, Clone, PartialEq)]
struct Asked {
    model: ObjectId,
    revision: u64,
    transform: Transform,
    table: Vec<SupportProfile>,
}

impl SupportGrowth {
    /// Starts growing `object`'s trees to `table` unless they hold, a run for it is under way
    /// (it goes on to its end) or one failed on the same asking.
    pub fn start(&mut self, object: &SceneObject, table: &[SupportProfile]) {
        if self.runs.iter().any(|run| run.asked.model == object.id) {
            return;
        }
        let asked = Asked {
            model: object.id,
            revision: object.supports.revision(),
            transform: object.transform,
            table: table.to_vec(),
        };
        if self.failed.contains(&asked) {
            return;
        }
        let Some(growth) = object.supports.growth(object.transform, table) else {
            return;
        };
        let model = Arc::clone(&object.mesh);
        let (surface, bvh) = object.support_surface();
        let (surface, bvh) = (Arc::clone(surface), Arc::clone(bvh));
        self.runs.push(Run {
            asked,
            job: Worker::spawn(
                move |_, _| {
                    pooled(
                        || Ok(growth.grow(&model, &surface, &bvh)),
                        |why| Err(Some(why)),
                    )
                },
                || Err(None),
            ),
        });
    }

    /// Hands grown trees to their models, tells `status` of a failed run, and answers whether
    /// any run is still going.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let failed = &mut self.failed;
        self.runs.retain_mut(|run| {
            let Some(grown) = run.job.poll() else {
                return true;
            };
            match grown {
                Ok(grown) => {
                    if let Some(object) = scene.get_mut(run.asked.model) {
                        object.supports.take_grown(grown);
                    }
                }
                Err(why) => {
                    *status = Status::Error(why.map_or_else(
                        || tr!("supports-growth-stopped"),
                        |reason| tr!("supports-cannot-grow", reason = reason),
                    ));
                    // Only a model's last failure is kept: dragged on, it fails anew at each place.
                    failed.retain(|asked| asked.model != run.asked.model);
                    failed.push(run.asked.clone());
                }
            }
            false
        });
        self.failed.retain(|asked| {
            scene
                .get(asked.model)
                .is_some_and(|object| object.supports.revision() == asked.revision)
        });
        !self.runs.is_empty()
    }
}

/// Rebuilds the columns and the painted patches of every object whose placement,
/// points, paint or profile moved under them, so that what the viewport draws is what
/// would be sliced. A profile still being dragged is grown to once it is let go.
pub fn refresh_supports(doc: &mut Doc, supports: &mut SupportTool, held: bool) {
    supports.settle_on_raft(&mut doc.scene);
    let plate = supports.table();
    let raft = supports.raft;
    doc.surfaces.start(&mut doc.scene);
    for object in doc.scene.objects_mut() {
        let untouched = object.supports.is_empty()
            && object.supports.meshes().is_none()
            && object.supports.drawn().is_empty()
            && object.supports.painted().is_empty()
            && object.supports.blocked().is_empty();
        if untouched {
            continue;
        }
        let table = with_own(plate.clone(), &object.own, raft);
        let retuned = held && object.supports.retuned(object.transform, &table);
        if object.supports.outgrown(object.transform, &table) {
            object
                .supports
                .refresh_patches(&object.mesh, object.transform);
            if !retuned {
                doc.growth.start(object, &table);
            }
            continue;
        }
        object.refresh_supports(&table);
    }
}

#[cfg(test)]
mod tests;
