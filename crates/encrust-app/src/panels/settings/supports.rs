//! The Supports page: every support profile as a table, the picked one opened in its form.
//! A group on the plate picks one and tunes its own copy.

mod auto;
mod form;

use egui::{Rect, Ui, pos2, vec2};
use printer_profiles::{AutoProfile, Catalogue, ContactShape, Kind, SupportProfile};

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::confirm::cost;
use crate::panels::settings::{export_profile, forget_profile, jump, report};
use crate::settings::{Deleting, Settings, can_delete, new_support};
use crate::state::Machine;
use crate::ui::window_frame::{SIDE_PAD, Table, child, scrolled, stroke, with_footer};
use crate::ui::{Confirm, confirm_button, icon, inline_button, quiet_button, subheading};

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
    Delete(String),
    Save,
}

/// The profile picked on the page: an automatic one, or a support profile.
fn picked(settings: &Settings) -> Option<(Kind, String)> {
    match (&settings.auto, &settings.support) {
        (Some(auto), _) => Some((Kind::Auto, auto.id.clone())),
        (None, Some(support)) => Some((Kind::Support, support.id.clone())),
        (None, None) => None,
    }
}

/// What the header says of the profile open in the form: its name, its kind where it is
/// automatic, and what Reset to defaults would do.
pub(super) struct Open {
    pub name: String,
    pub kind: Option<String>,
    pub reset: Option<Reset>,
}

/// Whether Reset to defaults has anything to put back, and what it puts back.
pub(super) struct Reset {
    pub changed: bool,
    pub help: String,
}

/// The profile open in the form, while one is.
pub(super) fn editing(settings: &Settings) -> Option<Open> {
    if !settings.editing_support {
        return None;
    }
    match (&settings.auto, &settings.support) {
        (Some(auto), _) => Some(Open {
            name: auto.values.name.clone(),
            kind: Some(tr!("support-table-auto")),
            reset: auto::stock(auto).map(|stock| Reset {
                changed: stock != auto.values,
                help: tr!("support-reset-auto-help"),
            }),
        }),
        (None, Some(support)) => {
            let (stock, help) = form::stock(support);
            Some(Open {
                name: support.values.name.clone(),
                kind: None,
                reset: Some(Reset {
                    changed: stock != support.values,
                    help,
                }),
            })
        }
        (None, None) => None,
    }
}

/// Puts every number of the profile open back as Reset to defaults has them; undo takes it
/// back.
pub(super) fn reset(settings: &mut Settings) {
    match (&mut settings.auto, &mut settings.support) {
        (Some(auto), _) => {
            if let Some(stock) = auto::stock(auto) {
                auto.values = stock;
            }
        }
        (None, Some(support)) => support.values = form::stock(support).0,
        (None, None) => {}
    }
}

/// The band the tabs over the form stand in, and the tabs themselves.
const TABS_H: f32 = 52.0;
const TAB_ROW_H: f32 = 28.0;

/// The page, laid out in `content`: the form of the profile being edited under its tabs, or
/// the table.
pub(super) fn page(ui: &mut Ui, content: Rect, window: &mut Window) {
    let settings = &window.machine.settings;
    let picked = picked(settings);
    let editing = settings.editing_support && picked.is_some();
    let (mut body, mut bar) = with_footer(ui, content);
    let mut action = None;
    if editing {
        form_under_tabs(&mut body, window, picked.as_ref());
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

/// The open profile's form under a row of tabs naming its sections.
fn form_under_tabs(body: &mut Ui, window: &mut Window, picked: Option<&(Kind, String)>) {
    let auto = matches!(picked, Some((Kind::Auto, _)));
    let id = egui::Id::new(("support-tabs", picked.map(|(_, id)| id)));
    let tabs = match (auto, window.machine.settings.support.as_mut()) {
        (false, Some(draft)) => form::tabs(&mut draft.values),
        (false, None) => Vec::new(),
        (true, _) => auto::tabs(),
    };
    let whole = body.max_rect();
    let strip = whole.with_max_y(whole.top() + TABS_H);
    body.painter()
        .hline(strip.x_range(), strip.bottom(), stroke());
    let mut row = child(body, strip.shrink2(vec2(SIDE_PAD, 0.0)));
    row.add_space((TABS_H - TAB_ROW_H) / 2.0);
    jump::tabs(&mut row, id, &tabs, &tr!("support-tab-off"));
    let rest = whole.with_min_y(strip.bottom() + 1.0);
    scrolled(&mut child(body, rest), |ui| match auto {
        true => auto::form(ui, window, id),
        false => form::form(ui, window, id),
    });
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
    // Both take the profile as last saved, so unsaved edits would not go with it.
    let unsaved = machine.settings.unsaved();
    let copyable = support.is_some() && !unsaved;
    let why = |response: egui::Response| match unsaved {
        true => response.on_disabled_hover_text(tr!("support-save-first")),
        false => response,
    };
    let duplicate = quiet_button(
        ui,
        icon::DUPLICATE,
        &tr!("action-duplicate"),
        false,
        copyable,
    );
    if why(duplicate).clicked() {
        *action = support.clone().map(Action::Duplicate);
    }
    if why(quiet_button(
        ui,
        icon::EXPORT,
        &tr!("common-export"),
        false,
        copyable,
    ))
    .clicked()
    {
        *action = support.clone().map(Action::Export);
    }
    let catalogue = &machine.slicing.catalogue;
    let deletable = support
        .as_deref()
        .is_some_and(|id| can_delete(catalogue, Kind::Support, id));
    let tooltip = match (&support, deletable) {
        (Some(id), true) => cost(catalogue, &Deleting::Profile(Kind::Support, id.clone())),
        (Some(_), false) => tr!("profiles-shipped"),
        (None, _) => String::new(),
    };
    let delete = Confirm {
        glyph: icon::REMOVE,
        text: &tr!("profiles-delete"),
        asked: &tr!("confirm-asked-delete"),
        tooltip: &tooltip,
        enabled: deletable,
    };
    if confirm_button(ui, &delete) {
        *action = support.map(Action::Delete);
    }
    if editing {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if inline_button(ui, "", &tr!("settings-save"), true, unsaved).clicked() {
                *action = Some(Action::Save);
            }
        });
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
        Action::Delete(id) => forget_profile(machine, Kind::Support, &id),
        Action::Save => {
            save(machine);
        }
    }
}

/// Writes the profile open in the form, automatic or not. Answers whether it is written.
pub(super) fn save(machine: &mut Machine) -> bool {
    match machine.settings.auto {
        Some(_) => auto::save(machine),
        None => form::save(machine),
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
