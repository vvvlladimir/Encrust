use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_volume::{DrainHole, HollowMode, VolumeError, plugs, stand_on_end};

use super::DrainTool;
use crate::i18n::tr;
use crate::landing::place;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// Space left between a model and the plugs laid out beside it, and between one plug and
/// the next, millimetres.
const PLUG_SPACING_MM: Scalar = 3.0;

/// What one model's plugs were last laid for, in its own space: they are laid again
/// whenever it changes (ADR 0313).
#[derive(Debug, Clone, PartialEq)]
pub struct PlugsLaid {
    holes: Vec<DrainHole>,
    wall_mm: Option<Scalar>,
    gap_mm: Scalar,
}

impl DrainTool {
    /// Lays again the plugs of every model whose holes, wall or plug gap moved since they
    /// were last laid, in place of the ones laid then; `drift_mm` is what the resin leaves a
    /// fit wider by (`Compensation::fit_drift_mm`). A model opened with its plugs already
    /// beside it takes them as laid.
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
                    None => scene
                        .objects()
                        .iter()
                        .any(|plug| plug.plug_of == Some(object.id)),
                };
                Some((object.id, wanted, kept))
            })
            .collect();

        for (id, wanted, kept) in due {
            if !kept && let Err(error) = lay_again(scene, id, &wanted) {
                *status = Status::Error(error.to_string());
            }
            if let Some(object) = scene.get_mut(id) {
                object.plugs_laid = Some(wanted);
            }
        }
    }

    /// The plugs `object` asks for now: one a hole, as long as the wall where it is an
    /// inside shell, as deep as the hole otherwise.
    fn plugs_for(&self, object: &SceneObject, drift_mm: Scalar) -> PlugsLaid {
        let least = object.transform.least_scale();
        PlugsLaid {
            holes: object.hollow.drains().to_vec(),
            wall_mm: object
                .hollow
                .asked()
                .filter(|asked| asked.mode == HollowMode::Internal)
                .map(|asked| asked.thickness_mm / least),
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
    let Some(object) = scene.get(id).filter(|_| !wanted.holes.is_empty()) else {
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
    let stood = made.map(|made| -> Vec<Mesh> {
        made.iter()
            .zip(&wanted.holes)
            .map(|(plug, hole)| {
                let placed = transform_mesh(plug, object.transform);
                stand_on_end(&placed, object.transform.linear() * hole.axis)
            })
            .collect()
    });
    let name = object.name.clone();
    if let Ok(stood) = &stood {
        lay_out(scene, id, &name, stood);
    }
    scene.select_many(&selection);
    stood.map(|_| ())
}

/// Stands `plugs` on the plate in a row past the right-hand side of model `beside`, each
/// marked as its plug.
fn lay_out(scene: &mut Scene, beside: ObjectId, name: &str, plugs: &[Mesh]) {
    let Some((bounds, plate)) = scene
        .get(beside)
        .and_then(|object| Some((object.world_bounds()?, object.plate)))
    else {
        return;
    };
    let mut x = bounds.maxs.x + PLUG_SPACING_MM;
    for (index, plug) in plugs.iter().enumerate() {
        let Some(size) = plug.aabb().map(|bounds| bounds.extents()) else {
            continue;
        };
        let offset = Vec3::new(x + size.x / 2.0, bounds.mins.y + size.y / 2.0, 0.0);
        let laid = transform_mesh(plug, Transform::from_translation(offset));
        let name = tr!("drain-plug-name", name = name.to_owned(), index = index + 1);
        let (id, _) = place(scene, plate, name, laid);
        if let Some(plug) = scene.get_mut(id) {
            plug.plug_of = Some(beside);
        }
        x += size.x + PLUG_SPACING_MM;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_geometry::{cuboid, diagnose};

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
}
