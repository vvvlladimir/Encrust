use std::sync::{Arc, Weak};

use core_geometry::{Aabb, Mesh, Scalar, Transform, Vec2, transform_mesh};
use core_plate::{ArrangeSettings, Rect, nearest_free};
use core_volume::{DrainHole, PLUG_SPACING_MM, PlugSource, Plugs, laid_beside, plug_wall_mm};

use super::DrainTool;
use crate::i18n::tr;
use crate::job::{PlugsJob, PlugsRequest, spawn_plugs};
use crate::landing::place;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// What one model's plugs were last laid for, in its own space: they are laid again
/// whenever it changes (ADR 0313). The default is laid for nothing, a model's first word.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlugsLaid {
    holes: Vec<DrainHole>,
    wall_mm: Option<Scalar>,
    gap_mm: Scalar,
}

/// The plugs being cut for one model, and what they are cut for.
#[derive(Debug)]
pub struct Laying {
    model: ObjectId,
    /// The mesh they are cut from, not held so a model replaced meanwhile is let go.
    mesh: Weak<Mesh>,
    wanted: PlugsLaid,
    /// The model asks for other plugs now: these are thrown away when they land, and the
    /// next cut starts then rather than beside this one, which cannot be stopped.
    stale: bool,
    /// The number of the edit that asked for them, `History::next_edit` when they started.
    asked_by: u64,
    job: PlugsJob,
}

/// Plugs a model asks for that cannot be started yet, and the edit that first asked for them.
#[derive(Debug)]
pub struct Waiting {
    model: ObjectId,
    wanted: PlugsLaid,
    asked_by: u64,
}

/// A cut that has ended, waiting to be laid into the edit that asked for it.
#[derive(Debug)]
pub struct Ended {
    model: ObjectId,
    wanted: PlugsLaid,
    asked_by: u64,
    made: Result<Plugs, String>,
}

impl Ended {
    /// The number of the edit that asked for these plugs, for `History::land`.
    pub fn asked_by(&self) -> u64 {
        self.asked_by
    }
}

/// The piece of a model read round its last holes, kept for the next cut while the model is
/// the same mesh.
#[derive(Debug)]
pub struct ReadModel {
    model: ObjectId,
    mesh: Weak<Mesh>,
    source: PlugSource,
}

impl DrainTool {
    /// Starts cutting again the plugs of every model whose holes, wall or plug gap moved
    /// since they were last laid; they land on a later frame through [`Self::land_plugs`].
    /// `drift_mm` is what the resin leaves a fit wider by (`Compensation::fit_drift_mm`). A
    /// model read back from a project has no record and takes its plugs as they stand, the
    /// ones deleted before it was saved too. No cut starts while `settled` is false, in the
    /// middle of a drag: the plugs follow once it ends. `asked_by` is `History::next_edit`.
    pub fn lay_plugs(&mut self, scene: &mut Scene, drift_mm: Scalar, settled: bool, asked_by: u64) {
        let due: Vec<(ObjectId, PlugsLaid, bool)> = scene
            .objects()
            .iter()
            .filter(|object| object.plug_of.is_none())
            .filter_map(|object| {
                let wanted = self.plugs_for(object, drift_mm);
                let kept = match &object.plugs_laid {
                    Some(laid) if *laid == wanted => return None,
                    Some(_) => false,
                    None => true,
                };
                Some((object.id, wanted, kept))
            })
            .collect();
        // A cut for what a model no longer asks for, as after an undo, or from a mesh since
        // replaced, is not laid.
        for laying in &mut self.laying {
            let same_mesh = scene
                .get(laying.model)
                .is_some_and(|object| laying.mesh.as_ptr() == Arc::as_ptr(&object.mesh));
            laying.stale = !same_mesh
                || !due
                    .iter()
                    .any(|(id, wanted, _)| *id == laying.model && *wanted == laying.wanted);
        }

        self.waiting.retain(|waiting| {
            due.iter()
                .any(|(id, wanted, _)| *id == waiting.model && *wanted == waiting.wanted)
        });

        for (id, wanted, kept) in due {
            if kept || wanted.holes.is_empty() {
                if !kept {
                    take_away(scene, id);
                }
                if let Some(object) = scene.get_mut(id) {
                    object.plugs_laid = Some(wanted);
                }
            } else {
                self.ask(scene, id, wanted, settled, asked_by);
            }
        }
    }

