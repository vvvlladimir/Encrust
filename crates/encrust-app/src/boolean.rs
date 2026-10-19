use anyhow::{Result, bail};
use core_geometry::{Scalar, transform_mesh};
use core_volume::Boolean;

use crate::i18n::tr;
use crate::job::{BooleanJob, BooleanOutcome, BooleanRequest, Method};
use crate::landing;
use crate::scene::{Scene, SceneObject, Taken};
use crate::status::Status;

/// Union, difference and intersection of the models picked on the plate, replacing them
/// with the one they make. See `docs/design/csg.md`.
#[derive(Debug, Default)]
pub struct BooleanTool {
    pub op: Boolean,
    pub job: Option<BooleanJob>,
}

impl BooleanTool {
    /// Why Combine is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<String> {
        (picked(scene).len() < 2).then(|| tr!("boolean-pick-two"))
    }

    /// Starts combining the picked models, in the order they were picked.
    pub fn start(&mut self, scene: &Scene) -> Result<()> {
        let models = picked(scene);
        if models.len() < 2 {
            bail!(tr!("boolean-pick-two"));
        }
        self.job = Some(BooleanJob::spawn(BooleanRequest {
            taken: models.iter().map(|object| Taken::of(object)).collect(),
            // The model itself, not its shell: a cavity is not a solid the cut can read,
            // and the combination is hollowed again if it is to be.
            placed: models
                .iter()
                .map(|object| transform_mesh(&object.mesh, object.transform))
                .collect(),
            op: self.op,
            method: Method::Sampled,
        }));
        Ok(())
    }

    /// Lands a finished run in the scene. Returns whether one is still going, which is what
    /// tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        if let Some(landed) = land(&mut self.job, scene, |done| match done.voxel_mm {
            None => tr!(
                "boolean-done",
                name = done.name.to_owned(),
                count = done.count
            ),
            Some(voxel_mm) => tr!(
                "boolean-done-sampled",
                name = done.name.to_owned(),
                count = done.count,
                lattice = format!("{voxel_mm:.2}")
            ),
        }) {
            *status = landed;
        }
        self.job.is_some()
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// Gives up on the run going, which leaves the picked models as they are.
    pub fn cancel(&mut self, status: &mut Status) {
        if self.job.take().is_some() {
            *status = Status::Info(tr!("boolean-cancelled"));
        }
    }
}

/// What a finished combination did, for the stage notice to say.
pub(crate) struct Done<'a> {
    /// The first model taken, which the result is named after.
    pub name: &'a str,
    pub count: usize,
    /// The lattice it was sampled on, millimetres, when it could not be cut exactly.
    pub voxel_mm: Option<Scalar>,
}

/// Lands `job` once it has finished: its result stands on the plate in place of the models
/// it took, unless one of them has changed since. Answers the stage notice, `said` by the
/// tool that ran it, on the frame it lands.
pub(crate) fn land(
    job: &mut Option<BooleanJob>,
    scene: &mut Scene,
    said: impl FnOnce(&Done) -> String,
) -> Option<Status> {
    let outcome = job.as_mut()?.poll()?;
    let job = job.take()?;
    let combined = match outcome {
        BooleanOutcome::Combined(combined) => combined,
        BooleanOutcome::Failed(message) => return Some(Status::Error(message)),
    };
    let taken = job.taken();
    let [first, ..] = taken else {
        return None;
    };
    if let Err(changed) = landing::take_off(scene, taken) {
        return Some(Status::Error(changed));
    }
    let (id, mended) = landing::place(scene, first.plate, first.name.clone(), combined.mesh);
    // What it replaced may have stood on supports, which do not come with it.
    if let Some(object) = scene.get_mut(id) {
        object.stand_on_plate();
    }
    let said = said(&Done {
        name: &first.name,
        count: taken.len(),
        voxel_mm: combined.voxel_mm,
    });
    let notes = landing::mending(usize::from(mended)).into_iter().collect();
    Some(Status::noted(said, notes))
}

/// The names of the models a combination would take, in the order they were picked.
pub(crate) fn picked_names(scene: &Scene) -> Vec<String> {
    picked(scene)
        .into_iter()
        .map(|object| object.name.clone())
        .collect()
}

