use std::sync::Arc;

use anyhow::{Result, bail};
use core_geometry::{Adjacency, Mesh, Vec2, Vec3};
use core_plate::{OrientPreset, OrientSettings};
use core_supports::Region;
use printer_profiles::{MaterialProfile, PrinterProfile, SupportProfile};

use crate::i18n::tr;
use crate::job::{
    Costs, OrientJob, OrientOutcome, OrientRequest, OrientTask, Pose, Proof, Turn, orient_tasks,
};
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::{Reading, Status};

/// A turn this small is the model already standing the way it should.
const SETTLED_DEG: f32 = 0.5;

/// How far the way down may differ from a pose's and the model still stand in it: what a
/// placement's rounding leaves.
const STANDING_EPS: f32 = 1e-4;

/// How far a neighbouring triangle may lean from the one pointed at and still be part of
/// the same flat face, degrees. Tessellation leaves a flat face within a hair of this.
const FACET_DEG: f32 = 1.0;

/// What the auto-orient button is set to, and the run it has going.
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
    /// The good ways to stand each model the last run turned, and which one it stands in.
    offers: Vec<(Turn, usize)>,
    /// What the last run was asked, which the button asked the same again steps through.
    asked: Option<Asked>,
}

impl Default for OrientTool {
    fn default() -> Self {
        Self {
            settings: OrientSettings {
                preset: OrientPreset::Auto,
                ..OrientSettings::default()
            },
            job: None,
            picking_face: false,
            facet: None,
            adjacency: None,
            offers: Vec::new(),
            asked: None,
        }
    }
}

/// The models a run was asked about, and what it judged them by.
struct Asked {
    tasks: Vec<OrientTask>,
    settings: OrientSettings,
    proof: Option<Proof>,
}

impl Asked {
    fn same_as(&self, other: &Self) -> bool {
        self.settings == other.settings
            && self.proof == other.proof
            && self.tasks.len() == other.tasks.len()
            && self
                .tasks
                .iter()
                .zip(&other.tasks)
                .all(|(one, two)| one.asks_as(two))
    }
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
    pub fn blocker(&self, scene: &Scene) -> Option<String> {
        if scene.target_count() == 0 {
            return Some(tr!("orient-nothing-visible"));
        }
        None
    }

    /// Turns one model, or everything visible when `only` is `None`, the way it prints
    /// best, and stops waiting for a face: the run replaces the rotation a laid face would
    /// set. Asked the same of the same models still standing as the last run left them,
    /// it stands each the next good way that run found instead of searching again.
    ///
    /// Each model is judged as standing on supports built to `support_of` unless it needs
    /// none. With a `machine` the search's best few are printed for it and the best taken.
    pub fn start(
        &mut self,
        scene: &mut Scene,
        plate: &BuildPlate,
        only: Option<ObjectId>,
        support_of: impl Fn(&SceneObject) -> SupportProfile,
        machine: Option<(&PrinterProfile, &MaterialProfile)>,
        status: &mut Status,
    ) -> Result<()> {
        let tasks = orient_tasks(scene, only, support_of);
        if tasks.is_empty() {
            bail!(tr!("orient-nothing-visible-error"));
        }

        self.stop_picking();
        let asked = Asked {
            tasks,
            settings: OrientSettings {
                on_supports: true,
                rest_on_plate: true,
                build_volume_mm: machine.map(|(printer, _)| {
                    let volume = &printer.build_volume;
                    Vec3::new(volume.x, volume.y, volume.z)
                }),
                ..self.settings
            },
            proof: machine.map(|(printer, material)| Proof {
                printer: printer.clone(),
                material: material.clone(),
            }),
        };
        if self.found_for(&asked) {
            *status = self.step(scene, plate);
            return Ok(());
        }

        self.offers.clear();
        self.job = Some(OrientJob::spawn(OrientRequest {
            tasks: asked.tasks.clone(),
            settings: asked.settings,
            proof: asked.proof.clone(),
        }));
        self.asked = Some(asked);
        Ok(())
    }

    /// Whether the last run was asked `asked` and every model stands as it left it.
    fn found_for(&self, asked: &Asked) -> bool {
        let standing = |task: &OrientTask| {
            self.offers.iter().any(|(turn, at)| {
                let down = turn.poses[*at].rotation.inverse() * Vec3::NEG_Z;
                turn.id == task.id && down.abs_diff_eq(task.down_now, STANDING_EPS)
            })
        };
        self.asked.as_ref().is_some_and(|last| last.same_as(asked))
            && asked.tasks.iter().all(standing)
    }

