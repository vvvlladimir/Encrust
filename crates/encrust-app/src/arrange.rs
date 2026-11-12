use core_geometry::{Mesh, Scalar, Transform, Vec3};
use core_plate::{ArrangeSettings, Footprint, Rect, arrange, move_clear};

use crate::i18n::tr;
use crate::plate::{BuildPlate, astray_now};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

mod turn;

use turn::{Turned, moved_of, standing_rect, turn_onto_plate};

/// Spreads everything visible over the plate, a model no slide fits turned the least that
/// does, and says what would not fit.
///
/// The footprint is the model with its supports and raft as they were last built: they
/// stand on the plate beside it and move with it, so they are packed as one.
pub fn arrange_plate(scene: &mut Scene, plate: &BuildPlate, clearance_mm: Scalar) -> Status {
    let settings = settings(clearance_mm);
    let visible: Vec<ObjectId> = scene
        .printable(scene.active_plate())
        .map(|object| object.id)
        .collect();
    let mut turned = Turned::default();
    turn_onto_plate(scene, plate, &visible, &settings, &mut turned);
    noting_turns(pack(scene, plate, &settings, &turned), turned.len())
}

/// Packs every visible model over the plate, those in `turned` with their supports moved.
fn pack(
    scene: &mut Scene,
    plate: &BuildPlate,
    settings: &ArrangeSettings,
    turned: &Turned,
) -> Status {
    let visible: Vec<_> = scene.printable(scene.active_plate()).collect();
    let footprints: Vec<Footprint> = visible
        .iter()
        .filter_map(|object| {
            let moved = moved_of(turned, object.id);
            let supports = object.supports.drawn_every();
            let parts: Vec<(&Mesh, Transform)> = std::iter::once((&*object.mesh, object.transform))
                .chain(supports.map(|group| (&*group.mesh, moved)))
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

    let arranged = arrange(&footprints, plate.plate_area(), settings);
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

/// Moves only the models past the usable area and one of each pair of `meeting`, the least it
/// takes, turned where no slide fits; with no room left, packs the plate (ADR 0418).
pub fn clear_the_way(
    scene: &mut Scene,
    plate: &BuildPlate,
    meeting: &[[ObjectId; 2]],
    clearance_mm: Scalar,
) -> Status {
    let settings = settings(clearance_mm);
    let astray: Vec<ObjectId> = match plate.name {
        Some(_) => astray_now(scene, plate).iter().map(|one| one.id).collect(),
        None => Vec::new(),
    };
    let mut turned = Turned::default();
    turn_onto_plate(scene, plate, &astray, &settings, &mut turned);
    let standing: Vec<(ObjectId, Rect)> = scene
        .printable(scene.active_plate())
        .filter_map(|object| {
            Some((
                object.id,
                standing_rect(object, moved_of(&turned, object.id))?,
            ))
        })
        .collect();
    let index_of = |id: ObjectId| standing.iter().position(|(owner, _)| *owner == id);
    let astray: Vec<usize> = astray.iter().filter_map(|&id| index_of(id)).collect();
    let meeting: Vec<[usize; 2]> = meeting
        .iter()
        .filter_map(|&[one, other]| Some([index_of(one)?, index_of(other)?]))
        .collect();
    let parts: Vec<Rect> = standing.iter().map(|(_, rect)| *rect).collect();
    let cleared = move_clear(&parts, &astray, &meeting, plate.plate_area(), &settings);
    if !cleared.left_out.is_empty() {
        let visible: Vec<ObjectId> = standing.iter().map(|(id, _)| *id).collect();
        turn_onto_plate(scene, plate, &visible, &settings, &mut turned);
        return noting_turns(pack(scene, plate, &settings, &turned), turned.len());
    }
    for moved in &cleared.moved {
        if let Some(object) = scene.get_mut(standing[moved.index].0) {
            object.transform.translation += moved.offset_mm.extend(0.0);
        }
    }
    noting_turns(
        Status::Info(tr!("arrange-cleared", count = cleared.moved.len())),
        turned.len(),
    )
}

fn settings(clearance_mm: Scalar) -> ArrangeSettings {
    ArrangeSettings {
        clearance_mm,
        ..ArrangeSettings::default()
    }
}

/// `status` with a line saying how many models were turned to fit, where any were.
fn noting_turns(status: Status, turned: usize) -> Status {
    if turned == 0 {
        return status;
    }
    let note = tr!("arrange-turned", count = turned);
    match status {
        Status::Info(said) => Status::Noted(said, vec![note]),
        Status::Noted(said, mut notes) => {
            notes.push(note);
            Status::Noted(said, notes)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests;
