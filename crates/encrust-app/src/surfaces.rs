//! The hierarchy of every surface supports stand against, built off the drawing thread: a
//! dense shell takes a while, and a frame waiting on it freezes the window (ADR 0334).

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

    /// Keeps each hierarchy that has been built where its model still prints as that
    /// surface, so its supports grow against it again, and tells `status` of a run that
    /// failed. Returns whether any is still going.
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
mod tests {
    use core_geometry::{Transform, Vec3, cuboid, diagnose};

    use super::*;
    use crate::scene::{ImportSummary, Imported};

    fn hollow_block() -> (Scene, ObjectId) {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let mut scene = Scene::default();
        let id = scene.insert(Imported::new(
            "block".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary::made(diagnose(&mesh)),
        ));
        let object = scene.get_mut(id).expect("the block is on the plate");
        let settings = object
            .hollow
            .asking(&core_volume::HollowSettings::for_wall(1.0));
        let mut shell = mesh.clone();
        shell.append(&cuboid(Vec3::splat(1.0), Vec3::splat(9.0)));
        object.hollow.take(core_volume::Shell {
            mesh: Arc::new(shell),
            cavity: 12..24,
            cavity_mm3: 512.0,
            voxel_mm: 0.05,
            coarsened: false,
            shape: object.transform.unturned(),
            settings,
        });
        (scene, id)
    }

    fn settle(surfaces: &mut SupportSurfaces, scene: &mut Scene) {
        surfaces.start(scene);
        while surfaces.poll(scene, &mut Status::Idle) {
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_shell_stands_supports_against_it_once_its_hierarchy_is_built_on_a_worker() {
        let (mut scene, id) = hollow_block();
        let shell = Arc::clone(
            scene
                .get(id)
                .and_then(|object| object.hollow.shell())
                .expect("the block is hollow"),
        );
        let mut surfaces = SupportSurfaces::default();
        surfaces.start(&mut scene);
        let object = scene.get(id).expect("the block is on the plate");
        assert!(
            !Arc::ptr_eq(object.support_surface().0, &shell),
            "the model itself stands in while the shell's hierarchy is built"
        );
        settle(&mut surfaces, &mut scene);
        let object = scene.get(id).expect("the block is on the plate");
        assert!(
            Arc::ptr_eq(object.support_surface().0, &shell),
            "the shell once its hierarchy is built"
        );
        assert!(object.supported_inside(), "and its cavity is known");
    }

    #[test]
    fn a_model_solid_again_lets_its_shell_go_on_the_next_frame() {
        let (mut scene, id) = hollow_block();
        let mut surfaces = SupportSurfaces::default();
        settle(&mut surfaces, &mut scene);
        let object = scene.get_mut(id).expect("the block is on the plate");
        object.hollow.clear();
        surfaces.start(&mut scene);
        let object = scene.get(id).expect("the block is on the plate");
        assert!(
            !object.supported_inside(),
            "a solid model has no cavity to hold up"
        );
        assert!(
            !surfaces.poll(&mut scene, &mut Status::Idle),
            "and nothing is left to build"
        );
    }

    #[test]
    fn a_surface_that_failed_to_build_is_reported_and_not_built_again() {
        let (mut scene, id) = hollow_block();
        let shell = Arc::clone(
            scene
                .get(id)
                .and_then(|object| object.hollow.shell())
                .expect("the block is hollow"),
        );
        let mut surfaces = SupportSurfaces::default();
        surfaces.runs.push(Run {
            model: id,
            mesh: Arc::downgrade(&shell),
            job: Worker::spawn(|_, _| Err(Some("out of memory".to_owned())), || Err(None)),
        });
        let mut status = Status::Idle;
        while surfaces.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert!(status.is_error(), "the failure is told: {status:?}");

        surfaces.start(&mut scene);
        assert!(
            surfaces.runs.is_empty(),
            "the same surface is not built again"
        );
    }
}
