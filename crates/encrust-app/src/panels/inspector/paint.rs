use core_geometry::Scalar;
use core_supports::InfillPattern;

use crate::i18n::tr;
use crate::panels::Window;
use crate::supports::Placing;
use crate::ui::{
    DropdownRow, Segment, Segmented, describe, dropdown_menu, dropdown_row, hint, icon, nested,
    number_row, picker_row, secondary_button, section, switch,
};

/// Millimetres per point of drag on the brush and the fill.
pub(super) const COARSE_STEP: f64 = 0.05;
const ANGLE_STEP: f64 = 0.5;

/// Bounds on the brush and the fill, millimetres of plate: finer than 0.1 paints one
/// triangle at a time, wider than the plate paints all of it.
pub(super) const MIN_BRUSH_MM: f32 = 0.1;
pub(super) const MAX_BRUSH_MM: f32 = 25.0;
const MIN_SPACING_MM: f32 = 0.5;
const MAX_SPACING_MM: f32 = 20.0;
const DEFAULT_SPACING_MM: f32 = 3.0;
const DEFAULT_SECOND_RIM_MM: f32 = 1.0;
/// The furthest in a rim or the inside may be asked to start, millimetres.
const MAX_OFFSET_MM: f32 = 10.0;

/// Bounds on how far a painted surface may turn from the face that was clicked.
const MIN_FLOOD_DEG: f32 = 1.0;
const MAX_FLOOD_DEG: f32 = 180.0;

/// What a placing mode does. Short, because it stays on screen under the switch for as
/// long as the tool is in hand.
fn paint_hint(placing: Placing) -> String {
    match placing {
        Placing::Point => tr!("paint-hint-point"),
        Placing::Paint => tr!("paint-hint-paint"),
        Placing::Block => tr!("paint-hint-block"),
        Placing::Erase => tr!("paint-hint-erase"),
    }
}

/// The place mode: a click puts one support down, a shift-click a line of them.
pub(super) fn place_fields(ui: &mut egui::Ui, window: &mut Window) {
    hint(ui, &paint_hint(Placing::Point));
    ui.add_space(4.0);
    number_row(
        ui,
        &tr!("paint-line-spacing"),
        &mut window.tools.supports.inline_spacing_mm,
        &tr!("unit-mm"),
        COARSE_STEP,
        MIN_SPACING_MM..=MAX_SPACING_MM,
        2,
    );
}

/// The area mode: paint or block, the brush, and the way to take that layer off again.
/// Alt erases whichever is in hand, so an eraser left from before reads as paint.
pub(super) fn area_fields(ui: &mut egui::Ui, window: &mut Window) {
    let tool = &mut window.tools.supports;
    if tool.placing == Placing::Erase {
        tool.placing = Placing::Paint;
    }
    let segments =
        [Placing::Paint, Placing::Block].map(|placing| Segment::new(placing, placing.label()));
    Segmented::new(&segments)
        .width(ui.available_width())
        .show(ui, &mut tool.placing);
    ui.add_space(4.0);
    hint(ui, &paint_hint(tool.placing));
    ui.add_space(4.0);
    number_row(
        ui,
        &tr!("paint-brush"),
        &mut tool.brush_radius_mm,
        &tr!("unit-mm"),
        COARSE_STEP,
        MIN_BRUSH_MM..=MAX_BRUSH_MM,
        2,
    );
    number_row(
        ui,
        &tr!("paint-surface-angle"),
        &mut tool.flood_angle_deg,
        &tr!("unit-deg"),
        ANGLE_STEP,
        MIN_FLOOD_DEG..=MAX_FLOOD_DEG,
        0,
    );
    clear(ui, window);
}

/// How a painted area is filled: its rim and its inside.
pub(super) fn fill_section(ui: &mut egui::Ui, window: &mut Window) {
    section(ui, &tr!("supports-placing-fill"), None, |ui| {
        describe(ui, &tr!("paint-fill-help"));
        rim_rows(ui, window);
        inside_rows(ui, window);
    });
}

/// Taking the paint, or the blocking, off again; with the eraser in hand, both.
fn clear(ui: &mut egui::Ui, window: &mut Window) {
    let (label, layers): (String, &[bool]) = match window.tools.supports.placing {
        Placing::Block => (tr!("paint-clear-blocked"), &[true]),
        Placing::Erase => (tr!("paint-clear-both"), &[false, true]),
        Placing::Point | Placing::Paint => (tr!("paint-clear-painted"), &[false]),
    };
    if secondary_button(ui, icon::REMOVE, &label).clicked() {
        for object in window.doc.scene.targets_mut() {
            for blocking in layers {
                object.supports.clear_paint(*blocking);
            }
        }
    }
}

