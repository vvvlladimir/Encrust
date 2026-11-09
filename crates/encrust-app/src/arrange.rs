use core_geometry::{Mesh, Scalar, Transform, Vec3};
use core_plate::{ArrangeSettings, Footprint, Rect, arrange, move_clear};

use crate::i18n::tr;
use crate::plate::{BuildPlate, astray_now, bounds};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

/// Spreads everything visible over the plate, and says what would not fit.
///
/// The footprint is the model with its supports and raft as they were last built: they
/// stand on the plate beside it and move with it, so they are packed as one.
pub fn arrange_plate(scene: &mut Scene, plate: &BuildPlate, clearance_mm: Scalar) -> Status {
    let settings = ArrangeSettings {
        clearance_mm,
        ..ArrangeSettings::default()
    };

    let visible: Vec<_> = scene.printable(scene.active_plate()).collect();
    let footprints: Vec<Footprint> = visible
        .iter()
        .filter_map(|object| {
            let supports = object.supports.drawn().iter();
            let parts: Vec<(&Mesh, Transform)> = std::iter::once((&*object.mesh, object.transform))
                .chain(supports.map(|group| (&*group.mesh, Transform::default())))
                .collect();
            Footprint::of_parts(&parts, settings.cell_mm)
        })
        .collect();
    let visible: Vec<ObjectId> = visible.iter().map(|object| object.id).collect();
    if footprints.len() != visible.len() {
        return Status::Error(tr!("arrange-no-footprint"));
    }
    if footprints.is_empty() {
        return Status::Info(tr!("arrange-nothing"));
    }

    let arranged = arrange(&footprints, plate.plate_area(), &settings);
    for placed in &arranged.placed {
        let Some(id) = visible.get(placed.index) else {
            continue;
        };
        if let Some(object) = scene.get_mut(*id) {
            object.transform.translation += Vec3::new(placed.offset_mm.x, placed.offset_mm.y, 0.0);
        }
    }

    match arranged.left_out.len() {
        0 => Status::Info(tr!("arrange-done", count = arranged.placed.len())),
        short => Status::Info(tr!("arrange-short", count = short)),
    }
}

/// Moves only the models in the way, each to the nearest clear place, the others left where
/// they stand: every one past the plate or in its inset, and one of each pair of `meeting`.
pub fn clear_the_way(
    scene: &mut Scene,
    plate: &BuildPlate,
    meeting: &[[ObjectId; 2]],
    clearance_mm: Scalar,
) -> Status {
    let standing: Vec<(ObjectId, Rect)> = scene
        .printable(scene.active_plate())
        .filter_map(|object| {
            let bounds = bounds(object)?;
            let rect = Rect {
                min: bounds.mins.truncate(),
                max: bounds.maxs.truncate(),
            };
            Some((object.id, rect))
        })
        .collect();
    let index_of = |id: ObjectId| standing.iter().position(|(owner, _)| *owner == id);
    let astray: Vec<usize> = match plate.name {
        Some(_) => astray_now(scene, plate)
            .iter()
            .filter_map(|one| index_of(one.id))
            .collect(),
        None => Vec::new(),
    };
    let meeting: Vec<[usize; 2]> = meeting
        .iter()
        .filter_map(|&[one, other]| Some([index_of(one)?, index_of(other)?]))
        .collect();
    let parts: Vec<Rect> = standing.iter().map(|(_, rect)| *rect).collect();
    let settings = ArrangeSettings {
        clearance_mm,
        ..ArrangeSettings::default()
    };
    let cleared = move_clear(&parts, &astray, &meeting, plate.plate_area(), &settings);
    for moved in &cleared.moved {
        if let Some(object) = scene.get_mut(standing[moved.index].0) {
            object.transform.translation += moved.offset_mm.extend(0.0);
        }
    }
    let said = tr!("arrange-cleared", count = cleared.moved.len());
    match cleared.left_out.len() {
        0 => Status::Info(said),
        short => Status::Noted(said, vec![tr!("arrange-cleared-short", count = short)]),
    }
}

#[cfg(test)]
mod tests;
