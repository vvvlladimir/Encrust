//! Which models on the plate keep resin Check warns of: a cavity with no drain hole, and a
//! solid model a wall would save much of (ADR 0393).

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use core_geometry::FastMap;
use core_volume::{holds_resin, hollowing_saves_mm3};

use crate::hollow::HollowTool;
use crate::scene::{ObjectId, Scene, SceneObject};

/// A model on the plate being edited whose resin is worth a word before it prints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeptResin {
    /// Hollowed with no drain hole, so its cavity prints full of resin.
    Sealed(ObjectId),
    /// Solid, and hollowing it to the wall Hollow would build saves much of its resin.
    Solid(ObjectId),
}

/// The models on the plate being edited that keep resin, kept in `ctx`: a model is measured
/// again only once its mesh, shape, cavity or wall changed and the pointer is let go.
pub fn kept_resin(ctx: &egui::Context, scene: &Scene, hollow: &HollowTool) -> Vec<KeptResin> {
    let settled = ctx.input(|input| !input.pointer.any_down());
    let id = egui::Id::new("plate-kept-resin");
    let mut kept = ctx.data_mut(|data| std::mem::take(data.get_temp_mut_or_default::<Kept>(id)));
    let answer = kept.refresh(scene, hollow, settled);
    ctx.data_mut(|data| data.insert_temp(id, kept));
    answer
}

/// What each model was last found to keep, by the [`key`] it was found for, and the answer
/// for the plate as the pointer last left it.
#[derive(Clone, Default)]
struct Kept {
    by_model: FastMap<ObjectId, (u64, Option<KeptResin>)>,
    answer: Option<Vec<KeptResin>>,
}

impl Kept {
    /// While the pointer is held the last answer stands: a model turned or scaled by a drag
    /// would otherwise be measured every frame.
    fn refresh(&mut self, scene: &Scene, hollow: &HollowTool, settled: bool) -> Vec<KeptResin> {
        if let (false, Some(answer)) = (settled, &self.answer) {
            return answer.clone();
        }
        let mut by_model = FastMap::default();
        let answer: Vec<KeptResin> = scene
            .printable(scene.active_plate())
            .filter_map(|object| {
                let key = key(object, hollow);
                let found = match self.by_model.get(&object.id) {
                    Some(&(was, found)) if was == key => found,
                    _ => measured(object, hollow),
                };
                by_model.insert(object.id, (key, found));
                found
            })
            .collect();
        self.by_model = by_model;
        self.answer = Some(answer.clone());
        answer
    }
}

fn measured(object: &SceneObject, hollow: &HollowTool) -> Option<KeptResin> {
    if object.mesh.is_empty() {
        return None;
    }
    match object.hollow.asked() {
        Some(asked) => (object.hollow.cavity_mm3() > 0.0
            && holds_resin(asked, object.hollow.drains()))
        .then_some(KeptResin::Sealed(object.id)),
        // Read only off a sound mesh, as the CLI does: an open one encloses nothing.
        None if !object.summary.is_sound() => None,
        None => {
            let wall_mm = hollow.settings_for(object).thickness_mm;
            hollowing_saves_mm3(object.printed(), object.transform.linear(), wall_mm)
                .map(|_| KeptResin::Solid(object.id))
        }
    }
}

/// What a model's answer follows: its mesh, its shape on the plate, its holes and the wall
/// it would be hollowed to. Where it stands does not count.
fn key(object: &SceneObject, hollow: &HollowTool) -> u64 {
    let mut hasher = DefaultHasher::new();
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
        let kept = Kept::default().refresh(&scene, &HollowTool::default(), true);
        assert_eq!(
            kept,
            vec![KeptResin::Solid(ids[0])],
            "a 2 mm wall saves 44.8 ml of the 40 mm cube and 3.2 ml of the 20 mm one"
        );
    }

    #[test]
    fn an_open_model_is_not_measured() {
        let mut scene = scene_of_boxes(&[(Vec3::ZERO, Vec3::splat(40.0))]);
        let id = scene.objects()[0].id;
        let object = scene.get_mut(id).expect("the cube is on the plate");
        object.summary.diagnostics.boundary_edges = 3;
        assert!(
            Kept::default()
                .refresh(&scene, &HollowTool::default(), true)
                .is_empty(),
            "an open mesh encloses nothing, though its box would save 44.8 ml"
        );
    }

    #[test]
    fn while_the_pointer_is_held_the_answer_it_left_stands() {
        let mut scene = scene_of_boxes(&[(Vec3::ZERO, Vec3::splat(20.0))]);
        let id = scene.objects()[0].id;
        let (hollow, mut kept) = (HollowTool::default(), Kept::default());
        assert!(
            kept.refresh(&scene, &hollow, true).is_empty(),
            "a 2 mm wall saves 3.2 ml of the 20 mm cube"
        );
        scene
            .get_mut(id)
            .expect("the cube is on the plate")
            .transform
            .scale = Vec3::splat(2.0);
        assert!(
            kept.refresh(&scene, &hollow, false).is_empty(),
            "a scale still being dragged is not measured"
        );
        assert_eq!(
            kept.refresh(&scene, &hollow, true),
            vec![KeptResin::Solid(id)],
            "let go, the cube is 40 mm and saves 44.8 ml"
        );
    }
}
