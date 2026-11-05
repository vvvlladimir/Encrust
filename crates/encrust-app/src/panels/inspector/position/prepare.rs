//! The Select card's Prepare for print: `--prepare` on the picked models, and what it decided
//! for the one in the card (ADR 0402).

use crate::i18n::tr;
use crate::panels::Window;
use crate::prepare::Prepared;
use crate::scene::ObjectId;
use core_volume::HollowSettings;

use crate::ui::{describe, icon, readings, secondary_button_unless, section, step_bar, subheading};

mod plan;

/// The button, the run's step while it goes, and what the last run did to model `id`.
pub(super) fn prepare_section(ui: &mut egui::Ui, window: &mut Window, id: ObjectId) {
    section(ui, &tr!("position-prepare"), None, |ui| {
        let wall_mm = format!("{:.2}", HollowSettings::default().thickness_mm);
        let wall = tr!("value-mm", value = wall_mm);
        describe(ui, &tr!("position-prepare-help-wall", wall = wall));
        describe(ui, &tr!("position-prepare-help-plan"));
        plan::picker(ui, window);
        if let Some((step, share)) = window.tools.prepare.progress() {
            if step_bar(ui, &step.label(), Some(share), true) {
                crate::prepare::cancel(window.doc, window.tools, window.machine);
            }
        } else {
            let blocked = crate::prepare::blocker(window.doc, window.tools, window.machine);
            let scope = window.doc.scene.scope();
            let label = tr!("position-prepare-run");
            let button = secondary_button_unless(ui, icon::PREPARE, &label, blocked.as_deref());
            let tip = tr!("position-prepare-tooltip", scope = scope);
            if button.on_hover_text(tip).clicked() {
                start(window);
            }
        }
        plan::plan(ui, window);
        let object = window.doc.scene.get(id);
        if let Some(prepared) = object.and_then(|object| window.tools.prepare.prepared(object)) {
            subheading(ui, &tr!("position-prepared"));
            readings(ui, &rows(prepared));
        }
    });
}

/// Starts Prepare for print, and says on the stage why when it cannot.
pub(crate) fn start(window: &mut Window) {
    let started = crate::prepare::start(window.doc, window.tools, window.machine);
    if let Err(error) = started {
        window.machine.status = crate::status::Status::failed(&error);
    }
}

/// What the run did, as the command line's `prepared` lists it.
fn rows(prepared: &Prepared) -> Vec<(String, String)> {
    let said = |done: bool| {
        if done {
            tr!("position-prepared-yes")
        } else {
            tr!("position-prepared-no")
        }
    };
    let mut rows = vec![
        (tr!("position-prepared-oriented"), said(prepared.oriented)),
        (tr!("position-prepared-repaired"), said(prepared.repaired)),
    ];
    match prepared.hollowed {
        Some(wall) => {
            let wall_mm = format!("{:.2}", wall.wall_mm);
            let saved_ml = format!("{:.2}", wall.saves_mm3 / 1000.0);
            rows.push((
                tr!("position-prepared-wall"),
                tr!("value-mm", value = wall_mm),
            ));
            rows.push((
                tr!("position-prepared-saved"),
                tr!("value-ml", value = saved_ml),
            ));
        }
        None => rows.push((tr!("position-prepared-hollowed"), said(false))),
    }
    rows.push((tr!("position-prepared-drained"), said(prepared.drained)));
    let supports = prepared
        .supports
        .clone()
        .unwrap_or_else(|| tr!("position-prepared-none"));
    rows.push((tr!("position-prepared-supports"), supports));
    rows
}
