//! The buttons that answer what the layers show: a suction cup, a weak neck, a hard peel.

use core_analysis::Risk;
use core_plate::{OrientGoal, OrientPreset};

use super::issues::Filter;
use crate::check::{cup_owners, drill_cups, lever_owners, only_owner, support_levers};
use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::ObjectId;
use crate::shortcuts;
use crate::ui::{describe, icon, secondary_button, section};
use crate::workspace::Tool;

/// Buttons side by side, greyed while `enabled` is false. Answers which was pressed.
pub(super) fn button_row(
    ui: &mut egui::Ui,
    enabled: bool,
    buttons: &[(&str, String)],
) -> Option<usize> {
    let mut pressed = None;
    ui.add_enabled_ui(enabled, |ui| {
        ui.columns(buttons.len(), |columns| {
            for (index, ((glyph, label), column)) in buttons.iter().zip(columns).enumerate() {
                if secondary_button(column, glyph, label).clicked() {
                    pressed = Some(index);
                }
            }
        });
    });
    pressed
}

/// Whether a fix may start: no recipe running, and no Auto-orient already going.
pub(super) fn free(window: &Window) -> bool {
    crate::prepare::holding(window.tools).is_none() && !window.tools.orient.is_running()
}

/// Starts Auto-orient for `preset`, on `only` or on the whole plate.
fn turn(window: &mut Window, preset: OrientPreset, only: Option<ObjectId>) {
    window.tools.orient.settings.preset = preset;
    crate::panels::auto_orient(window, only);
}

fn has(risks: &[Risk], filter: Filter) -> bool {
    risks.iter().any(|risk| filter.admits(&risk.kind))
}

/// A section for one kind of issue, where there is any: `first`'s button beside Auto-orient.
/// Answers which was pressed.
fn fix_section(
    ui: &mut egui::Ui,
    window: &Window,
    (risks, filter): (&[Risk], Filter),
    (title, help): (String, String),
    first: (&str, String),
) -> Option<usize> {
    if !has(risks, filter) {
        return None;
    }
    let enabled = free(window);
    section(ui, &title, None, |ui| {
        describe(ui, &help);
        let buttons = [first, (icon::AUTO_ORIENT, tr!("check-auto-orient"))];
        button_row(ui, enabled, &buttons)
    })
    .flatten()
}

/// The cups' own section: a hole through each one's floor, or a pose that leaves none.
pub(super) fn cups(ui: &mut egui::Ui, window: &mut Window, risks: &[Risk]) {
    let words = (tr!("check-section-cups"), tr!("check-cups-help"));
    let first = (icon::DRAIN, tr!("check-drill-cups"));
    let Some(pressed) = fix_section(ui, window, (risks, Filter::Cups), words, first) else {
        return;
    };
    let (Some(settings), Some(plan)) = (
        window.machine.slicing.raster_settings(),
        window.machine.preview.plan(),
    ) else {
        return;
    };
    let stack = (plan, &settings);
    if pressed == 0 {
        let plate = (&mut window.doc.scene, &mut window.tools.drain);
        window.machine.status = drill_cups(plate, stack, risks, None);
    } else {
        let owners = cup_owners(&window.doc.scene, stack, risks);
        let only = only_owner(owners.iter().map(|owner| owner.map(|(id, _)| id)));
        turn(window, OrientPreset::Auto, only);
    }
}

/// The weak necks' own section: a support under each, or a pose that leaves none.
pub(super) fn levers(ui: &mut egui::Ui, window: &mut Window, risks: &[Risk]) {
    let words = (tr!("check-section-levers"), tr!("check-levers-help"));
    let first = (icon::SUPPORTS, tr!("check-support-levers"));
    let Some(pressed) = fix_section(ui, window, (risks, Filter::Levers), words, first) else {
        return;
    };
    let (Some(settings), Some(plan)) = (
        window.machine.slicing.raster_settings(),
        window.machine.preview.plan(),
    ) else {
        return;
    };
    let stack = (plan, &settings);
    if pressed == 0 {
        let plate = (&mut window.doc.scene, &mut window.tools.supports);
        window.machine.status = support_levers(plate, stack, risks, None);
    } else {
        let owners = lever_owners(&window.doc.scene, stack, risks);
        let only = only_owner(owners.iter().map(|owner| owner.map(|(id, _)| id)));
        turn(window, OrientPreset::Auto, only);
    }
}

/// Under the peel readings where a layer pulls too hard: the pose that pulls least, and the
/// lift settings on a printer that leaves them to the file.
pub(super) fn peel(ui: &mut egui::Ui, window: &mut Window, risks: &[Risk]) {
    if !has(risks, Filter::Peel) {
        return;
    }
    let lift_is_ours = window
        .machine
        .slicing
        .printer
        .as_ref()
        .is_some_and(|printer| printer.locked.lift_distance_mm.is_none());
    let mut buttons = vec![(icon::AUTO_ORIENT, tr!("check-least-peel"))];
    if lift_is_ours {
        buttons.push((icon::PARAMETERS, tr!("check-lift-settings")));
    }
    match button_row(ui, free(window), &buttons) {
        Some(0) => {
            let least_peel = OrientPreset::Goal(OrientGoal::LeastPeel);
            turn(window, least_peel, None);
        }
        Some(_) => shortcuts::pick(window, Tool::PrintSettings),
        None => {}
    }
}