    /// Drains a running run into the scene. Returns whether one is still going, which is
    /// what tells the window to keep repainting.
    ///
    /// A turn replaces the model's rotation rather than adding to it — the search
    /// measures the mesh in its own space — and the model is stood back where it was on
    /// `plate`, because turning it leaves it hanging, buried or off to one side.
    pub fn poll(&mut self, scene: &mut Scene, plate: &BuildPlate, status: &mut Status) -> bool {
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
                for turn in &turns {
                    if let Some(object) = scene.get_mut(turn.id) {
                        moved += usize::from(stand(object, turn.poses[0], turn.lift_mm, plate));
                    }
                }
                *status = turned(&turns, moved);
                self.offers = turns.into_iter().map(|turn| (turn, 0)).collect();
            }
            OrientOutcome::Failed(message) => {
                self.asked = None;
                *status = Status::Error(message);
            }
        }
        false
    }

    /// Stands every model the last run turned the next good way it found, round from the
    /// last to the first, and says what that one costs against the one before.
    fn step(&mut self, scene: &mut Scene, plate: &BuildPlate) -> Status {
        let alone = self.offers.len() == 1;
        let mut stepped = 0;
        let mut said = None;
        for (turn, at) in &mut self.offers {
            let count = turn.poses.len();
            if count < 2 {
                continue;
            }
            let was = turn.poses[*at];
            *at = (*at + 1) % count;
            let pose = turn.poses[*at];
            if let Some(object) = scene.get_mut(turn.id) {
                stand(object, pose, turn.lift_mm, plate);
                stepped += 1;
            }
            if alone {
                let headline = tr!("orient-pose", index = *at + 1, count = count);
                said = Some(costed(headline, pose.costs, was.costs));
            }
        }
        match (said, stepped) {
            (Some(said), _) => said,
            (None, stepped) => Status::Info(tr!("orient-stepped", count = stepped)),
        }
    }
}

/// What a run did: how many models it turned, how many stand on the plate, how many other
/// good poses the button steps through next, and what one model now costs.
fn turned(turns: &[Turn], moved: usize) -> Status {
    let seated = turns.iter().filter(|turn| turn.poses[0].on_plate).count();
    let others: usize = turns.iter().map(|turn| turn.poses.len() - 1).sum();
    let turned = tr!("orient-turned", count = moved);
    let turned = match seated {
        0 => turned,
        seated => tr!("orient-seated", turned = turned, count = seated),
    };
    let turned = match others {
        0 => turned,
        others => tr!("orient-others", turned = turned, count = others),
    };
    match turns {
        [turn] => costed(turned, turn.poses[0].costs, None),
        _ => Status::Info(turned),
    }
}

/// `headline` and what a pose costs, each figure against `was` where there was one.
fn costed(headline: String, costs: Option<Costs>, was: Option<Costs>) -> Status {
    let Some(costs) = costs else {
        return Status::Info(headline);
    };
    let reading = |now: fn(&Costs) -> f64, shown: fn(f64) -> String| {
        Reading::lower_better(now(&costs), was.as_ref().map(now), shown)
    };
    Status::Readings(
        headline,
        vec![
            reading(
                |costs| f64::from(costs.supports_ml),
                |ml| tr!("orient-supports-ml", value = format!("{ml:.2}")),
            ),
            reading(
                |costs| costs.pull_n,
                |n| tr!("orient-pull", value = format!("{n:.1}")),
            ),
            reading(
                |costs| f64::from(costs.stack_mm),
                |mm| tr!("orient-tall", value = format!("{mm:.1}")),
            ),
        ],
    )
}

