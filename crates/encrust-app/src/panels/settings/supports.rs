//! The Supports page: every support profile as a table, the picked one opened in its form.
//! A group on the plate picks one and tunes its own copy.

mod auto;
mod form;

use egui::{Rect, Ui, pos2};
use printer_profiles::{AutoProfile, Catalogue, ContactShape, Kind, SupportProfile};

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::frame::{SIDE_PAD, Table, child, scrolled, with_footer};
use crate::panels::settings::{export_profile, report};
use crate::settings::{Deleting, Settings, can_delete, new_support};
use crate::state::Machine;
use crate::ui::{describe, icon, quiet_button, secondary_button, subheading};

/// Below this a form has no room for two columns of cards.
pub(super) const TWO_COLUMNS_W: f32 = 720.0;

const TABLE: Table = Table {
    heads: || {
        vec![
            tr!("table-profile"),
            tr!("support-table-tip"),
            tr!("support-contact"),
            tr!("support-group-pillar"),
            tr!("support-table-spacing"),
        ]
    },
    columns: &[0.0, 0.42, 0.6, 0.74, 0.88],
    words: 2,
    lead: 12.0,
};

/// What a click on the page asked for, carried out once the page has been drawn.
enum Action {
    Pick(Kind, String),
    Edit(Kind, String),
    Duplicate(String),
    Export(String),
    Ask(String),
}

/// The profile picked on the page: an automatic one, or a support profile.
fn picked(settings: &Settings) -> Option<(Kind, String)> {
    match (&settings.auto, &settings.support) {
        (Some(auto), _) => Some((Kind::Auto, auto.id.clone())),
        (None, Some(support)) => Some((Kind::Support, support.id.clone())),
        (None, None) => None,
    }
}

/// The page, laid out in `content`: the form of the profile being edited, or the table.
pub(super) fn page(ui: &mut Ui, content: Rect, window: &mut Window) {
    let settings = &window.machine.settings;
    let picked = picked(settings);
    let editing = settings.editing_support && picked.is_some();
    let (mut body, mut bar) = with_footer(ui, content);
    let mut action = None;
    if editing {
        scrolled(&mut body, |ui| {
            if quiet_button(ui, icon::BACK, &tr!("profiles-every"), false, true).clicked() {
                window.machine.settings.editing_support = false;
            }
            match picked {
                Some((Kind::Auto, _)) => auto::form(ui, window),
                _ => form::form(ui, window),
            }
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
            picked.as_ref(),
            &mut action,
        );
    }
    actions(
        &mut bar,
        window.machine,
        picked.as_ref(),
        editing,
        &mut action,
    );
    if let Some(action) = action {
        apply(window.machine, action);
    }
}

/// Every support profile, by name, with the measurements that tell them apart: the
/// automatic profiles and the shapes they build to first, then those chosen by hand.
fn table(
    ui: &mut Ui,
    machine: &Machine,
    picked: Option<&(Kind, String)>,
    action: &mut Option<Action>,
) {
    let catalogue = &machine.slicing.catalogue;
    TABLE.head_row(ui);
    let scroll = egui::ScrollArea::vertical().id_salt("support-table");
    scroll.auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for automatic in [true, false] {
            let mut entries: Vec<_> = catalogue
                .supports()
                .filter(|entry| catalogue.is_auto_shape(&entry.id) == automatic)
                .collect();
            if entries.is_empty() {
                continue;
            }
            // Lightest first, as automatic sizing reaches for them.
            entries.sort_by_key(|entry| auto_rank(catalogue, &entry.id));
            part_heading(
                ui,
                &if automatic {
                    tr!("support-table-auto")
                } else {
                    tr!("support-table-manual")
                },
            );
            if automatic {
                for entry in catalogue.autos() {
                    let row = (Kind::Auto, entry.id.clone());
                    let name = &entry.profile.name;
                    let clicked =
                        TABLE.row(ui, name, picked == Some(&row), &auto_cells(&entry.profile));
                    on_row(clicked, row, action);
                }
            }
            for entry in entries {
                let profile = &entry.profile;
                let row = (Kind::Support, entry.id.clone());
                let clicked = TABLE.row(ui, &profile.name, picked == Some(&row), &cells(profile));
                on_row(clicked, row, action);
            }
        }
    });
}

/// A double click opens the row's profile in the form, a click picks it.
fn on_row(response: egui::Response, (kind, id): (Kind, String), action: &mut Option<Action>) {
    if response.double_clicked() {
        *action = Some(Action::Edit(kind, id));
    } else if response.clicked() {
        *action = Some(Action::Pick(kind, id));
    }
}

/// Where `id` stands among the shapes of the automatic profiles, lightest first, and after
/// all of them when it is none of theirs.
fn auto_rank(catalogue: &Catalogue, id: &str) -> usize {
    catalogue
        .autos()
        .find_map(|auto| auto.profile.every_shape().position(|shape| shape == id))
        .unwrap_or(usize::MAX)
}

