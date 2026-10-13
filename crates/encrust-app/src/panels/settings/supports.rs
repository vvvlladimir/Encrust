//! The Supports page: every support profile as a table, and the one picked opened in its
//! form, every measurement in the groups the Supports tool folds them into. A group on the
//! plate picks one of these and tunes its own copy.

use egui::{Rect, Ui, pos2};
use printer_profiles::{ContactShape, Kind, SupportProfile};

use crate::panels::Window;
use crate::panels::settings::frame::{SIDE_PAD, Table, child, scrolled, with_footer};
use crate::panels::settings::{export_profile, form_card, name_row, report, settled};
use crate::panels::support_fields::Group;
use crate::settings::{Deleting, can_delete, new_support};
use crate::state::Machine;
use crate::ui::{describe, icon, quiet_button};

/// Below this a form has no room for two columns of cards.
const TWO_COLUMNS_W: f32 = 720.0;

const TABLE: Table = Table {
    heads: &["Profile", "Tip", "Contact", "Pillar", "Spacing"],
    columns: &[0.0, 0.42, 0.6, 0.74, 0.88],
    words: 2,
    lead: 12.0,
};

/// What a click on the page asked for, carried out once the page has been drawn.
enum Action {
    Pick(String),
    Edit(String),
    Duplicate(String),
    Export(String),
    Ask(String),
}

/// The page, laid out in `content`: the form of the profile being edited, or the table.
pub(super) fn page(ui: &mut Ui, content: Rect, window: &mut Window) {
    let settings = &window.machine.settings;
    let picked = settings.support.as_ref().map(|draft| draft.id.clone());
    let editing = settings.editing_support && picked.is_some();
    let (mut body, mut bar) = with_footer(ui, content);
    let mut action = None;
    if editing {
        scrolled(&mut body, |ui| {
            if quiet_button(ui, icon::BACK, "Every profile", false, true).clicked() {
                window.machine.settings.editing_support = false;
            }
            form(ui, window);
        });
    } else {
        let top = body.max_rect();
        let rows = Rect::from_min_max(
            pos2(top.left() + SIDE_PAD, top.top() + 8.0),
            pos2(top.right() - SIDE_PAD, top.bottom()),
        );
        table(
            &mut child(&mut body, rows),
            window.machine,
            picked.as_deref(),
            &mut action,
        );
    }
    actions(
        &mut bar,
        window.machine,
        picked.as_deref(),
        editing,
        &mut action,
    );
    if let Some(action) = action {
        apply(window.machine, action);
    }
}

/// Every support profile, by name, with the measurements that tell them apart.
fn table(ui: &mut Ui, machine: &Machine, picked: Option<&str>, action: &mut Option<Action>) {
    TABLE.head_row(ui);
    let scroll = egui::ScrollArea::vertical().id_salt("support-table");
    scroll.auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for entry in machine.slicing.catalogue.supports() {
            let profile = &entry.profile;
            let row = TABLE.row(
                ui,
                &profile.name,
                picked == Some(&entry.id),
                &cells(profile),
            );
            if row.double_clicked() {
                *action = Some(Action::Edit(entry.id.clone()));
            } else if row.clicked() {
                *action = Some(Action::Pick(entry.id.clone()));
            }
        }
    });
}

fn cells(profile: &SupportProfile) -> [String; 5] {
    let tip = match profile.tip.shape {
        ContactShape::Sphere => "Sphere",
        ContactShape::Plane => "Plane",
    };
    [
        profile.name.clone(),
        tip.to_owned(),
        format!("{:.2} mm", profile.tip.contact_diameter_mm),
        format!("{:.2} mm", profile.middle.diameter_mm),
        format!("{:.1} mm", profile.contact_spacing_mm),
    ]
}

/// What the bar offers for the profile picked. A shipped profile would only come back, so
/// it cannot be deleted.
fn actions(
    ui: &mut Ui,
    machine: &Machine,
    picked: Option<&str>,
    editing: bool,
    action: &mut Option<Action>,
) {
    let some = picked.is_some();
    let owned = |id: &str| id.to_owned();
    if !editing && quiet_button(ui, icon::EDIT, "Edit", false, some).clicked() {
        *action = picked.map(owned).map(Action::Edit);
    }
    if quiet_button(ui, icon::DUPLICATE, "Duplicate", false, some).clicked() {
        *action = picked.map(owned).map(Action::Duplicate);
    }
    if quiet_button(ui, icon::EXPORT, "Export", false, some).clicked() {
        *action = picked.map(owned).map(Action::Export);
    }
    let catalogue = &machine.slicing.catalogue;
    let deletable = picked.is_some_and(|id| can_delete(catalogue, Kind::Support, id));
    let delete = quiet_button(ui, icon::REMOVE, "Delete", true, deletable);
    let delete = match deletable {
        true => delete,
        false => delete.on_disabled_hover_text("A profile this build ships cannot be deleted."),
    };
    if delete.clicked() {
        *action = picked.map(owned).map(Action::Ask);
    }
}

fn apply(machine: &mut Machine, action: Action) {
    let settings = &mut machine.settings;
    let catalogue = &machine.slicing.catalogue;
    match action {
        Action::Pick(id) => settings.pick_support(catalogue, &id),
        Action::Edit(id) => {
            settings.pick_support(catalogue, &id);
            settings.editing_support = true;
        }
        Action::Duplicate(id) => add(machine, Some(&id)),
        Action::Export(id) => export_profile(machine, Kind::Support, &id),
        Action::Ask(id) => settings.confirm = Some(Deleting::Profile(Kind::Support, id)),
    }
}

/// A new support profile with the stock numbers, or a copy of `from`, opened in the form.
pub(super) fn add(machine: &mut Machine, from: Option<&str>) {
    let outcome = new_support(&mut machine.slicing.catalogue, from);
    if let Some(id) = report(&mut machine.status, "cannot add a support profile", outcome) {
        machine
            .settings
            .pick_support(&machine.slicing.catalogue, &id);
        machine.settings.editing_support = true;
    }
}

/// The profile open in the form, written back as it is edited.
fn form(ui: &mut Ui, window: &mut Window) {
    let Some(draft) = window.machine.settings.support.as_mut() else {
        describe(ui, "Pick a support profile.");
        return;
    };

    form_card(ui, "Profile", |ui| {
        if let Some(name) = name_row(ui, "support", &draft.values.name) {
            draft.values.name = name;
        }
    });

    let profile = &mut draft.values;
    if ui.available_width() < TWO_COLUMNS_W {
        for group in Group::ALL {
            form_card(ui, group.label(), |ui| group.show(ui, profile, None));
        }
    } else {
        let (left, right) = Group::ALL.split_at(Group::ALL.len().div_ceil(2));
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.columns(2, |columns| {
            for (ui, groups) in columns.iter_mut().zip([left, right]) {
                for group in groups {
                    form_card(ui, group.label(), |ui| group.show(ui, profile, None));
                }
            }
        });
    }

    if settled(ui) {
        autosave(window.machine);
    }
}

fn autosave(machine: &mut Machine) {
    let Some(draft) = machine.settings.support.as_mut() else {
        return;
    };
    if !draft.is_dirty() {
        return;
    }
    let outcome = machine
        .slicing
        .catalogue
        .save_support(&draft.id, &draft.values);
    if report(
        &mut machine.status,
        "cannot save the support profile",
        outcome,
    )
    .is_some()
    {
        draft.mark_saved();
    }
}
