//! The support trees of every model, grown off the drawing thread: merging a thousand tips
//! into trunks takes a second or more on a dense model, and a frame waiting on it freezes
//! the window (ADR 0340).

use std::sync::Arc;

use core_supports::Grown;
use printer_profiles::SupportProfile;

use crate::job::{Worker, pooled};
use crate::scene::{ObjectId, Scene, SceneObject};

/// The trees being grown, one run a model at most.
#[derive(Debug, Default)]
pub struct SupportGrowth {
    runs: Vec<Run>,
}

#[derive(Debug)]
struct Run {
    model: ObjectId,
    job: Worker<Option<Grown>>,
}

impl SupportGrowth {
    /// Starts growing the trees of `object` to `table` where they need it and no run for
    /// it is under way. One that is goes on to its end, so a model dragged across the plate
    /// is grown for where it stands when the run lands rather than for every frame.
    pub fn start(&mut self, object: &SceneObject, table: &[SupportProfile]) {
        if self.runs.iter().any(|run| run.model == object.id) {
            return;
        }
        let Some(growth) = object.supports.growth(object.transform, table) else {
            return;
        };
        let model = Arc::clone(&object.mesh);
        let (surface, bvh) = object.support_surface();
        let (surface, bvh) = (Arc::clone(surface), Arc::clone(bvh));
        self.runs.push(Run {
            model: object.id,
            job: Worker::spawn(
                move |_, _| pooled(|| Some(growth.grow(&model, &surface, &bvh)), |_| None),
                || None,
            ),
        });
    }

    /// Hands every tree that has been grown to its model, which turns it away if its
    /// supports have changed since. Returns whether any run is still going.
    pub fn poll(&mut self, scene: &mut Scene) -> bool {
        self.runs.retain_mut(|run| {
            let Some(grown) = run.job.poll() else {
                return true;
            };
            if let (Some(grown), Some(object)) = (grown, scene.get_mut(run.model)) {
                object.supports.take_grown(grown);
            }
            false
        });
        !self.runs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Transform, Vec3, cuboid, diagnose};

    use super::*;
    use crate::scene::{ImportSummary, Imported};

    /// A 10 mm cube whose underside hangs 5 mm over the plate, with one support under it.
    fn supported_cube() -> Scene {
        let cube = cuboid(Vec3::splat(-5.0), Vec3::splat(5.0));
        let summary = ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: core_geometry::Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&cube),
        };
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
        while growth.poll(scene) {
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
}
