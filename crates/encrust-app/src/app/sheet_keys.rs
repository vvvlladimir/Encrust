//! Esc over a sheet that stands above the plate.

use super::states::app;
use crate::shortcuts::{Action, act};

/// BUG: Esc reached the plate under the update and printer sheets and dropped the selection
/// instead of closing them.
#[test]
fn esc_closes_the_update_sheet_and_keeps_the_selection() {
    let mut app = app(true);
    app.machine.updates.sheet = true;
    act(&mut app.window(), Action::Deselect);
    assert!(!app.machine.updates.sheet, "the sheet closes");
    assert!(app.doc.scene.has_selection(), "the plate keeps its pick");
}

#[test]
fn esc_closes_the_printer_sheet_and_keeps_the_selection() {
    let mut app = app(true);
    app.machine.network.open = Some("printer".to_owned());
    act(&mut app.window(), Action::Deselect);
    assert_eq!(app.machine.network.open, None, "the sheet closes");
    assert!(app.doc.scene.has_selection(), "the plate keeps its pick");
}
