use core_geometry::{Mesh, Scalar, Transform, Vec2, Vec3};
use core_plate::{ArrangeSettings, Footprint, arrange};

use crate::i18n::tr;
use crate::plate::BuildPlate;
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

    let arranged = arrange(&footprints, Vec2::new(plate.x_mm, plate.y_mm), &settings);
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

#[cfg(test)]
mod tests;