/// Stands `object` the way `pose` says, over the middle it stood over, slid only as far as
/// keeps it on `plate`, dropped back onto it and lifted `lift_mm` onto its supports where it
/// is not set on it. Returns whether that turned it.
fn stand(object: &mut SceneObject, pose: Pose, lift_mm: f32, plate: &BuildPlate) -> bool {
    let middle = |object: &SceneObject| {
        object
            .world_bounds()
            .map(|bounds| ((bounds.mins + bounds.maxs) / 2.0).truncate())
    };
    let before = middle(object);
    let by = pose.rotation * object.transform.rotation.inverse();
    object.transform = object.transform.turned(by);
    if let Some((before, after)) = before.zip(middle(object)) {
        object.transform.translation += (before - after).extend(0.0);
    }
    object.slide_onto_plate(Vec2::new(plate.x_mm, plate.y_mm));
    object.stand_on_plate();
    if !pose.on_plate {
        object.transform.translation.z += lift_mm;
    }
    by.to_axis_angle().1.to_degrees() > SETTLED_DEG
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{ImportSummary, Imported};
    use crate::status::Change;
    use core_geometry::{Mesh, Orientation, Quat, Transform, Vec3, diagnose};
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
        while tool.poll(scene, &BuildPlate::default(), status) {
            std::thread::yield_now();
        }
    }

    /// Presses Auto-orient over the whole plate and waits for what it does.
    fn press(tool: &mut OrientTool, scene: &mut Scene, status: &mut Status) {
        let plate = BuildPlate::default();
        tool.start(
            scene,
            &plate,
            None,
            |_| SupportProfile::medium(),
            None,
            status,
        )
        .expect("a visible model can be oriented");
        settle(tool, scene, status);
    }

    #[test]
    fn a_model_oriented_again_after_a_change_is_not_turned_the_second_time() {
        let mut scene = scene_with(cuboid(Vec3::new(40.0, 40.0, 4.0)));
        let mut tool = OrientTool::default();
        let mut status = Status::default();
        press(&mut tool, &mut scene, &mut status);
        tool.settings.preset = core_plate::OrientGoal::FewestSupports.into();
        press(&mut tool, &mut scene, &mut status);
        assert!(
            matches!(&status, Status::Info(text) if text.starts_with("Everything already")),
            "the second run finds it standing as the first left it, got {status:?}"
        );
    }

    #[test]
    fn pressed_again_with_nothing_changed_it_steps_rather_than_searches() {
        let mut scene = scene_with(cuboid(Vec3::new(40.0, 20.0, 4.0)));
        let mut tool = OrientTool::default();
        let mut status = Status::default();
        press(&mut tool, &mut scene, &mut status);
        let plate = BuildPlate::default();

        tool.start(
            &mut scene,
            &plate,
            None,
            |_| SupportProfile::medium(),
            None,
            &mut status,
        )
        .expect("a visible slab can be oriented");
        assert!(!tool.is_running(), "nothing new to ask, so no run");

        scene.objects_mut()[0].lift_to(3.0);
        tool.start(
            &mut scene,
            &plate,
            None,
            |_| SupportProfile::medium(),
            None,
            &mut status,
        )
        .expect("a visible slab can be oriented");
        assert!(tool.is_running(), "a new lift is a new question");
        settle(&mut tool, &mut scene, &mut status);
    }

    #[test]
    fn a_turned_model_lands_back_on_the_plate() {
        let mut scene = scene_with(cuboid(Vec3::new(40.0, 40.0, 4.0)));
        let mut tool = OrientTool::default();
        let mut status = Status::default();

        press(&mut tool, &mut scene, &mut status);

        let bounds = scene.world_bounds().expect("the slab is still there");
        assert!(
            bounds.mins.z.abs() < 1e-3,
            "the model stands on the plate, not through it, got {}",
            bounds.mins.z
        );
        assert!(!status.is_error(), "the run finished");
    }

    #[test]
    fn a_turned_model_stays_over_the_spot_it_stood_on() {
        // A 6 x 6 x 80 column lying along X, its own origin at one end: the shortest arc
        // that stands it up swings it about that end, off the spot and past the plate.
        let mut scene = scene_with(cuboid(Vec3::new(6.0, 6.0, 80.0)));
        let id = scene.objects()[0].id;
        scene.objects_mut()[0].transform = Transform {
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            translation: Vec3::new(20.0, 40.0, 6.0),
            ..Transform::default()
        };
        let middle = |scene: &Scene| {
            let bounds = scene.world_bounds().expect("the column is there");
            ((bounds.mins + bounds.maxs) / 2.0).truncate()
        };
        let before = middle(&scene);
        let upright = Pose {
            rotation: Quat::IDENTITY,
            on_plate: true,
            costs: None,
        };
        let column = scene.get_mut(id).expect("the column is there");
        stand(column, upright, 0.0, &BuildPlate::default());

        let after = middle(&scene);
        assert!(
            after.abs_diff_eq(before, 1e-3),
            "{before} before, {after} after"
        );
        let bounds = scene.world_bounds().expect("the column is there");
        assert!(
            bounds.mins.z.abs() < 1e-3,
            "on the plate, got {}",
            bounds.mins.z
        );
    }

    #[test]
    fn a_model_turned_near_the_edge_is_slid_back_onto_the_plate() {
        // Standing 4 mm from the plate's left edge: laid on its side over the same middle,
        // the 80 mm column would reach 36 mm past that edge.
        let mut scene = scene_with(cuboid(Vec3::new(6.0, 6.0, 80.0)));
        let id = scene.objects()[0].id;
        scene.objects_mut()[0].transform.translation = Vec3::new(1.0, 30.0, 0.0);
        let on_side = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let lying = Pose {
            rotation: on_side,
            on_plate: true,
            costs: None,
        };
        let column = scene.get_mut(id).expect("the column is there");
        stand(column, lying, 0.0, &BuildPlate::default());

        let bounds = scene.world_bounds().expect("the column is there");
        let plate = BuildPlate::default();
        assert!(
            bounds.mins.x >= -1e-3 && bounds.maxs.x <= plate.x_mm + 1e-3,
            "inside the plate's width, got {:?}",
            bounds
        );
    }

    #[test]
    fn a_resized_model_is_judged_at_the_size_it_prints() {
        // A 100 mm cube squashed to 4 mm is a slab pulling some 98 N lying flat, which
        // the least peel turns off its face.
        let mut scene = scene_with(cuboid(Vec3::splat(100.0)));
        scene.objects_mut()[0].transform.scale = Vec3::new(1.0, 1.0, 0.04);
        let mut tool = OrientTool::default();
        tool.settings.preset = core_plate::OrientGoal::LeastPeel.into();
        let mut status = Status::default();

        press(&mut tool, &mut scene, &mut status);

        let bounds = scene.world_bounds().expect("the slab is still there");
        let height_mm = bounds.maxs.z - bounds.mins.z;
        assert!(
            height_mm > 10.0,
            "the 4 mm slab does not lie flat, got {height_mm}"
        );
    }

    #[test]
    fn stepping_through_the_offered_poses_turns_the_model_and_wraps_round() {
        let mut scene = scene_with(cuboid(Vec3::new(40.0, 20.0, 4.0)));
        let id = scene.objects()[0].id;
        let pose = |rotation, supports_ml| Pose {
            rotation,
            on_plate: true,
            costs: Some(Costs {
                supports_ml,
                pull_n: 5.0,
                stack_mm: 10.0,
            }),
        };
        let on_edge = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let mut tool = OrientTool {
            offers: vec![(
                Turn {
                    id,
                    poses: vec![pose(Quat::IDENTITY, 1.0), pose(on_edge, 2.0)],
                    lift_mm: 0.0,
                },
                0,
            )],
            ..OrientTool::default()
        };
        let plate = BuildPlate::default();

        let said = tool.step(&mut scene, &plate);
        let turned = scene.objects()[0].transform.rotation;
        assert!(
            turned.abs_diff_eq(on_edge, 1e-5),
            "the second pose, got {turned:?}"
        );
        let Status::Readings(headline, readings) = said else {
            panic!("a proven pose says what it costs, got {said:?}");
        };
        assert_eq!(headline, "Pose 2 of 2");
        assert_eq!(
            readings[0].change,
            Change::Worse,
            "2 ml of supports after 1"
        );
        assert_eq!(readings[1].change, Change::Same, "the same pull");

        tool.step(&mut scene, &plate);
        let turned = scene.objects()[0].transform.rotation;
        assert!(
            turned.abs_diff_eq(Quat::IDENTITY, 1e-5),
            "past the last, the first, got {turned:?}"
        );
    }

    #[test]
    fn a_lift_the_user_asked_for_stands_a_supported_pose_on_it() {
        let mut scene = scene_with(cuboid(Vec3::splat(10.0)));
        scene.objects_mut()[0].lift_to(2.5);
        let tasks = orient_tasks(&scene, None, |_| SupportProfile::medium());
        assert_eq!(tasks[0].support.z_lift_mm, 2.5);

        scene.objects_mut()[0].lift_to(0.0);
        let tasks = orient_tasks(&scene, None, |_| SupportProfile::medium());
        assert_eq!(
            tasks[0].support.z_lift_mm,
            SupportProfile::medium().z_lift_mm,
            "back on the plate, the profile's lift"
        );
    }

    #[test]
    fn an_empty_plate_has_nothing_to_orient() {
        let mut tool = OrientTool::default();
        let scene = Scene::default();
        assert!(tool.blocker(&scene).is_some());
        let mut scene = scene;
        let plate = BuildPlate::default();
        let none = |_: &SceneObject| SupportProfile::medium();
        let mut status = Status::default();
        assert!(
            tool.start(&mut scene, &plate, None, none, None, &mut status)
                .is_err()
        );
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