/// The fill's rim: its spacing, how far in it stands, a second rim inside it and the
/// profile it is built to.
fn rim_rows(ui: &mut egui::Ui, window: &mut Window) {
    let fill = &mut window.tools.supports.fill;
    if !half_switch(ui, &tr!("paint-rim"), &mut fill.border_spacing_mm) {
        return;
    }
    nested(ui, |ui| {
        if let Some(spacing_mm) = fill.border_spacing_mm.as_mut() {
            spacing_row(ui, spacing_mm);
        }
        offset_row(ui, &tr!("paint-offset"), &mut fill.border_offset_mm);
        let mut second = fill.second_border_mm.is_some();
        if switch(ui, &mut second, &tr!("paint-second-rim")).changed() {
            fill.second_border_mm = second.then_some(DEFAULT_SECOND_RIM_MM);
        }
        if let Some(gap_mm) = fill.second_border_mm.as_mut() {
            offset_row(ui, &tr!("paint-second-rim-gap"), gap_mm);
        }
        let catalogue = &window.machine.slicing.catalogue;
        profile_row(
            ui,
            catalogue,
            &mut window.tools.supports.fill_profiles.rim,
            tr!("paint-profile-in-hand"),
        );
    });
}

/// The fill's inside: its spacing, its pattern, how far inside the rim it begins and the
/// profile it is built to.
fn inside_rows(ui: &mut egui::Ui, window: &mut Window) {
    let fill = &mut window.tools.supports.fill;
    if !half_switch(ui, &tr!("paint-inside"), &mut fill.infill_spacing_mm) {
        return;
    }
    nested(ui, |ui| {
        if let Some(spacing_mm) = fill.infill_spacing_mm.as_mut() {
            spacing_row(ui, spacing_mm);
        }
        let segments = [
            Segment::new(InfillPattern::Grid, tr!("paint-pattern-grid")),
            Segment::new(InfillPattern::Concentric, tr!("paint-pattern-rings")),
            Segment::new(InfillPattern::Random, tr!("paint-pattern-random")),
        ];
        let width = ui.available_width();
        Segmented::new(&segments)
            .width(width)
            .show(ui, &mut fill.pattern);
        offset_row(ui, &tr!("paint-offset"), &mut fill.infill_offset_mm);
        let catalogue = &window.machine.slicing.catalogue;
        profile_row(
            ui,
            catalogue,
            &mut window.tools.supports.fill_profiles.inside,
            tr!("paint-profile-in-hand"),
        );
    });
}

/// The switch that leaves one half of the fill out. Answers whether that half is on.
fn half_switch(ui: &mut egui::Ui, label: &str, spacing_mm: &mut Option<Scalar>) -> bool {
    let mut on = spacing_mm.is_some();
    if switch(ui, &mut on, label).changed() {
        *spacing_mm = on.then_some(DEFAULT_SPACING_MM);
    }
    on
}

/// How far apart the supports of one half of the fill stand.
fn spacing_row(ui: &mut egui::Ui, spacing_mm: &mut Scalar) {
    number_row(
        ui,
        &tr!("support-table-spacing"),
        spacing_mm,
        &tr!("unit-mm"),
        COARSE_STEP,
        MIN_SPACING_MM..=MAX_SPACING_MM,
        2,
    );
}

/// A distance measured in from the patch's outline, or in from the rim.
fn offset_row(ui: &mut egui::Ui, label: &str, offset_mm: &mut Scalar) {
    number_row(
        ui,
        label,
        offset_mm,
        &tr!("unit-mm"),
        COARSE_STEP,
        0.0..=MAX_OFFSET_MM,
        2,
    );
}

/// The saved profile a kind of support is built to, or `in_hand` naming what it is built
/// to with none chosen.
pub(super) fn profile_row(
    ui: &mut egui::Ui,
    catalogue: &printer_profiles::Catalogue,
    chosen: &mut Option<String>,
    in_hand: String,
) {
    let shown = chosen
        .as_deref()
        .and_then(|id| catalogue.supports().find(|entry| entry.id == id))
        .map_or(in_hand.clone(), |entry| entry.profile.name.clone());
    let response = picker_row(ui, &tr!("paint-profile"), icon::SUPPORTS, &shown);
    dropdown_menu(&response, |ui| {
        if dropdown_row(ui, &DropdownRow::named(&in_hand, chosen.is_none())).clicked() {
            *chosen = None;
            ui.close();
        }
        let offered = catalogue
            .supports()
            .filter(|entry| !catalogue.is_auto_shape(&entry.id));
        for entry in offered {
            let picked = chosen.as_deref() == Some(entry.id.as_str());
            if dropdown_row(ui, &DropdownRow::named(&entry.profile.name, picked)).clicked() {
                *chosen = Some(entry.id.clone());
                ui.close();
            }
        }
    });
}
