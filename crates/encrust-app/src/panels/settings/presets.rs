//! The Hollow and Drain pages: each tool's named settings (ADR 0226) as a table, renamed,
//! written, taken in and removed here; their numbers are edited in the tool's Profile row.

use std::path::Path;

use core_engine::project::{DrainState, HollowState};
use core_volume::InfillPattern;
use egui::{Rect, Ui, pos2};
use printer_profiles::{Entry, Kind, Preset};

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::{export_profile, forget_profile, report};
use crate::state::Machine;
use crate::ui::window_frame::{SIDE_PAD, Table, child, with_footer};
use crate::ui::{
    Confirm, confirm_button, describe, icon, is_renaming, quiet_button, rename_box, start_rename,
};
use crate::workspace::Tool;

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
    Delete(String),
    OpenTool,
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
    table(
        &mut child(&mut body, rows),
        window.machine,
        kind,
        picked.as_deref(),
        &mut action,
    );
    actions(&mut bar, picked.as_deref(), &mut action);
    if let Some(action) = action {
        apply(window, kind, action);
    }
}

/// Closes the window on the tool whose presets the page lists, its card open.
fn open_tool(window: &mut Window, kind: Kind) {
    window.machine.settings.close();
    *window.tool = match kind {
        Kind::Hollow => Tool::Hollow,
        _ => Tool::Drain,
    };
    window.view.options.inspector = true;
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
            if let Some(asked) = row(ui, columns, entry, chosen, cells) {
                *action = Some(asked);
            }
        }
        if empty {
            ui.add_space(crate::ui::theme::PANEL_PAD);
            describe(ui, &tr!("presets-empty", kind = kind.dir()));
        }
        ui.add_space(crate::ui::theme::PANEL_PAD);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let open = tr!("presets-open-tool", kind = kind.dir());
            if quiet_button(ui, icon::ELSEWHERE, &open, false, true).clicked() {
                *action = Some(Action::OpenTool);
            }
        });
    });
}

/// One preset's row: a click picks it, a double click turns its name into a box to type a
/// new one in.
fn row(
    ui: &mut Ui,
    columns: &Table,
    entry: &Entry<Preset>,
    chosen: bool,
    mut cells: Vec<String>,
) -> Option<Action> {
    let name = &entry.profile.name;
    let id = ui.id().with(("rename-preset", &entry.id));
    let renaming = is_renaming(ui, id);
    if renaming {
        cells[0].clear();
    }
    let response = columns.row(ui, name, chosen, &cells);
    if renaming {
        let typed = rename_box(ui, columns.name_box(response.rect), id);
        return typed
            .filter(|typed| typed != name)
            .map(|typed| Action::Rename(entry.id.clone(), typed));
    }
    if response.double_clicked() {
        start_rename(ui, id, name);
    }
    (response.clicked() || response.double_clicked()).then(|| Action::Pick(entry.id.clone()))
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
    let delete = Confirm {
        glyph: icon::REMOVE,
        text: &tr!("profiles-delete"),
        asked: &tr!("confirm-asked-delete"),
        tooltip: &tr!("confirm-cannot-undo"),
        enabled: some,
    };
    if confirm_button(ui, &delete) {
        *action = picked.map(owned).map(Action::Delete);
    }
}

fn apply(window: &mut Window, kind: Kind, action: Action) {
    let machine = &mut *window.machine;
    match action {
        Action::Pick(id) => machine.settings.preset = Some(id),
        Action::Rename(id, name) => rename(machine, kind, &id, name),
        Action::Export(id) => export_profile(machine, kind, &id),
        Action::Delete(id) => forget_profile(machine, kind, &id),
        Action::OpenTool => open_tool(window, kind),
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