    /// Starts cutting the plugs `wanted` for model `id` where nothing holds them back, else
    /// keeps them waiting, under the edit that first asked for them either way.
    fn ask(
        &mut self,
        scene: &Scene,
        id: ObjectId,
        wanted: PlugsLaid,
        settled: bool,
        asked_by: u64,
    ) {
        let asked_by = self
            .waiting
            .iter()
            .find(|waiting| waiting.model == id)
            .map_or(asked_by, |waiting| waiting.asked_by);
        self.waiting.retain(|waiting| waiting.model != id);
        if settled && !self.laying.iter().any(|laying| laying.model == id) {
            self.start_plugs(scene, id, wanted, asked_by);
        } else {
            self.waiting.push(Waiting {
                model: id,
                wanted,
                asked_by,
            });
        }
    }

    /// Starts cutting the plugs `wanted` for model `id`, handing the cut the model as last
    /// read where it is still the same mesh.
    fn start_plugs(&mut self, scene: &Scene, id: ObjectId, wanted: PlugsLaid, asked_by: u64) {
        let Some(object) = scene.get(id) else {
            return;
        };
        let source = self
            .read
            .take_if(|read| read.model == id && read.mesh.as_ptr() == Arc::as_ptr(&object.mesh))
            .map(|read| read.source);
        let job = spawn_plugs(PlugsRequest {
            mesh: Arc::clone(&object.mesh),
            bvh: Arc::clone(&object.bvh),
            source,
            holes: wanted.holes.clone(),
            wall_mm: wanted.wall_mm,
            gap_mm: wanted.gap_mm,
        });
        self.laying.push(Laying {
            model: id,
            mesh: Arc::downgrade(&object.mesh),
            wanted,
            stale: false,
            asked_by,
            job,
        });
    }

    /// The cuts that have ended and still stand for what their model asks, keeping the model
    /// each read for the next cut. Cheap while none has ended, so it runs every frame.
    pub fn poll_plugs(&mut self) -> Vec<Ended> {
        let mut ended = Vec::new();
        let mut index = 0;
        while index < self.laying.len() {
            let Some((source, made)) = self.laying[index].job.poll() else {
                index += 1;
                continue;
            };
            let laying = self.laying.swap_remove(index);
            if let Some(source) = source.filter(|_| laying.mesh.strong_count() > 0) {
                self.read = Some(ReadModel {
                    model: laying.model,
                    mesh: laying.mesh,
                    source,
                });
            }
            if !laying.stale {
                ended.push(Ended {
                    model: laying.model,
                    wanted: laying.wanted,
                    asked_by: laying.asked_by,
                    made,
                });
            }
        }
        ended
    }

    /// Lays the plugs `ended` cut in place of the ones laid before, on a plate `plate_mm`
    /// across, and says so where a plug is only sampled, missing or off the free plate.
    pub fn land_plugs(scene: &mut Scene, ended: &Ended, plate_mm: Vec2, status: &mut Status) {
        let id = ended.model;
        match &ended.made {
            Ok(made) => {
                let crowded = replace(scene, id, &ended.wanted.holes, &made.meshes, plate_mm);
                let notes: Vec<String> = [
                    (
                        made.sampled,
                        tr!("drain-plugs-sampled", count = made.sampled),
                    ),
                    (made.closed, tr!("drain-plugs-closed", count = made.closed)),
                    (crowded, tr!("drain-plugs-crowded", count = crowded)),
                ]
                .into_iter()
                .filter(|(count, _)| *count > 0)
                .map(|(_, note)| note)
                .collect();
                if let Some((first, rest)) = notes.split_first() {
                    *status = Status::Noted(first.clone(), rest.to_vec());
                }
            }
            Err(message) => {
                let error = anyhow::Error::msg(message.clone()).context(tr!("drain-plugs-failed"));
                *status = Status::failed(&error);
            }
        }
        if let Some(object) = scene.get_mut(id) {
            object.plugs_laid = Some(ended.wanted.clone());
        }
    }

    /// Whether plugs are being cut.
    pub fn laying_plugs(&self) -> bool {
        !self.laying.is_empty()
    }

    /// Lets go of the piece of a model kept read for its plugs once they are no longer being
    /// worked on.
    pub fn forget_read_model(&mut self) {
        self.read = None;
    }

