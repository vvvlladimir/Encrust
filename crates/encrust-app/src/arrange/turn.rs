use core_document::{Standing, standing_rect, turn_to_fit, turned_by};
use core_geometry::{FastMap, Transform};
use core_plate::{ArrangeSettings, Rect};

use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};

/// The models turned onto the plate, each with how what stood under it moved: its supports
/// as last drawn, which regrow from the same points where the model now stands.
pub(super) type Turned = FastMap<ObjectId, Transform>;

/// `object` with its supports as last drawn, those moved by `moved`.
pub(super) fn standing_of(object: &SceneObject, moved: Transform) -> Standing<'_> {
    Standing {
        mesh: &object.mesh,
        transform: object.transform,
        supports: object
            .supports
            .drawn_every()
            .map(|group| &*group.mesh)
            .collect(),
        moved,
    }
}

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
        let Some(moved) = turn_to_fit(&standing_of(object, Transform::default()), area, settings)
        else {
            continue;
        };
        if let Some(object) = scene.get_mut(id) {
            object.transform = turned_by(object.transform, moved);
        }
        turned.insert(id, moved);
    }
}

/// How what stood under `id` moved, if it was turned.
pub(super) fn moved_of(turned: &Turned, id: ObjectId) -> Transform {
    turned.get(&id).copied().unwrap_or_default()
}

/// The box across the plate the model stands in with its supports, those moved by `moved`.
pub(super) fn standing_rect_of(object: &SceneObject, moved: Transform) -> Option<Rect> {
    standing_rect(&standing_of(object, moved))
}
