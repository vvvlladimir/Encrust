//! Every model's cuts taken out of it exactly, off the drawing thread (ADR 0322).

use std::sync::{Arc, Weak};

use core_geometry::Mesh;

use crate::i18n::tr;
use crate::job::{DrillJob, DrillRequest, spawn_drill};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

/// The cuts being taken out, one run a model at a time.
#[derive(Debug, Default)]
pub struct Drilling {
    runs: Vec<Run>,
}

/// A run for one model, and the meshes it took, held weakly so a replaced one is let go.
#[derive(Debug)]
struct Run {
    model: ObjectId,
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
            let job = spawn_drill(DrillRequest {
                source: Arc::clone(source),
                cavity: object.hollow.cavity_faces(),
                cuts: Arc::clone(cuts),
            });
            self.runs.push(Run {
                model: object.id,
                source: Arc::downgrade(source),
                cuts: Arc::downgrade(cuts),
                job,
            });
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
            if outcome.is_err() {
                *status = Status::Info(tr!("drill-refused", name = object.name.clone()));
            }
            object.hollow.keep_drilled(&source, &cuts, outcome.ok());
        }
        self.is_running()
    }

    /// Whether any model's cuts are being taken out.
    pub fn is_running(&self) -> bool {
        !self.runs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Bvh, Transform, Vec3, cuboid, diagnose, signed_volume};
    use core_volume::HoleSize;

    use super::*;
    use crate::scene::{ImportSummary, Imported};

    /// A 10 mm block with a 3 mm hole 5 mm down into the middle of its top.
    fn a_drilled_block() -> (Scene, ObjectId) {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let mut scene = Scene::default();
        let id = scene.insert(Imported::new(
            "block".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary::made(diagnose(&mesh)),
        ));
        let object = scene.get_mut(id).expect("the block is on the plate");
        let bvh = Bvh::build(&mesh);
        let size = HoleSize::round(3.0, 5.0, 1.0);
        let at = Vec3::new(5.0, 5.0, 10.0);
        object
            .hollow
            .add_drain(&mesh, &bvh, at, Vec3::Z, size, Transform::default());
        (scene, id)
    }

    /// Runs frames until no cut is being taken out, as the frames after an edit do.
    fn settle(drilling: &mut Drilling, scene: &mut Scene, status: &mut Status) {
        for _ in 0..10_000 {
            drilling.start(scene, true);
            if !drilling.poll(scene, status) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("the cuts were still being taken out after ten seconds");
    }

    #[test]
    fn a_hole_is_taken_out_of_the_block_it_was_drilled_in() {
        let (mut scene, id) = a_drilled_block();
        let mut drilling = Drilling::default();
        settle(&mut drilling, &mut scene, &mut Status::default());

        let object = scene.get(id).expect("the block");
        let volume = signed_volume(object.printed());
        assert!(
            volume < 1000.0 - 30.0,
            "the 1000 mm3 block less a 3 mm by 5 mm bore of some 35 mm3, got {volume}"
        );
        assert!(!object.hollow.wants_drilling(object.uncut()), "and kept");
    }

    #[test]
    fn nothing_is_taken_out_in_the_middle_of_a_drag() {
        let (mut scene, _) = a_drilled_block();
        let mut drilling = Drilling::default();
        drilling.start(&scene, false);
        assert!(
            !drilling.is_running(),
            "nothing starts while the pointer is down"
        );
        settle(&mut drilling, &mut scene, &mut Status::default());
    }

    #[test]
    fn a_cut_for_holes_since_changed_is_not_kept() {
        let (mut scene, id) = a_drilled_block();
        let mut drilling = Drilling::default();
        drilling.start(&scene, true);
        scene.get_mut(id).expect("the block").hollow.clear_drains();
        while drilling.poll(&mut scene, &mut Status::default()) {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let object = scene.get(id).expect("the block");
        assert!(
            Arc::ptr_eq(object.printed(), &object.mesh),
            "a block with no hole left prints as it came"
        );
    }

    #[test]
    fn an_undo_step_keeps_no_drilled_mesh() {
        let (mut scene, id) = a_drilled_block();
        let mut drilling = Drilling::default();
        settle(&mut drilling, &mut scene, &mut Status::default());

        let kept = scene.snapshot();
        let object = kept.get(id).expect("the block");
        assert!(
            object.hollow.wants_drilling(object.uncut()),
            "the step drills again"
        );
        let mut put_back = kept.clone();
        put_back.adopt_drilled(&scene);
        let object = put_back.get(id).expect("the block");
        assert!(
            !object.hollow.wants_drilling(object.uncut()),
            "unless the plate it replaces still has it for the same cuts"
        );
    }
}
