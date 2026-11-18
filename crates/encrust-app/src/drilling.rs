//! Every model's cuts taken out of it and its raft exactly, off the drawing thread
//! (ADR 0322, 0441).

use std::sync::{Arc, Weak};

use core_geometry::Mesh;

use core_volume::{ModelRead, Step};

use crate::i18n::tr;
use crate::job::{
    Counted, DrillJob, DrillRequest, PrintedSupportsJob, PrintedSupportsRequest, spawn_drill,
    spawn_printed_supports,
};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

mod reads;
mod supports;

pub use reads::ModelReads;
pub use supports::{BuiltFrom, PrintedSupports, SupportsUnder};

/// The cuts being taken out, one run a model and one for its raft at a time.
#[derive(Debug, Default)]
pub struct Drilling {
    runs: Vec<Run>,
    support_runs: Vec<SupportRun>,
    /// What the runs and the plugs' cuts read the models as, shared between them.
    pub reads: ModelReads,
}

/// A run for one model's printed raft, and what it was started for.
#[derive(Debug)]
struct SupportRun {
    model: ObjectId,
    from: BuiltFrom,
    job: PrintedSupportsJob,
}

/// A run for one model, and the meshes it took, held weakly so a replaced one is let go.
#[derive(Debug)]
struct Run {
    model: ObjectId,
    read: Arc<ModelRead>,
    source: Weak<Mesh>,
    cuts: Weak<Mesh>,
    job: DrillJob,
}

impl Drilling {
    /// Starts taking the cuts out of every model whose cuts or mesh changed since they last
    /// were and that has no run under way. None starts while `settled` is false, in the
    /// middle of a drag: the cuts follow once it ends.
    pub fn start(&mut self, scene: &Scene, settled: bool) {
        if !settled {
            return;
        }
        for object in scene.objects() {
            let source = object.uncut();
            let busy = self.runs.iter().any(|run| run.model == object.id);
            let Some(cuts) = object.hollow.cut_bodies() else {
                continue;
            };
            if busy || !object.hollow.wants_drilling(source) {
                continue;
            }
            let read = self.reads.of(&object.mesh);
            let job = spawn_drill(DrillRequest {
                read: Arc::clone(&read),
                source: Arc::clone(source),
                cavity: object.hollow.cavity_faces(),
                cuts: Arc::clone(cuts),
            });
            self.runs.push(Run {
                model: object.id,
                read,
                source: Arc::downgrade(source),
                cuts: Arc::downgrade(cuts),
                job,
            });
        }
        self.start_supports(scene);
    }

    /// Starts building the printed raft of every model whose raft, cuts or placement changed
    /// since it last was and that has no such run under way.
    fn start_supports(&mut self, scene: &Scene) {
        for object in scene.objects() {
            let under = object.supports_under();
            let busy = self.support_runs.iter().any(|run| run.model == object.id);
            let Some(raft) = under
                .raft
                .filter(|_| !busy && object.printed_supports.wants(under))
            else {
                continue;
            };
            self.support_runs.push(SupportRun {
                model: object.id,
                from: BuiltFrom::of(under),
                job: spawn_printed_supports(PrintedSupportsRequest {
                    raft: raft.clone(),
                    cuts: under.cuts.cloned(),
                    transform: under.transform,
                }),
            });
        }
    }

    /// Keeps what each raft run that has ended built, where its model still stands as it was
    /// started for. A lost run is noted, and not started again for the same raft.
    fn poll_supports(&mut self, scene: &mut Scene) {
        let mut index = 0;
        while index < self.support_runs.len() {
            let Some(outcome) = self.support_runs[index].job.poll() else {
                index += 1;
                continue;
            };
            let run = self.support_runs.swap_remove(index);
            let Some(object) = scene.get_mut(run.model) else {
                continue;
            };
            if !run.from.matches(object.supports_under()) {
                continue;
            }
            match outcome {
                Some((raft, bvh)) => object.printed_supports.keep(run.from, raft, bvh),
                None => object.printed_supports.lose(run.from),
            }
        }
    }

    /// Keeps what each run that has ended took out, where its model still has the mesh and
    /// the cuts it was started for. Returns whether any run is still going.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let mut index = 0;
        while index < self.runs.len() {
            let Some(outcome) = self.runs[index].job.poll() else {
                index += 1;
                continue;
            };
            let run = self.runs.swap_remove(index);
            let (Some(source), Some(cuts)) = (run.source.upgrade(), run.cuts.upgrade()) else {
                continue;
            };
            let Some(object) = scene.get_mut(run.model) else {
                continue;
            };
            if !Arc::ptr_eq(object.uncut(), &source) {
                continue;
            }
            let current = object
                .hollow
                .cut_bodies()
                .is_some_and(|now| Arc::ptr_eq(now, &cuts));
            if outcome.is_err() && current {
                *status = Status::Info(tr!("drill-refused", name = object.name.clone()));
            }
            object.hollow.keep_drilled(&source, &cuts, outcome.ok());
        }
        self.poll_supports(scene);
        self.is_running()
    }

    /// The step and share of the model's cut least far along, a run not yet counting reading;
    /// `None` while no model's cuts are being taken out.
    pub fn progress(&self) -> Option<(Step, f32)> {
        least_along(self.runs.iter().map(|run| (run.job.counted(), &*run.read)))
    }

    /// Whether model `id`'s cuts are being taken out.
    pub fn drills(&self, id: ObjectId) -> bool {
        self.runs.iter().any(|run| run.model == id)
    }

    /// Gives up taking the cuts out: each model's are left to the winding rule, as where the
    /// engine refused it, and not taken out again until they or its mesh change.
    pub fn give_up(&mut self, scene: &mut Scene) {
        for run in self.runs.drain(..) {
            let (Some(source), Some(cuts)) = (run.source.upgrade(), run.cuts.upgrade()) else {
                continue;
            };
            if let Some(object) = scene.get_mut(run.model)
                && Arc::ptr_eq(object.uncut(), &source)
            {
                object.hollow.keep_drilled(&source, &cuts, None);
            }
        }
    }

    /// Whether any model's cuts are being taken out, or its printed supports built.
    pub fn is_running(&self) -> bool {
        !self.runs.is_empty() || !self.support_runs.is_empty()
    }
}

/// The step and share of the run least far along of `runs`, each with the model read it waits
/// on: a run reading stands where that read or a read of its own has got, whichever is further.
pub fn least_along<'a>(
    runs: impl Iterator<Item = (Option<Counted>, &'a ModelRead)>,
) -> Option<(Step, f32)> {
    runs.map(|(counted, read)| {
        let waited_on = read.progress().share().map_or(0.0, |(_, share)| share);
        match counted {
            Some(counted) if counted.step != Step::Reading => (counted.step, counted.share),
            own => {
                let own = own.map_or(0.0, |counted| counted.share);
                (Step::Reading, own.max(waited_on))
            }
        }
    })
    .min_by(|(_, one), (_, other)| one.total_cmp(other))
}

#[cfg(test)]
mod tests;
