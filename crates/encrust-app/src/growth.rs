//! The support trees of every model, grown off the drawing thread: merging a thousand tips
//! into trunks takes a second or more on a dense model (ADR 0340).

use std::sync::Arc;

use core_geometry::Transform;
use core_supports::Grown;
use printer_profiles::SupportProfile;

use crate::i18n::tr;
use crate::job::{Worker, pooled};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

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
    /// Starts growing the trees of `object` to `table` where they need it, no run for it is
    /// under way and none failed on the same. One under way goes on to its end, so a model
    /// dragged across the plate is grown for where it stands when the run lands.
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

    /// Hands every tree that has been grown to its model, which turns it away if its
    /// supports have changed since, and tells `status` of a run that failed. Returns
    /// whether any run is still going.
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

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid, diagnose};

    use super::*;
    use crate::scene::{ImportSummary, Imported};

    /// A 10 mm cube whose underside hangs 5 mm over the plate, with one support under it.
    fn supported_cube() -> Scene {
        let cube = cuboid(Vec3::splat(-5.0), Vec3::splat(5.0));
        let summary = ImportSummary::made(diagnose(&cube));
        let mut scene = Scene::default();
        let transform = Transform::from_translation(Vec3::new(0.0, 0.0, 10.0));
        let id = scene.insert(Imported::new(
            "cube".to_owned(),
            Arc::new(cube),
            transform,
            summary,
        ));
        let object = scene.get_mut(id).expect("just inserted");
        object.supports.add(Vec3::new(0.0, 0.0, 5.0), transform, 0);
        scene
    }

    fn settle(growth: &mut SupportGrowth, scene: &mut Scene) {
        while growth.poll(scene, &mut Status::Idle) {
            std::thread::yield_now();
        }
    }

    #[test]
    fn grown_trees_land_on_their_model_and_build_without_growing_again() {
        let mut scene = supported_cube();
        let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
        let mut growth = SupportGrowth::default();

        growth.start(&scene.objects()[0], &table);
        settle(&mut growth, &mut scene);

        let object = &mut scene.objects_mut()[0];
        assert!(!object.supports.needs_growing(object.transform, &table));
        object.refresh_supports(&table);
        assert_eq!(object.supports.standing(), 1, "the one support stands");
    }

    #[test]
    fn a_model_moved_while_its_trees_grow_holds_its_supports_pending() {
        let mut scene = supported_cube();
        let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
        let mut growth = SupportGrowth::default();
        growth.start(&scene.objects()[0], &table);
        settle(&mut growth, &mut scene);
        let object = &mut scene.objects_mut()[0];
        object.refresh_supports(&table);
        assert!(!object.supports_pending());

        object.transform = Transform::from_translation(Vec3::new(20.0, 0.0, 10.0));
        assert!(object.supports.outgrown(object.transform, &table));
        assert!(
            object.supports_pending(),
            "what stood where the model was is not sliced"
        );
    }

    #[test]
    fn trees_grown_for_supports_since_changed_are_grown_again() {
        let mut scene = supported_cube();
        let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
        let mut growth = SupportGrowth::default();

        growth.start(&scene.objects()[0], &table);
        let object = &mut scene.objects_mut()[0];
        object
            .supports
            .add(Vec3::new(3.0, 0.0, 5.0), object.transform, 0);
        settle(&mut growth, &mut scene);

        let object = &scene.objects()[0];
        assert!(object.supports.needs_growing(object.transform, &table));
    }

    #[test]
    fn a_failed_growth_is_reported_and_not_tried_again_until_the_supports_change() {
        let mut scene = supported_cube();
        let table = [SupportProfile::shipped("medium").expect("the medium preset ships")];
        let object = &scene.objects()[0];
        let asked = Asked {
            model: object.id,
            revision: object.supports.revision(),
            transform: object.transform,
            table: table.to_vec(),
        };
        let mut growth = SupportGrowth::default();
        growth.runs.push(Run {
            asked,
            job: Worker::spawn(|_, _| Err(Some("out of memory".to_owned())), || Err(None)),
        });
        let mut status = Status::Idle;
        while growth.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert!(status.is_error(), "the failure is told: {status:?}");

        growth.start(&scene.objects()[0], &table);
        assert!(growth.runs.is_empty(), "the same asking is not run again");

        let object = &mut scene.objects_mut()[0];
        object
            .supports
            .add(Vec3::new(3.0, 0.0, 5.0), object.transform, 0);
        growth.poll(&mut scene, &mut status);
        growth.start(&scene.objects()[0], &table);
        assert_eq!(growth.runs.len(), 1, "an edit is grown again");
    }
}