/// The picked models a combination would take, in the order they were picked: only those
/// on the plate being edited and not hidden.
fn picked(scene: &Scene) -> Vec<&SceneObject> {
    scene
        .selection()
        .iter()
        .filter_map(|id| scene.targets().find(|object| object.id == *id))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_geometry::{Transform, Vec3, cuboid, diagnose, signed_volume};

    use super::*;
    use crate::scene::{ImportSummary, Imported, ObjectId};

    fn cube_at(scene: &mut Scene, name: &str, at: Vec3) -> ObjectId {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(20.0));
        scene.insert(Imported::new(
            name.to_owned(),
            Arc::new(mesh.clone()),
            Transform {
                translation: at,
                ..Transform::default()
            },
            ImportSummary::made(diagnose(&mesh)),
        ))
    }

    /// Two 20 mm cubes overlapping in a 10 mm cube, both picked, the first one first.
    fn two_picked() -> (Scene, ObjectId, ObjectId) {
        let mut scene = Scene::default();
        let first = cube_at(&mut scene, "first", Vec3::ZERO);
        let second = cube_at(&mut scene, "second", Vec3::splat(10.0));
        scene.select_many(&[first, second]);
        (scene, first, second)
    }

    fn run(tool: &mut BooleanTool, scene: &mut Scene) -> Status {
        tool.start(scene).expect("two models are picked");
        let mut status = Status::default();
        while tool.poll(scene, &mut status) {
            std::thread::yield_now();
        }
        status
    }

    #[test]
    fn a_difference_replaces_the_picked_models_with_the_first_less_the_second() {
        let (mut scene, first, second) = two_picked();
        let mut tool = BooleanTool {
            op: Boolean::Difference,
            ..BooleanTool::default()
        };
        let status = run(&mut tool, &mut scene);
        assert!(matches!(status, Status::Info(_)), "{status:?}");
        assert!(scene.get(first).is_none() && scene.get(second).is_none());
        let [combined] = scene.objects() else {
            panic!("one model is left on the plate");
        };
        assert_eq!(combined.name, "first", "it is named after what it kept");
        let volume = signed_volume(&transform_mesh(&combined.mesh, combined.transform));
        assert!(
            (volume - 7_000.0).abs() < 1e-1,
            "8000 mm³ less the 1000 shared; got {volume}"
        );
    }

    #[test]
    fn one_picked_model_is_not_enough_to_combine() {
        let (mut scene, first, _) = two_picked();
        scene.select(Some(first));
        let tool = BooleanTool::default();
        assert_eq!(tool.blocker(&scene), Some(tr!("boolean-pick-two")));
    }

    #[test]
    fn models_that_share_nothing_intersect_in_an_error_and_stay_on_the_plate() {
        let mut scene = Scene::default();
        let first = cube_at(&mut scene, "first", Vec3::ZERO);
        let second = cube_at(&mut scene, "second", Vec3::splat(50.0));
        scene.select_many(&[first, second]);
        let mut tool = BooleanTool {
            op: Boolean::Intersection,
            ..BooleanTool::default()
        };
        let status = run(&mut tool, &mut scene);
        assert_eq!(status, Status::Error(tr!("boolean-nothing-left")));
        assert_eq!(scene.objects().len(), 2, "nothing was replaced");
    }

    #[test]
    fn a_model_moved_while_it_was_combined_keeps_its_place_and_the_result_is_dropped() {
        let (mut scene, first, second) = two_picked();
        let mut tool = BooleanTool::default();
        tool.start(&scene).expect("two models are picked");
        scene
            .get_mut(first)
            .expect("the first cube is on the plate")
            .transform
            .translation = Vec3::splat(100.0);
        let mut status = Status::default();
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert_eq!(
            status,
            Status::Error(tr!("job-model-changed", name = "first"))
        );
        assert!(scene.get(first).is_some() && scene.get(second).is_some());
        assert_eq!(scene.objects().len(), 2, "nothing was replaced");
    }

    #[test]
    fn a_combination_lands_on_the_plate_it_was_taken_from_while_another_is_open() {
        let (mut scene, _, _) = two_picked();
        let mut tool = BooleanTool::default();
        tool.start(&scene).expect("two models are picked");
        let other = scene.add_plate();
        let mut status = Status::default();
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }
        assert!(matches!(status, Status::Info(_)), "{status:?}");
        assert_eq!(
            scene.on_plate(other).count(),
            0,
            "nothing lands on the plate opened"
        );
        assert_eq!(
            scene.on_plate(0).count(),
            1,
            "the union replaces the two it took"
        );
    }

    #[test]
    fn a_combination_of_lifted_models_stands_on_the_plate() {
        let mut scene = Scene::default();
        let first = cube_at(&mut scene, "first", Vec3::new(0.0, 0.0, 5.0));
        let second = cube_at(&mut scene, "second", Vec3::new(10.0, 10.0, 5.0));
        scene.select_many(&[first, second]);
        let status = run(&mut BooleanTool::default(), &mut scene);
        assert!(matches!(status, Status::Info(_)), "{status:?}");
        let [combined] = scene.objects() else {
            panic!("one model is left on the plate");
        };
        let bounds = combined.world_bounds().expect("the union has faces");
        assert!(
            bounds.mins.z.abs() < 1e-4,
            "both cubes stood 5 mm up, as supports lift them; got {}",
            bounds.mins.z
        );
    }
}
