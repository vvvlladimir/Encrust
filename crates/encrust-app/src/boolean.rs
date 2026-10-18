use anyhow::{Result, bail};
use core_geometry::{Mesh, transform_mesh};
use core_volume::Boolean;

use crate::i18n::tr;
use crate::job::{BooleanJob, BooleanOutcome, BooleanRequest};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// Union, difference and intersection of the models picked on the plate, replacing them
/// with the one they make. See `docs/design/csg.md`.
#[derive(Debug)]
pub struct BooleanTool {
    pub op: Boolean,
    pub job: Option<BooleanJob>,
}

impl Default for BooleanTool {
    fn default() -> Self {
        Self {
            op: Boolean::Union,
            job: None,
        }
    }
}

impl BooleanTool {
    /// Why Combine is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<String> {
        (picked(scene).len() < 2).then(|| tr!("boolean-pick-two"))
    }

    /// Starts combining the picked models, in the order they were picked.
    pub fn start(&mut self, scene: &Scene) -> Result<()> {
        let models = picked(scene);
        let [first, ..] = models.as_slice() else {
            bail!(tr!("boolean-pick-two"));
        };
        if models.len() < 2 {
            bail!(tr!("boolean-pick-two"));
        }
        self.job = Some(BooleanJob::spawn(BooleanRequest {
            ids: models.iter().map(|object| object.id).collect(),
            name: first.name.clone(),
            // The model itself, not its shell: a cavity is not a solid the cut can read,
            // and the combination is hollowed again if it is to be.
            placed: models
                .iter()
                .map(|object| transform_mesh(&object.mesh, object.transform))
                .collect(),
            op: self.op,
            exact: false,
        }));
        Ok(())
    }

    /// Lands a finished run in the scene. Returns whether one is still going, which is what
    /// tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let Some(outcome) = self.job.as_mut().and_then(BooleanJob::poll) else {
            return self.job.is_some();
        };
        self.job = None;
        *status = match outcome {
            BooleanOutcome::Combined {
                ids,
                name,
                mesh,
                voxel_mm,
            } => {
                let mending = land(scene, &ids, name.clone(), mesh);
                Status::Info(match voxel_mm {
                    None => tr!(
                        "boolean-done",
                        name = name,
                        count = ids.len(),
                        mending = mending
                    ),
                    Some(voxel_mm) => tr!(
                        "boolean-done-sampled",
                        name = name,
                        count = ids.len(),
                        lattice = format!("{voxel_mm:.2}"),
                        mending = mending
                    ),
                })
            }
            BooleanOutcome::Failed(message) => Status::Error(message),
        };
        false
    }

    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }
}

/// Puts `mesh` on the plate in place of the models `ids`, and answers what the stage notice
/// adds about mending it.
pub(crate) fn land(scene: &mut Scene, ids: &[ObjectId], name: String, mesh: Mesh) -> String {
    for id in ids {
        scene.remove(*id);
    }
    let (_, mended) = crate::cut::place(scene, name, mesh);
    crate::cut::mending(usize::from(mended))
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
    use crate::scene::{ImportSummary, Imported};

    fn cube_at(scene: &mut Scene, name: &str, at: Vec3) -> ObjectId {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(20.0));
        scene.insert(Imported::new(
            name.to_owned(),
            Arc::new(mesh.clone()),
            Transform {
                translation: at,
                ..Transform::default()
            },
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: core_geometry::Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
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
}
