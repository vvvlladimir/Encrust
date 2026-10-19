use core_geometry::{Mesh, Scalar};
use core_volume::{DrainHole, VolumeError, laid_beside, plug_wall_mm, plugs};

use super::DrainTool;
use crate::i18n::tr;
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

impl DrainTool {
    /// Lays again the plugs of every model whose holes, wall or plug gap moved since they
    /// were last laid, in place of the ones laid then; `drift_mm` is what the resin leaves a
    /// fit wider by (`Compensation::fit_drift_mm`). A model read back from a project has no
    /// record and takes its plugs as they stand, the ones deleted before it was saved too.
    pub fn lay_plugs(&self, scene: &mut Scene, drift_mm: Scalar, status: &mut Status) {
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

        for (id, wanted, kept) in due {
            if !kept && let Err(error) = lay_again(scene, id, &wanted) {
                *status =
                    Status::failed(&anyhow::Error::new(error).context(tr!("drain-plugs-failed")));
            }
            if let Some(object) = scene.get_mut(id) {
                object.plugs_laid = Some(wanted);
            }
        }
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

/// Takes away the plugs laid for model `id` and lays the ones `wanted` asks for, leaving
/// the selection where it was.
fn lay_again(scene: &mut Scene, id: ObjectId, wanted: &PlugsLaid) -> Result<(), VolumeError> {
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
    let found = scene.get(id).filter(|_| !wanted.holes.is_empty());
    let Some((object, beside)) = found.and_then(|object| Some((object, object.world_bounds()?)))
    else {
        scene.select_many(&selection);
        return Ok(());
    };
    let made = plugs(
        &object.mesh,
        &object.bvh,
        &wanted.holes,
        wanted.wall_mm,
        wanted.gap_mm,
    );
    let laid = made.map(|made| laid_beside(&made, &wanted.holes, object.transform, beside));
    let (name, plate) = (object.name.clone(), object.plate);
    let result = laid.map(|laid| lay_out(scene, id, plate, &name, laid));
    scene.select_many(&selection);
    result
}

/// Puts `plugs`, already in plate coordinates, on `plate`, each marked as a plug of `model`.
fn lay_out(scene: &mut Scene, model: ObjectId, plate: u32, name: &str, plugs: Vec<Mesh>) {
    for (index, plug) in plugs.into_iter().enumerate() {
        let name = tr!("drain-plug-name", name = name.to_owned(), index = index + 1);
        let (id, _) = place(scene, plate, name, plug);
        if let Some(plug) = scene.get_mut(id) {
            plug.plug_of = Some(model);
        }
    }
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

    fn plugs_of(scene: &Scene, id: ObjectId) -> Vec<&SceneObject> {
        scene
            .objects()
            .iter()
            .filter(|object| object.plug_of == Some(id))
            .collect()
    }

    #[test]
    fn a_plug_for_each_hole_stands_on_the_plate_clear_of_its_model() {
        let (mut scene, id, tool) = a_block_with_holes(&[3.0, 7.0]);
        tool.lay_plugs(&mut scene, 0.0, &mut Status::default());

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
        let (mut scene, id, tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        tool.lay_plugs(&mut scene, 0.0, &mut status);
        let first = plugs_of(&scene, id)[0].id;
        tool.lay_plugs(&mut scene, 0.0, &mut status);
        assert_eq!(
            plugs_of(&scene, id)[0].id,
            first,
            "nothing moved, nothing is laid"
        );

        drill(&mut scene, id, &tool, &[7.0]);
        tool.lay_plugs(&mut scene, 0.0, &mut status);
        assert_eq!(
            plugs_of(&scene, id).len(),
            2,
            "a second hole, a second plug"
        );

        let object = scene.get_mut(id).expect("the block is on the plate");
        object.hollow.clear_drains();
        tool.lay_plugs(&mut scene, 0.0, &mut status);
        assert!(plugs_of(&scene, id).is_empty(), "no hole, no plug");
        assert_eq!(scene.objects().len(), 1, "only the block is left");
    }

    #[test]
    fn a_model_opened_with_its_plugs_keeps_them() {
        let (mut scene, id, tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        tool.lay_plugs(&mut scene, 0.0, &mut status);
        let first = plugs_of(&scene, id)[0].id;
        // A project read back carries the link, not what the plugs were laid for.
        scene.get_mut(id).expect("the block").plugs_laid = None;

        tool.lay_plugs(&mut scene, 0.0, &mut status);
        assert_eq!(
            plugs_of(&scene, id)[0].id,
            first,
            "the plug read back is kept"
        );
    }

    #[test]
    fn a_plug_deleted_before_saving_stays_deleted_once_opened() {
        let (mut scene, id, tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        tool.lay_plugs(&mut scene, 0.0, &mut status);
        let plug = plugs_of(&scene, id)[0].id;
        scene.remove(plug);
        scene.get_mut(id).expect("the block").plugs_laid = None;

        tool.lay_plugs(&mut scene, 0.0, &mut status);
        assert!(
            plugs_of(&scene, id).is_empty(),
            "the project is opened as it was saved"
        );
    }

    #[test]
    fn a_copy_of_a_model_gets_plugs_of_its_own() {
        let (mut scene, id, tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        tool.lay_plugs(&mut scene, 0.0, &mut status);
        let copy = scene
            .duplicate(id, Vec3::new(0.0, 30.0, 0.0))
            .expect("the block is there to copy");

        tool.lay_plugs(&mut scene, 0.0, &mut status);
        assert_eq!(
            plugs_of(&scene, copy).len(),
            1,
            "the copy's hole is plugged"
        );
        assert_eq!(plugs_of(&scene, id).len(), 1, "the original keeps its own");
    }

    #[test]
    fn a_copied_plate_keeps_its_plugs_apart_from_the_original() {
        let (mut scene, id, tool) = a_block_with_holes(&[3.0]);
        let mut status = Status::default();
        tool.lay_plugs(&mut scene, 0.0, &mut status);
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
        tool.lay_plugs(&mut scene, 0.0, &mut status);
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
