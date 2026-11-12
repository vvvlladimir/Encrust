//! The Clean surface switch and its brush, in Auto-orient and in the Where section of
//! Supports, both painting the same faces (ADR 0425).

use super::paint::{COARSE_STEP, MAX_BRUSH_MM, MIN_BRUSH_MM};
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{icon, number_row, secondary_button, switch_with_toggle};

/// The switch, its brush the toggle while it is on, and the brush's size while it paints;
/// `about` says what the switch does where it stands. Clear what shows once any is painted.
pub(super) fn clean_surface(ui: &mut egui::Ui, window: &mut Window, about: &str) {
    let orient = &mut window.tools.orient;
    let paint = tr!("position-paint-shown");
    let brushing = orient.shown_brush.brushing;
    let toggled = switch_with_toggle(
        ui,
        &mut orient.settings.clean_surface,
        &tr!("position-clean-surface"),
        (icon::PAINT, &paint, brushing),
    );
    if !orient.settings.clean_surface {
        orient.shown_brush.brushing = false;
        return;
    }
    if toggled {
        orient.shown_brush.brushing = !brushing;
        orient.stop_picking();
    }
    crate::ui::describe(ui, about);
    let brush = &mut orient.shown_brush;
    if brush.brushing {
        number_row(
            ui,
            &tr!("paint-brush"),
            &mut brush.radius_mm,
            &tr!("unit-mm"),
            COARSE_STEP,
            MIN_BRUSH_MM..=MAX_BRUSH_MM,
            2,
        );
    }
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
