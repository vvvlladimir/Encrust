use std::sync::Arc;

use anyhow::{Result, bail};
use core_geometry::{Adjacency, Mesh, Vec3, drop_to_plate};
use core_plate::OrientSettings;
use core_supports::Region;

use crate::job::{OrientJob, OrientOutcome, OrientRequest, orient_tasks};
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// A turn this small is the model already standing the way it should.
const SETTLED_DEG: f32 = 0.5;

/// How far a neighbouring triangle may lean from the one pointed at and still be part of
/// the same flat face, degrees. Tessellation leaves a flat face within a hair of this.
const FACET_DEG: f32 = 1.0;

/// What the auto-orient button is set to, and the run it has going.
#[derive(Default)]
pub struct OrientTool {
    pub settings: OrientSettings,
    pub job: Option<OrientJob>,
    /// The next click on a model lays the face under it on the plate.
    pub picking_face: bool,
    /// The flat face under the cursor while one is being picked.
    pub facet: Option<Facet>,
    /// The edges of the last mesh pointed at, which finding a face walks. Built once per
    /// mesh, since a mesh of a million faces takes a moment.
    adjacency: Option<(usize, Arc<Adjacency>)>,
}

/// The flat face a click would lay on the plate: every triangle joined to the one
/// pointed at within `FACET_DEG`, and the way they face together.
#[derive(Debug, Clone)]
pub struct Facet {
    pub id: ObjectId,
    mesh: usize,
    pub faces: Vec<usize>,
    /// Area-weighted, in the model's own space.
    pub normal: Vec3,
}

impl Facet {
    fn of(id: ObjectId, mesh: &Arc<Mesh>, adjacency: &Adjacency, seed: usize) -> Option<Self> {
        let mut region = Region::default();
        region.flood(mesh, adjacency, seed, FACET_DEG, true);
        let faces: Vec<usize> = region.faces().collect();
        let normal = faces
            .iter()
            .filter_map(|face| mesh.triangle(*face))
            .map(|triangle| triangle.normal_unnormalized())
            .sum::<Vec3>()
            .try_normalize()?;
        Some(Self {
            id,
            mesh: Arc::as_ptr(mesh) as usize,
            faces,
            normal,
        })
    }
}

impl OrientTool {
    pub fn is_running(&self) -> bool {
        self.job.is_some()
    }

    /// Points at triangle `seed` of `object`, and keeps the flat face it belongs to as
    /// `facet`. Walks the face again only when the pointer has left the last one.
    pub fn point_at(&mut self, object: &SceneObject, seed: usize) {
        let key = Arc::as_ptr(&object.mesh) as usize;
        let same = |facet: &Facet| {
            facet.id == object.id && facet.mesh == key && facet.faces.contains(&seed)
        };
        if self.facet.as_ref().is_some_and(same) {
            return;
        }
        let adjacency = match &self.adjacency {
            Some((built_for, adjacency)) if *built_for == key => Arc::clone(adjacency),
            _ => {
                let adjacency = Arc::new(Adjacency::of(&object.mesh));
                self.adjacency = Some((key, Arc::clone(&adjacency)));
                adjacency
            }
        };
        self.facet = Facet::of(object.id, &object.mesh, &adjacency, seed);
    }

    /// Stops waiting for a face, and lets go of what finding one kept.
    pub fn stop_picking(&mut self) {
        self.picking_face = false;
        self.facet = None;
        self.adjacency = None;
    }

    /// Why the button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<&'static str> {
        if scene.target_count() == 0 {
            return Some("Nothing visible on the plate to orient.");
        }
        None
    }

    /// Starts a run over one model, or over everything visible when `only` is `None`.
    pub fn start(&mut self, scene: &Scene, only: Option<ObjectId>) -> Result<()> {
        let tasks = orient_tasks(scene, only);
        if tasks.is_empty() {
            bail!("nothing visible on the plate to orient");
        }

        self.job = Some(OrientJob::spawn(OrientRequest {
            tasks,
            settings: self.settings,
        }));
        Ok(())
    }

    /// Drains a running run into the scene. Returns whether one is still going, which is
    /// what tells the window to keep repainting.
    ///
    /// A turn replaces the model's rotation rather than adding to it — the search
    /// measures the mesh in its own space — and the model is dropped back onto the plate
    /// afterwards, because turning it leaves it hanging or buried.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };

        self.job = None;
        match outcome {
            OrientOutcome::Turned(turns) => {
                let mut moved = 0;
                for turn in turns {
                    if turn.angle_deg > SETTLED_DEG {
                        moved += 1;
                    }
                    if let Some(object) = scene.get_mut(turn.id) {
                        object.transform.rotation = turn.rotation;
                    }
                    let Some(bounds) = scene.get(turn.id).and_then(|o| o.world_bounds()) else {
                        continue;
                    };
                    if let Some(object) = scene.get_mut(turn.id) {
                        object.transform.translation += drop_to_plate(&bounds);
                    }
                }
                *status = Status::Info(match moved {
                    0 => "Everything already stands the way it prints best".to_owned(),
                    1 => "Turned 1 model".to_owned(),
                    moved => format!("Turned {moved} models"),
                });
            }
            OrientOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{ImportSummary, Imported};
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
    use std::sync::Arc;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    fn scene_with(mesh: Mesh) -> Scene {
        let mut scene = Scene::default();
        scene.insert(Imported::new(
            "slab".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        scene
    }

    /// Runs a job to its end, the way the window's frame loop would.
    fn settle(tool: &mut OrientTool, scene: &mut Scene, status: &mut Status) {
        while tool.poll(scene, status) {
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_turned_model_lands_back_on_the_plate() {
        let mut scene = scene_with(cuboid(Vec3::new(40.0, 40.0, 4.0)));
        let mut tool = OrientTool::default();
        let mut status = Status::default();

        tool.start(&scene, None)
            .expect("a visible slab can be oriented");
        settle(&mut tool, &mut scene, &mut status);

        let bounds = scene.world_bounds().expect("the slab is still there");
        assert!(
            bounds.mins.z.abs() < 1e-3,
            "the model stands on the plate, not through it, got {}",
            bounds.mins.z
        );
        assert!(!status.is_error(), "the run finished");
    }

    #[test]
    fn an_empty_plate_has_nothing_to_orient() {
        let mut tool = OrientTool::default();
        let scene = Scene::default();
        assert!(tool.blocker(&scene).is_some());
        assert!(tool.start(&scene, None).is_err());
    }

    #[test]
    fn a_hidden_model_is_not_turned() {
        let mut scene = scene_with(cuboid(Vec3::splat(10.0)));
        scene.objects_mut()[0].visible = false;
        let tool = OrientTool::default();
        assert!(tool.blocker(&scene).is_some());
    }

    #[test]
    fn pointing_at_one_triangle_of_a_cube_side_finds_the_whole_side() {
        let scene = scene_with(cuboid(Vec3::splat(10.0)));
        let object = &scene.objects()[0];
        let mut tool = OrientTool::default();
        // Face 0 is one of the two triangles of the bottom, facing -Z.
        tool.point_at(object, 0);

        let facet = tool.facet.expect("a cube side is flat");
        assert_eq!(facet.faces.len(), 2, "a side of the cube is two triangles");
        assert!(
            facet.normal.abs_diff_eq(-Vec3::Z, 1e-5),
            "got {:?}",
            facet.normal
        );
    }
}
