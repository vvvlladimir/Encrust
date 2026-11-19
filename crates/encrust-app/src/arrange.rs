use core_document::spread;
use core_geometry::Scalar;
use core_plate::{ArrangeSettings, Rect, move_clear};

use crate::i18n::tr;
use crate::plate::{BuildPlate, astray_now};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;

mod turn;

use turn::{Turned, moved_of, standing_of, standing_rect_of, turn_onto_plate};

/// Spreads everything visible over the plate, a model no slide fits turned the least that
/// does, and says what would not fit.
///
/// The footprint is the model with its supports and raft as they were last built: they
/// stand on the plate beside it and move with it, so they are packed as one.
pub fn arrange_plate(scene: &mut Scene, plate: &BuildPlate, clearance_mm: Scalar) -> Status {
    let (status, turned) = pack(scene, plate, &settings(clearance_mm), &Turned::default());
    noting_turns(status, turned)
}

/// Packs every visible model over the plate, those in `turned` with their supports moved, and
/// says how many more it turned.
fn pack(
    scene: &mut Scene,
    plate: &BuildPlate,
    settings: &ArrangeSettings,
    turned: &Turned,
) -> (Status, usize) {
    let ids: Vec<ObjectId> = scene
        .printable(scene.active_plate())
        .map(|object| object.id)
        .collect();
    if ids.is_empty() {
        return (Status::Info(tr!("arrange-nothing")), 0);
    }
    let mut standing: Vec<_> = (ids.iter())
        .filter_map(|id| scene.get(*id))
        .map(|object| standing_of(object, moved_of(turned, object.id)))
        .collect();
    let Ok(spread) = spread(&mut standing, plate.plate_area(), settings) else {
        return (Status::Error(tr!("arrange-no-footprint")), 0);
    };
    drop(standing);
    for (id, transform) in ids.iter().zip(&spread.transforms) {
        if let Some(object) = scene.get_mut(*id) {
            object.transform = *transform;
        }
    }
    let status = match spread.arranged.left_out.len() {
        0 => Status::Info(tr!("arrange-done", count = spread.arranged.placed.len())),
        short => Status::Info(tr!("arrange-short", count = short)),
    };
    (status, spread.turned)
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
                standing_rect_of(object, moved_of(&turned, object.id))?,
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
        let (status, more) = pack(scene, plate, &settings, &turned);
        return noting_turns(status, turned.len() + more);
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
