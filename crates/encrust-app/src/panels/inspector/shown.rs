//! The Clean surface brush, one row in Auto-orient and in the Where section of Supports, both
//! painting the same faces (ADR 0427).

use super::paint::{MAX_BRUSH_MM, MIN_BRUSH_MM};
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{brush_row, icon, secondary_button};

/// The brush and its size, with the switch that holds Auto-orient to a clean surface where
/// `with_switch`.
pub(super) fn brush(ui: &mut egui::Ui, window: &mut Window, with_switch: bool) {
    let orient = &mut window.tools.orient;
    let paint = tr!("position-paint-shown");
    let brushing = orient.shown_brush.brushing;
    let switch = with_switch.then_some(&mut orient.settings.clean_surface);
    let used = brush_row(
        ui,
        &tr!("position-clean-surface"),
        (icon::PAINT, &paint, brushing),
        &mut orient.shown_brush.radius_mm,
        MIN_BRUSH_MM..=MAX_BRUSH_MM,
        switch,
    );
    if used.toggled {
        orient.shown_brush.brushing = !brushing;
        orient.stop_picking();
    }
}

/// Clear what shows, once any picked model has a face painted.
pub(super) fn clear(ui: &mut egui::Ui, window: &mut Window) {
    let scene = &mut window.doc.scene;
    let painted = scene
        .targets()
        .any(|object| !object.shown_painted.is_empty());
    if painted && secondary_button(ui, icon::REMOVE, &tr!("position-paint-shown-clear")).clicked() {
        for object in scene.targets_mut() {
            object.shown_painted.clear();
        }
    }
}
