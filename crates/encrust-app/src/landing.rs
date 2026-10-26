//! Putting what a tool made on the plate in place of the models it took.

use std::sync::Arc;

use core_geometry::{MendMethod, MendSettings, Mesh, Transform, diagnose, mend};

use crate::i18n::tr;
use crate::scene::{ImportSummary, Imported, ObjectId, Scene, Taken};

/// Takes the models `taken` off their plate, or answers why not when one of them has
/// changed since the job took it.
pub(crate) fn take_off(scene: &mut Scene, taken: &[Taken]) -> Result<(), String> {
    if let Some(changed) = taken.iter().find(|model| !model.is_current(scene)) {
        return Err(tr!("job-model-changed", name = changed.name.clone()));
    }
    for model in taken {
        scene.remove(model.id);
    }
    Ok(())
}

/// Puts a mesh a tool made, already in plate coordinates, on `plate`, mending it first if
/// it did not come out sound. Answers the model it became, and whether it had to be mended.
///
/// Mending is not asked for the way an imported model's is (ADR 0194): the surface is the
/// tool's own, and the stage notice says what was done to it.
pub(crate) fn place(scene: &mut Scene, plate: u32, name: String, mesh: Mesh) -> (ObjectId, bool) {
    let mut mesh = mesh;
    let mut diagnostics = diagnose(&mesh);
    let mended = !diagnostics.is_sound();
    if mended {
        mend(&mut mesh, MendMethod::Repair, MendSettings::default());
        diagnostics = diagnose(&mesh);
    }
    let id = scene.insert(Imported::new(
        name,
        Arc::new(mesh),
        Transform::default(),
        ImportSummary::made(diagnostics),
    ));
    scene.move_to_plate(id, plate);
    (id, mended)
}

/// Puts each named mesh on `plate` with [`place`], and answers how many had to be mended.
pub(crate) fn place_all(
    scene: &mut Scene,
    plate: u32,
    pieces: impl IntoIterator<Item = (String, Mesh)>,
) -> usize {
    pieces
        .into_iter()
        .map(|(name, mesh)| usize::from(place(scene, plate, name, mesh).1))
        .sum()
}

/// What the stage notice notes about the pieces that had to be mended on their way down.
pub(crate) fn mending(pieces: usize) -> Option<String> {
    (pieces > 0).then(|| tr!("cut-mended", count = pieces))
}
