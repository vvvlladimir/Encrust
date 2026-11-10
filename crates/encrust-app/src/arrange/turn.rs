use core_geometry::{BoundingVolume, FastMap, Transform, Vec2, placed_aabb};
use core_plate::{ArrangeSettings, Rect, turned_onto};
use rayon::prelude::*;

use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};

/// The models turned onto the plate, each with how what stood under it moved: its supports
/// as last drawn, which regrow from the same points where the model now stands.
pub(super) type Turned = FastMap<ObjectId, Transform>;

/// Turns each of `ids` that no slide brings onto the plate about the vertical through the middle
/// of what it stands on, by the least turn that does (ADR 0418); those in `turned` are left.
pub(super) fn turn_onto_plate(
    scene: &mut Scene,
    plate: &BuildPlate,
    ids: &[ObjectId],
    settings: &ArrangeSettings,
    turned: &mut Turned,
) {
    let area = plate.plate_area();
    for &id in ids {
        if turned.contains_key(&id) {
            continue;
        }
        let Some(object) = scene.get(id) else {
            continue;
        };
        let Some(rect) = standing_rect(object, Transform::default()) else {
            continue;
        };
        let Some(moved) = turned_onto(rect, || outline(object), area, settings) else {
            continue;
        };
        let rotation = moved.rotation;
        if let Some(object) = scene.get_mut(id) {
            object.transform = Transform {
                translation: moved
                    .to_matrix()
                    .transform_point3(object.transform.translation),
                ..object.transform.turned(rotation)
            };
        }
        turned.insert(id, moved);
    }
}

/// How what stood under `id` moved, if it was turned.
pub(super) fn moved_of(turned: &Turned, id: ObjectId) -> Transform {
    turned.get(&id).copied().unwrap_or_default()
}

/// The box across the plate the model stands in with its supports, those moved by `moved`.
pub(super) fn standing_rect(object: &SceneObject, moved: Transform) -> Option<Rect> {
    let supports = object.supports.drawn().iter();
    let bounds = object
        .world_bounds()
        .into_iter()
        .chain(supports.filter_map(|group| placed_aabb(&group.mesh, moved)))
        .reduce(|a, b| a.merged(&b))?;
    Some(Rect {
        min: bounds.mins.truncate(),
        max: bounds.maxs.truncate(),
    })
}

/// Every point of the model and its supports as they stand, seen from above.
fn outline(object: &SceneObject) -> Vec<Vec2> {
    let matrix = object.transform.to_matrix();
    let model = object
        .mesh
        .vertices
        .par_iter()
        .map(|vertex| matrix.transform_point3(*vertex).truncate());
    let supports = object.supports.drawn().par_iter().flat_map(|group| {
        group
            .mesh
            .vertices
            .par_iter()
            .map(|vertex| vertex.truncate())
    });
    model.chain(supports).collect()
}
