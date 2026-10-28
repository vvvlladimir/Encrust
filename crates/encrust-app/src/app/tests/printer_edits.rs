use printer_profiles::PlateInset;

use super::super::states::{PRINTER, app};

/// BUG: an inset saved in the printer form showed only after picking another printer and
/// back, because an edit that keeps the volume left the drawn plate as it was.
#[test]
fn a_saved_inset_shows_on_the_plate_at_once() {
    let mut app = app(false);
    let mut profile = app
        .machine
        .slicing
        .printer
        .clone()
        .expect("the pictured printer is in use");
    let inset = PlateInset {
        min_x_mm: 1.5,
        ..PlateInset::default()
    };
    profile.plate_inset = Some(inset);
    crate::profiles::printer_edited(&mut app.window(), profile, PRINTER.to_owned());
    assert_eq!(app.doc.plate.inset, inset);
}