/// The heading over one part of the table.
fn part_heading(ui: &mut Ui, title: &str) {
    ui.add_space(12.0);
    subheading(ui, title);
    ui.add_space(4.0);
}

/// An automatic profile's row: no shape of its own, and the spacing it places by.
fn auto_cells(auto: &AutoProfile) -> [String; 5] {
    let none = || "—".to_owned();
    [
        auto.name.clone(),
        none(),
        none(),
        none(),
        tr!(
            "value-mm",
            value = format!("{:.1}", auto.contact_spacing_mm)
        ),
    ]
}

fn cells(profile: &SupportProfile) -> [String; 5] {
    let tip = match profile.tip.shape {
        ContactShape::Sphere => tr!("support-shape-sphere"),
        ContactShape::Plane => tr!("support-shape-plane"),
    };
    [
        profile.name.clone(),
        tip.to_owned(),
        tr!(
            "value-mm",
            value = format!("{:.2}", profile.tip.contact_diameter_mm)
        ),
        tr!(
            "value-mm",
            value = format!("{:.2}", profile.middle.diameter_mm)
        ),
        tr!(
            "value-mm",
            value = format!("{:.1}", profile.contact_spacing_mm)
        ),
    ]
}

/// What the bar offers for the profile picked. A shipped profile would only come back, so
/// it cannot be deleted, and an automatic profile is only ever the shipped one's copy, so it
/// is neither copied nor exported.
fn actions(
    ui: &mut Ui,
    machine: &Machine,
    picked: Option<&(Kind, String)>,
    editing: bool,
    action: &mut Option<Action>,
) {
    let some = picked.is_some();
    let picked = picked.cloned();
    if !editing && quiet_button(ui, icon::EDIT, &tr!("profiles-edit"), false, some).clicked() {
        *action = picked.clone().map(|(kind, id)| Action::Edit(kind, id));
    }
    let support = picked
        .clone()
        .filter(|(kind, _)| *kind == Kind::Support)
        .map(|(_, id)| id);
    let copyable = support.is_some();
    if quiet_button(
        ui,
        icon::DUPLICATE,
        &tr!("action-duplicate"),
        false,
        copyable,
    )
    .clicked()
    {
        *action = support.clone().map(Action::Duplicate);
    }
    if quiet_button(ui, icon::EXPORT, &tr!("common-export"), false, copyable).clicked() {
        *action = support.clone().map(Action::Export);
    }
    let catalogue = &machine.slicing.catalogue;
    let deletable = support
        .as_deref()
        .is_some_and(|id| can_delete(catalogue, Kind::Support, id));
    let delete = quiet_button(ui, icon::REMOVE, &tr!("profiles-delete"), true, deletable);
    let delete = match deletable {
        true => delete,
        false => delete.on_disabled_hover_text(tr!("profiles-shipped")),
    };
    if delete.clicked() {
        *action = support.map(Action::Ask);
    }
}

fn apply(machine: &mut Machine, action: Action) {
    let settings = &mut machine.settings;
    let catalogue = &machine.slicing.catalogue;
    match action {
        Action::Pick(kind, id) => pick(settings, catalogue, kind, &id),
        Action::Edit(kind, id) => {
            pick(settings, catalogue, kind, &id);
            settings.editing_support = true;
        }
        Action::Duplicate(id) => add(machine, Some(&id)),
        Action::Export(id) => export_profile(machine, Kind::Support, &id),
        Action::Ask(id) => settings.confirm = Some(Deleting::Profile(Kind::Support, id)),
    }
}

fn pick(settings: &mut Settings, catalogue: &Catalogue, kind: Kind, id: &str) {
    match kind {
        Kind::Auto => settings.pick_auto(catalogue, id),
        _ => settings.pick_support(catalogue, id),
    }
}

/// A new support profile with the stock numbers, or a copy of `from`, opened in the form.
pub(super) fn add(machine: &mut Machine, from: Option<&str>) {
    let outcome = new_support(&mut machine.slicing.catalogue, from);
    if let Some(id) = report(&mut machine.status, &tr!("support-cannot-add"), outcome) {
        machine
            .settings
            .pick_support(&machine.slicing.catalogue, &id);
        machine.settings.editing_support = true;
    }
}

/// Puts every number of `profile` back as `stock` has them, explained by `help`; undo in
/// the form takes it back.
fn reset_row<T: PartialEq>(ui: &mut Ui, profile: &mut T, stock: T, help: &str) {
    describe(ui, help);
    ui.add_space(4.0);
    let changed = stock != *profile;
    ui.add_enabled_ui(changed, |ui| {
        if secondary_button(ui, icon::RESET, &tr!("support-reset")).clicked() {
            *profile = stock;
        }
    });
}