    /// The plugs `object` asks for now: one a hole as it is cut, through the wall.
    fn plugs_for(&self, object: &SceneObject, drift_mm: Scalar) -> PlugsLaid {
        let least = object.transform.least_scale();
        PlugsLaid {
            holes: object.hollow.cut().0.to_vec(),
            wall_mm: object
                .hollow
                .asked()
                .and_then(|asked| plug_wall_mm(asked.mode, asked.thickness_mm / least)),
            gap_mm: (self.state_for(object).plug_gap_mm - drift_mm) / least,
        }
    }
}

/// Takes away the plugs laid for model `id`, leaving the selection where it was.
fn take_away(scene: &mut Scene, id: ObjectId) {
    let selection = scene.selection().to_vec();
    let old: Vec<ObjectId> = scene
        .objects()
        .iter()
        .filter(|object| object.plug_of == Some(id))
        .map(|object| object.id)
        .collect();
    for plug in old {
        scene.remove(plug);
    }
    scene.select_many(&selection);
}

/// Takes away the plugs laid for model `id` and lays `made`, cut for its `holes`, on a plate
/// `plate_mm` across, leaving the selection where it was. Answers how many found no room.
fn replace(
    scene: &mut Scene,
    id: ObjectId,
    holes: &[DrainHole],
    made: &[Mesh],
    plate_mm: Vec2,
) -> usize {
    take_away(scene, id);
    let selection = scene.selection().to_vec();
    let found = scene.get(id).and_then(|object| {
        let beside = object.world_bounds()?;
        Some((beside, object.transform, object.name.clone(), object.plate))
    });
    let mut crowded = 0;
    if let Some((beside, transform, name, plate)) = found {
        // `laid_beside` leaves out an empty plug, so each is numbered after its own hole.
        let numbers = (1..).zip(made).filter(|(_, plug)| !plug.is_empty());
        let laid = numbers
            .map(|(number, _)| number)
            .zip(laid_beside(made, holes, transform, beside));
        crowded = lay_out(scene, id, plate, &name, laid, plate_mm);
    }
    scene.select_many(&selection);
    crowded
}

/// Puts `plugs`, numbered and already in plate coordinates, on `plate`, each marked as a plug
/// of `model` and moved to the nearest spot clear of what stands there, inside the plate
/// where it fits. Answers how many fit nowhere and were left where they were laid.
fn lay_out(
    scene: &mut Scene,
    model: ObjectId,
    plate: u32,
    name: &str,
    plugs: impl Iterator<Item = (usize, Mesh)>,
    plate_mm: Vec2,
) -> usize {
    let mut taken = covered(scene, plate);
    let settings = ArrangeSettings {
        clearance_mm: PLUG_SPACING_MM,
        ..ArrangeSettings::default()
    };
    let mut crowded = 0;
    for (number, plug) in plugs {
        let Some(bounds) = plug.aabb() else {
            continue;
        };
        let (min, max) = (bounds.mins.truncate(), bounds.maxs.truncate());
        let toward = (min + max) / 2.0;
        let at =
            nearest_free(max - min, &taken, toward, plate_mm, &settings).unwrap_or_else(|| {
                crowded += 1;
                toward
            });
        let shift = (at - toward).extend(0.0);
        taken.push(Rect {
            min: min + shift.truncate(),
            max: max + shift.truncate(),
        });
        let plug = transform_mesh(&plug, Transform::from_translation(shift));
        let name = tr!("drain-plug-name", name = name.to_owned(), index = number);
        let (id, _) = place(scene, plate, name, plug);
        if let Some(plug) = scene.get_mut(id) {
            plug.plug_of = Some(model);
        }
    }
    crowded
}

