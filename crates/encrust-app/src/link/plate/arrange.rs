//! The arrange edit as the window carries it out: the plate's models spread at once.

use core_edit::{Edit, ObjectId};
use printer_profiles::PlateInset;

use crate::arrange::arrange_plate;
use crate::panels::Window;
use crate::plate::BuildPlate;

/// Spreads the plate's models over one `plate_mm` across at once, answering `None`; refused
/// where the window's Arrange ends in an error.
pub(super) fn start_arrange(window: &mut Window, edit: &Edit) -> Result<Option<ObjectId>, String> {
    let &Edit::Arrange {
        plate_mm,
        clearance_mm,
    } = edit
    else {
        return Ok(None);
    };
    let plate = BuildPlate {
        x_mm: plate_mm[0],
        y_mm: plate_mm[1],
        inset: PlateInset::default(),
        ..window.doc.plate.clone()
    };
    let status = arrange_plate(&mut window.doc.scene, &plate, clearance_mm);
    let refused = status.is_error().then(|| status.text());
    window.machine.status = status;
    refused.map_or(Ok(None), Err)
}
