//! Which models on the plate keep resin Check warns of: a cavity with no drain hole, and a
//! solid model a wall would save much of (ADR 0393).

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use core_volume::{holds_resin, hollowing_saves_mm3};

use crate::hollow::HollowTool;
use crate::scene::{ObjectId, Scene};

/// A model on the plate being edited whose resin is worth a word before it prints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeptResin {
    /// Hollowed with no drain hole, so its cavity prints full of resin.
    Sealed(ObjectId),
    /// Solid, and hollowing it to the wall Hollow would build saves much of its resin.
    Solid(ObjectId),
}

/// The models on the plate being edited that keep resin, kept in `ctx` until a model, its
/// cavity or the wall it would be hollowed to change.
pub fn kept_resin(ctx: &egui::Context, scene: &Scene, hollow: &HollowTool) -> Vec<KeptResin> {
    let fingerprint = fingerprint(scene, hollow);
    let id = egui::Id::new("plate-kept-resin");
    if let Some((cached, kept)) = ctx.data(|data| data.get_temp::<(u64, Vec<KeptResin>)>(id))
        && cached == fingerprint
    {
        return kept;
    }
    let kept = measured(scene, hollow);
    ctx.data_mut(|data| data.insert_temp(id, (fingerprint, kept.clone())));
    kept
}

fn measured(scene: &Scene, hollow: &HollowTool) -> Vec<KeptResin> {
    scene
        .printable(scene.active_plate())
        .filter(|object| !object.mesh.is_empty())
        .filter_map(|object| match object.hollow.asked() {
            Some(asked) => (object.hollow.cavity_mm3() > 0.0
                && holds_resin(asked, object.hollow.drains()))
            .then_some(KeptResin::Sealed(object.id)),
            None => {
                let wall_mm = hollow.settings_for(object).thickness_mm;
                hollowing_saves_mm3(object.printed(), object.transform.linear(), wall_mm)
                    .map(|_| KeptResin::Solid(object.id))
            }
        })
        .collect()
}

/// What the answer follows: each model's mesh, its shape on the plate, its holes and the
/// wall it would be hollowed to. Where it stands does not count.
fn fingerprint(scene: &Scene, hollow: &HollowTool) -> u64 {
    let mut hasher = DefaultHasher::new();
    for object in scene.printable(scene.active_plate()) {
        object.id.hash(&mut hasher);
        Arc::as_ptr(object.printed()).hash(&mut hasher);
        let linear = object.transform.linear().to_cols_array();
        linear.map(f32::to_bits).hash(&mut hasher);
        let wall_mm = hollow.settings_for(object).thickness_mm;
        wall_mm.to_bits().hash(&mut hasher);
        object
            .hollow
            .asked()
            .map(|asked| asked.open_toward.is_some())
            .hash(&mut hasher);
        for hole in object.hollow.drains() {
            hole.at.to_array().map(f32::to_bits).hash(&mut hasher);
            (hole.kind as u8).hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use core_geometry::Vec3;

    use super::*;
    use crate::scene::fixtures::scene_of_boxes;

    #[test]
    fn a_large_solid_model_is_named_and_a_small_one_is_not() {
        let scene = scene_of_boxes(&[
            (Vec3::ZERO, Vec3::splat(40.0)),
            (Vec3::new(50.0, 0.0, 0.0), Vec3::new(70.0, 20.0, 20.0)),
        ]);
        let ids: Vec<ObjectId> = scene.objects().iter().map(|object| object.id).collect();
        let kept = measured(&scene, &HollowTool::default());
        assert_eq!(
            kept,
            vec![KeptResin::Solid(ids[0])],
            "a 2 mm wall saves 44.8 ml of the 40 mm cube and 3.2 ml of the 20 mm one"
        );
    }
}