/// What the models on `plate`, their supports and their rafts cover across it.
fn covered(scene: &Scene, plate: u32) -> Vec<Rect> {
    let rect = |bounds: Aabb| Rect {
        min: bounds.mins.truncate(),
        max: bounds.maxs.truncate(),
    };
    scene
        .printable(plate)
        .flat_map(|object| {
            let supports = object
                .supports
                .drawn()
                .iter()
                .filter_map(|mesh| mesh.aabb());
            object.world_bounds().into_iter().chain(supports)
        })
        .map(rect)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_geometry::{Transform, Vec3, cuboid, diagnose};

    use super::*;
    use crate::scene::{ImportSummary, Imported};

    fn a_block_with_holes(at: &[Scalar]) -> (Scene, ObjectId, DrainTool) {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let mut scene = Scene::default();
        let id = scene.insert(Imported::new(
            "block".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary::made(diagnose(&mesh)),
        ));
        let tool = DrainTool::default();
        drill(&mut scene, id, &tool, at);
        (scene, id, tool)
    }

    fn drill(scene: &mut Scene, id: ObjectId, tool: &DrainTool, at: &[Scalar]) {
        let object = scene.get_mut(id).expect("the block is on the plate");
        let (mesh, bvh) = (Arc::clone(&object.mesh), Arc::clone(&object.bvh));
        for x in at {
            object.hollow.add_drain(
                &mesh,
                &bvh,
                Vec3::new(*x, 5.0, 10.0),
                Vec3::Z,
                tool.size(),
                Transform::default(),
            );
        }
    }

    /// Runs frames until no plug is due or being cut, as the frames after an edit do.
    fn lay(tool: &mut DrainTool, scene: &mut Scene, status: &mut Status) {
        for _ in 0..10_000 {
            tool.lay_plugs(scene, 0.0, true, 0);
            if !tool.laying_plugs() {
                return;
            }
            let ended = tool.poll_plugs();
            for ended in &ended {
                DrainTool::land_plugs(scene, ended, Vec2::new(200.0, 120.0), status);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("the plugs were still being cut after ten seconds");
    }

    fn plugs_of(scene: &Scene, id: ObjectId) -> Vec<&SceneObject> {
        scene
            .objects()
            .iter()
            .filter(|object| object.plug_of == Some(id))
            .collect()
    }

    #[test]
    fn a_plug_for_each_hole_stands_on_the_plate_clear_of_its_model() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0, 7.0]);
        lay(&mut tool, &mut scene, &mut Status::default());

        let plugs = plugs_of(&scene, id);
        assert_eq!(plugs.len(), 2, "one plug a hole");
        assert_eq!(scene.selection(), [id], "the block stays picked");
        let edge = scene.get(id).and_then(SceneObject::world_bounds);
        let edge = edge.expect("the block has bounds").maxs.x;
        for plug in plugs {
            let bounds = plug.world_bounds().expect("a plug has bounds");
            assert!(
                bounds.mins.z.abs() < 1e-4,
                "{} stands on the plate",
                plug.name
            );
            assert!(bounds.mins.x > edge, "{} is clear of the block", plug.name);
            assert!(
                (bounds.maxs.z - 3.0).abs() < 1e-3,
                "a solid block's plug is as deep as its 3 mm hole, got {}",
                bounds.maxs.z
            );
        }
    }

    #[test]
    fn plugs_follow_the_holes_and_are_left_alone_while_nothing_moves() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        lay(&mut tool, &mut scene, &mut status);
        let first = plugs_of(&scene, id)[0].id;
        lay(&mut tool, &mut scene, &mut status);
        assert_eq!(
            plugs_of(&scene, id)[0].id,
            first,
            "nothing moved, nothing is laid"
        );

        drill(&mut scene, id, &tool, &[7.0]);
        lay(&mut tool, &mut scene, &mut status);
        assert_eq!(
            plugs_of(&scene, id).len(),
            2,
            "a second hole, a second plug"
        );

        let object = scene.get_mut(id).expect("the block is on the plate");
        object.hollow.clear_drains();
        lay(&mut tool, &mut scene, &mut status);
        assert!(plugs_of(&scene, id).is_empty(), "no hole, no plug");
        assert_eq!(scene.objects().len(), 1, "only the block is left");
    }

    #[test]
    fn plugs_cut_for_holes_taken_back_before_they_land_are_let_go() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        tool.lay_plugs(&mut scene, 0.0, true, 0);
        assert!(tool.laying_plugs(), "the plug is being cut");

        // An undo puts the block back as it was before the hole.
        let object = scene.get_mut(id).expect("the block is on the plate");
        object.hollow.clear_drains();
        lay(&mut tool, &mut scene, &mut status);
        assert!(!tool.laying_plugs(), "the cut is let go");
        assert!(plugs_of(&scene, id).is_empty(), "and lays nothing");
    }

    #[test]
    fn plugs_cut_from_a_mesh_replaced_before_they_land_are_cut_again() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        tool.lay_plugs(&mut scene, 0.0, true, 0);
        let object = scene.get_mut(id).expect("the block is on the plate");
        object.mesh = Arc::new((*object.mesh).clone());

        lay(&mut tool, &mut scene, &mut Status::default());
        assert_eq!(plugs_of(&scene, id).len(), 1, "the plug is laid");
        let read = tool.read.as_ref().expect("the model is kept read");
        let mesh = &scene.get(id).expect("the block").mesh;
        assert_eq!(
            read.mesh.as_ptr(),
            Arc::as_ptr(mesh),
            "cut from the mesh the block has now"
        );
    }

    /// Whether two boxes across the plate overlap, touching aside.
    fn overlap(one: Aabb, other: Aabb) -> bool {
        one.mins.x < other.maxs.x
            && other.mins.x < one.maxs.x
            && one.mins.y < other.maxs.y
            && other.mins.y < one.maxs.y
    }

    #[test]
    fn a_plug_keeps_clear_of_a_model_standing_where_it_would_go() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0, 7.0]);
        // A second block just right of the first, where the plugs are laid in a row.
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let neighbour = scene.insert(Imported::new(
            "neighbour".to_owned(),
            Arc::new(mesh.clone()),
            Transform::from_translation(Vec3::new(12.0, 0.0, 0.0)),
            ImportSummary::made(diagnose(&mesh)),
        ));
        lay(&mut tool, &mut scene, &mut Status::default());

        let next = scene.get(neighbour).expect("the neighbour");
        let beside = next.world_bounds().expect("the neighbour has bounds");
        let plugs: Vec<Aabb> = plugs_of(&scene, id)
            .iter()
            .map(|plug| plug.world_bounds().expect("a plug has bounds"))
            .collect();
        assert_eq!(plugs.len(), 2, "one plug a hole");
        for plug in &plugs {
            assert!(
                !overlap(*plug, beside),
                "no plug stands on the neighbour: {plug:?}"
            );
            assert!(
                plug.mins.x >= 2.0 - 1e-4 && plug.mins.y >= 2.0 - 1e-4,
                "on the plate"
            );
        }
        assert!(!overlap(plugs[0], plugs[1]), "nor on one another");
    }

    #[test]
    fn a_model_at_the_edge_of_the_plate_gets_its_plug_on_the_plate() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        // The test plate is 200 mm across: the block's right side 1 mm short of its edge.
        scene.get_mut(id).expect("the block").transform =
            Transform::from_translation(Vec3::new(189.0, 50.0, 0.0));
        lay(&mut tool, &mut scene, &mut Status::default());

        let plug = plugs_of(&scene, id)[0]
            .world_bounds()
            .expect("a plug has bounds");
        assert!(
            plug.maxs.x <= 200.0 - 2.0 + 1e-4,
            "the plug stays inside the plate's margin, got {plug:?}"
        );
    }

    #[test]
    fn a_plug_with_no_room_on_the_plate_is_said_to_have_none() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        tool.lay_plugs(&mut scene, 0.0, true, 0);
        while tool.laying_plugs() {
            for ended in tool.poll_plugs() {
                // A plate no wider than the block leaves no room beside it.
                DrainTool::land_plugs(&mut scene, &ended, Vec2::new(10.0, 10.0), &mut status);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(plugs_of(&scene, id).len(), 1, "the plug is still laid");
        assert!(
            matches!(&status, Status::Noted(_, rest) if rest.is_empty()),
            "and the user told it found no room: {status:?}"
        );
    }

    #[test]
    fn a_plug_is_numbered_after_its_hole_when_an_earlier_one_has_none() {
        let (mut scene, id, _) = a_block_with_holes(&[3.0, 7.0]);
        let holes = scene.get(id).expect("the block").hollow.cut().0.to_vec();
        let made = [Mesh::default(), cuboid(Vec3::ZERO, Vec3::ONE)];
        replace(&mut scene, id, &holes, &made, Vec2::new(200.0, 120.0));

        let plugs = plugs_of(&scene, id);
        assert_eq!(plugs.len(), 1, "the empty plug is not laid");
        let name = tr!("drain-plug-name", name = "block".to_owned(), index = 2);
        assert_eq!(plugs[0].name, name, "the one laid is the second hole's");
    }

    #[test]
    fn plugs_started_again_after_a_stale_cut_land_in_the_edit_that_asked_for_them() {
        let (mut scene, _, mut tool) = a_block_with_holes(&[5.0]);
        tool.lay_plugs(&mut scene, 0.0, true, 0);
        tool.state.plug_gap_mm = 0.5;
        tool.lay_plugs(&mut scene, 0.0, true, 1);

        let mut landed = Vec::new();
        for frame in 2..10_000 {
            tool.lay_plugs(&mut scene, 0.0, true, frame);
            if !tool.laying_plugs() {
                break;
            }
            for ended in tool.poll_plugs() {
                landed.push(ended.asked_by());
                DrainTool::land_plugs(
                    &mut scene,
                    &ended,
                    Vec2::new(200.0, 120.0),
                    &mut Status::default(),
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            landed,
            [1],
            "the gap's edit asked for the plugs that landed"
        );
    }

    #[test]
    fn no_plug_is_cut_in_the_middle_of_a_drag() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        tool.lay_plugs(&mut scene, 0.0, false, 0);
        assert!(
            !tool.laying_plugs(),
            "nothing starts while the pointer is down"
        );

        lay(&mut tool, &mut scene, &mut Status::default());
        assert_eq!(
            plugs_of(&scene, id).len(),
            1,
            "the plug follows once it is up"
        );
    }

    #[test]
    fn a_gap_changed_while_plugs_are_cut_waits_for_them_and_reads_the_model_once() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[5.0]);
        tool.lay_plugs(&mut scene, 0.0, true, 0);
        tool.state.plug_gap_mm = 0.5;
        tool.lay_plugs(&mut scene, 0.0, true, 0);
        assert_eq!(tool.laying.len(), 1, "one cut a model at a time");

        lay(&mut tool, &mut scene, &mut Status::default());
        let plugs = plugs_of(&scene, id);
        assert_eq!(
            plugs.len(),
            1,
            "the plug cut for the old gap is thrown away"
        );
        let width = plugs[0]
            .world_bounds()
            .expect("a plug has bounds")
            .extents()
            .x;
        // A 3 mm hole, 0.5 mm in on each side: under 2 mm across, where 0.15 left 2.7.
        assert!(
            width < 2.05,
            "the plug is cut for the last gap, got {width} mm"
        );
        let read = tool.read.as_ref().expect("the model is kept read");
        let mesh = &scene.get(id).expect("the block").mesh;
        assert_eq!(
            read.mesh.as_ptr(),
            Arc::as_ptr(mesh),
            "and kept for the block as it is"
        );
    }

    #[test]
    fn a_model_opened_with_its_plugs_keeps_them() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        lay(&mut tool, &mut scene, &mut status);
        let first = plugs_of(&scene, id)[0].id;
        // A project read back carries the link, not what the plugs were laid for.
        scene.get_mut(id).expect("the block").plugs_laid = None;

        lay(&mut tool, &mut scene, &mut status);
        assert_eq!(
            plugs_of(&scene, id)[0].id,
            first,
            "the plug read back is kept"
        );
    }

    #[test]
    fn a_plug_deleted_before_saving_stays_deleted_once_opened() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        lay(&mut tool, &mut scene, &mut status);
        let plug = plugs_of(&scene, id)[0].id;
        scene.remove(plug);
        scene.get_mut(id).expect("the block").plugs_laid = None;

        lay(&mut tool, &mut scene, &mut status);
        assert!(
            plugs_of(&scene, id).is_empty(),
            "the project is opened as it was saved"
        );
    }

    #[test]
    fn a_copy_of_a_model_gets_plugs_of_its_own() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        lay(&mut tool, &mut scene, &mut status);
        let copy = scene
            .duplicate(id, Vec3::new(0.0, 30.0, 0.0))
            .expect("the block is there to copy");

        lay(&mut tool, &mut scene, &mut status);
        assert_eq!(
            plugs_of(&scene, copy).len(),
            1,
            "the copy's hole is plugged"
        );
        assert_eq!(plugs_of(&scene, id).len(), 1, "the original keeps its own");
    }

    #[test]
    fn a_copied_plate_keeps_its_plugs_apart_from_the_original() {
        let (mut scene, id, mut tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        lay(&mut tool, &mut scene, &mut status);
        let copied = scene.duplicate_plate(0).expect("plate 0 is there to copy");
        let model = scene
            .on_plate(copied)
            .find(|object| object.plug_of.is_none())
            .map(|object| object.id)
            .expect("the block was copied");
        assert_eq!(
            plugs_of(&scene, model).len(),
            1,
            "the copied plug is the copy's"
        );

        drill(&mut scene, id, &tool, &[7.0]);
        lay(&mut tool, &mut scene, &mut status);
        assert_eq!(
            plugs_of(&scene, id).len(),
            2,
            "the original lays its own again"
        );
        assert_eq!(
            plugs_of(&scene, model).len(),
            1,
            "and leaves the copy's alone"
        );
    }
}
