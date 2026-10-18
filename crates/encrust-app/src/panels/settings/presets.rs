//! The Hollow and Drain pages: the settings each tool keeps under a name (ADR 0226), as a
//! table. They are made from the tool's own Profile row, where their numbers are edited;
//! here they are named, written out, taken in and thrown away.

use std::path::Path;

use core_engine::project::{DrainState, HollowState};
use core_volume::InfillPattern;
use egui::{Rect, Ui, pos2};
use printer_profiles::{Entry, Kind, Preset};

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::frame::{SIDE_PAD, Table, child, with_footer};
use crate::panels::settings::{export_profile, form_card, name_row, report};
use crate::settings::Deleting;
use crate::state::Machine;
use crate::ui::{describe, icon, quiet_button};

const HOLLOW: Table = Table {
    heads: || {
        vec![
            tr!("table-profile"),
            tr!("hollow-wall"),
            tr!("hollow-infill"),
            tr!("presets-blocker"),
        ]
    },
    columns: &[0.0, 0.55, 0.7, 0.85],
    words: 1,
    lead: 12.0,
};

const DRAIN: Table = Table {
    heads: || {
        vec![
            tr!("table-profile"),
            tr!("support-diameter"),
            tr!("presets-depth"),
            tr!("presets-taper"),
        ]
    },
    columns: &[0.0, 0.55, 0.7, 0.85],
    words: 1,
    lead: 12.0,
};

/// What a click on the page asked for, carried out once the page has been drawn.
enum Action {
    Pick(String),
    Rename(String, String),
    Export(String),
    Ask(String),
}

/// The page of the tool the Settings window has open, laid out in `content`.
pub(super) fn page(ui: &mut Ui, content: Rect, window: &mut Window) {
    let Some(kind) = window.machine.settings.section.kind() else {
        return;
    };
    let picked = window.machine.settings.preset.clone();
    let (mut body, mut bar) = with_footer(ui, content);
    let top = body.max_rect();
    let rows = Rect::from_min_max(
        pos2(top.left() + SIDE_PAD, top.top() + 8.0),
        pos2(top.right() - SIDE_PAD, top.bottom()),
    );
    let mut action = None;
    let catalogue = &window.machine.slicing.catalogue;
    let picked_entry = picked
        .as_deref()
        .and_then(|id| catalogue.presets(kind).find(|entry| entry.id == id));
    let mut rows = child(&mut body, rows);
    if let Some(entry) = picked_entry {
        form_card(&mut rows, &tr!("settings-name"), |ui| {
            if let Some(name) = name_row(ui, kind.dir(), &entry.profile.name) {
                action = Some(Action::Rename(entry.id.clone(), name));
            }
        });
        rows.add_space(8.0);
    }
    table(
        &mut rows,
        window.machine,
        kind,
        picked.as_deref(),
        &mut action,
    );
    actions(&mut bar, picked.as_deref(), &mut action);
    if let Some(action) = action {
        apply(window.machine, kind, action);
    }
}

/// Every preset the tool keeps, with the numbers that tell them apart.
fn table(
    ui: &mut Ui,
    machine: &Machine,
    kind: Kind,
    picked: Option<&str>,
    action: &mut Option<Action>,
) {
    let columns = match kind {
        Kind::Hollow => &HOLLOW,
        _ => &DRAIN,
    };
    columns.head_row(ui);
    let mut empty = true;
    let scroll = egui::ScrollArea::vertical().id_salt(("preset-table", kind.dir()));
    scroll.auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for entry in machine.slicing.catalogue.presets(kind) {
            // A preset an older release wrote, whose numbers no longer read, is left out
            // here as it is in the tool (ADR 0226).
            let Some(cells) = cells(kind, entry) else {
                continue;
            };
            empty = false;
            let chosen = picked == Some(entry.id.as_str());
            if columns
                .row(ui, &entry.profile.name, chosen, &cells)
                .clicked()
            {
                *action = Some(Action::Pick(entry.id.clone()));
            }
        }
        if empty {
            ui.add_space(12.0);
            describe(ui, &tr!("presets-empty", kind = kind.dir()));
        }
    });
}

fn cells(kind: Kind, entry: &Entry<Preset>) -> Option<Vec<String>> {
    let path = Path::new(&entry.id);
    let name = entry.profile.name.clone();
    Some(match kind {
        Kind::Hollow => {
            let state = entry.profile.read::<HollowState>(path).ok()?;
            let infill = match state.infill_on {
                true => match state.infill.pattern {
                    InfillPattern::Hive => tr!("hollow-hive"),
                    InfillPattern::Grid => tr!("hollow-grid"),
                    InfillPattern::Scaffold => tr!("hollow-scaffold"),
                },
                false => tr!("presets-none"),
            };
            vec![
                name,
                tr!("value-mm", value = format!("{:.2}", state.thickness_mm)),
                infill,
                tr!("value-mm", value = format!("{:.1}", state.blocker_mm)),
            ]
        }
        _ => {
            let state = entry.profile.read::<DrainState>(path).ok()?;
            vec![
                name,
                tr!("value-mm", value = format!("{:.2}", state.diameter_mm)),
                tr!("value-mm", value = format!("{:.1}", state.depth_mm)),
                format!("{:.2}", state.taper),
            ]
        }
    })
}

fn actions(ui: &mut Ui, picked: Option<&str>, action: &mut Option<Action>) {
    let some = picked.is_some();
    let owned = |id: &str| id.to_owned();
    if quiet_button(ui, icon::EXPORT, &tr!("common-export"), false, some).clicked() {
        *action = picked.map(owned).map(Action::Export);
    }
    if quiet_button(ui, icon::REMOVE, &tr!("profiles-delete"), true, some).clicked() {
        *action = picked.map(owned).map(Action::Ask);
    }
}

fn apply(machine: &mut Machine, kind: Kind, action: Action) {
    match action {
        Action::Pick(id) => machine.settings.preset = Some(id),
        Action::Rename(id, name) => rename(machine, kind, &id, name),
        Action::Export(id) => export_profile(machine, kind, &id),
        Action::Ask(id) => machine.settings.confirm = Some(Deleting::Profile(kind, id)),
    }
}

/// Gives a preset a new name under the id it has: the tools find a preset by its numbers,
/// so nothing that uses it has to follow.
fn rename(machine: &mut Machine, kind: Kind, id: &str, name: String) {
    let catalogue = &mut machine.slicing.catalogue;
    let Some(entry) = catalogue.presets(kind).find(|entry| entry.id == id) else {
        return;
    };
    let preset = Preset {
        name,
        ..entry.profile.clone()
    };
    let outcome = catalogue.save_preset(kind, id, &preset);
    report(&mut machine.status, &tr!("presets-cannot-rename"), outcome);
}
