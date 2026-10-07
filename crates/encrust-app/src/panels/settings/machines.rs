//! The printers, each opening onto the resins set up on it: a printer's resins are its
//! own presets, and any resin ever added to a printer is in the pool for the others.

use egui::{Align, Color32, Layout, RichText, Sense, Ui, UiBuilder, vec2};
use printer_profiles::{Catalogue, Entry, Kind, PrinterProfile, ProfileError};

mod library;

use crate::panels::Window;
use crate::panels::settings::{printer, report, resin};
use crate::settings::{
    Deleting, Library, Node, add_resin, delete_resin, duplicate_resin, installed, new_printer,
    new_resin, pool_for, resins_of, take_resin_off,
};
use crate::state::Machine;
use crate::ui::{describe, icon, icon_button, secondary_button, theme};

const ROW_H: f32 = 32.0;
const INDENT: f32 = 18.0;

/// What a click in the list asked for, carried out once the list has been drawn.
enum Action {
    Pick(Node),
    NewPrinter,
    /// A machine taken out of the library, which is a copy of it in the user's directory.
    InstallPrinter(String),
    NewResin(String),
    AddResin {
        printer: String,
        resin: String,
    },
    DuplicateResin {
        printer: String,
        resin: String,
    },
    /// Something that cannot be undone, which the screen asks about before doing it.
    Ask(Deleting),
}

/// The printers, and under the one open, its resins and the way to add another.
///
/// What is listed is what the user installed, by brand, over a search line. The shipped
/// catalogue is the library behind the `+`; see `docs/decisions/0158`.
pub fn list(ui: &mut Ui, window: &mut Window) {
    ui.spacing_mut().item_spacing.y = 2.0;
    let mut action = None;
    if heading(ui, "Machines").clicked() {
        window.machine.settings.library = Some(Library::default());
    }
    search_line(ui, &mut window.machine.settings.search);

    let catalogue = &window.machine.slicing.catalogue;
    let node = window.machine.settings.node.clone();
    let open = node
        .as_ref()
        .map(Node::printer)
        .unwrap_or_default()
        .to_owned();
    let listed: Vec<String> = mine(catalogue, &window.machine.settings.search)
        .into_iter()
        .map(|entry| entry.id.clone())
        .collect();
    if listed.is_empty() {
        let empty = match window.machine.settings.search.trim().is_empty() {
            true => "No machine yet. Add one with the + above.",
            false => "No machine of that name.",
        };
        describe(ui, empty);
    }
    for id in listed {
        let Ok(entry) = catalogue.printer(&id) else {
            continue;
        };
        let picked = node == Some(Node::Printer(id.clone()));
        let label = label_of(entry);
        // Everything listed is the user's own copy, so every machine here comes off again.
        let actions = Actions {
            duplicate: None,
            delete: Some("Remove this machine"),
        };
        let row = row(ui, 0, Lead::Glyph(icon::PRINTER), &label, picked, &actions);
        if row.clicked {
            action = Some(Action::Pick(Node::Printer(id.clone())));
        }
        if row.delete {
            action = Some(Action::Ask(Deleting::Printer(id.clone())));
        }
        if id == open {
            resins(ui, window, &id, &node, &mut action);
        }
    }

    if let Some(action) = action {
        apply(window, action);
    }
}

/// What a machine is called in the list and matched against in the search.
fn label_of(entry: &Entry<PrinterProfile>) -> String {
    format!("{} {}", entry.profile.manufacturer, entry.profile.name)
}

/// Every machine the catalogue knows, in the order a list shows them: what the user has
/// first, then by brand, and only what `search` matches.
fn sorted<'a>(catalogue: &'a Catalogue, search: &str) -> Vec<&'a Entry<PrinterProfile>> {
    let needle = search.trim().to_lowercase();
    let mut entries: Vec<&Entry<PrinterProfile>> = catalogue
        .printers()
        .filter(|entry| needle.is_empty() || label_of(entry).to_lowercase().contains(&needle))
        .collect();
    entries.sort_by_key(|entry| {
        (
            !installed(&entry.source),
            entry.profile.manufacturer.to_lowercase(),
            entry.profile.name.to_lowercase(),
        )
    });
    entries
}

/// The same, narrowed to the machines the user installed, which is what the list shows.
fn mine<'a>(catalogue: &'a Catalogue, search: &str) -> Vec<&'a Entry<PrinterProfile>> {
    sorted(catalogue, search)
        .into_iter()
        .filter(|entry| installed(&entry.source))
        .collect()
}

/// The line that narrows the list, which is the only way through a long catalogue.
fn search_line(ui: &mut Ui, search: &mut String) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(search)
                .hint_text("Search machines")
                .desired_width(ui.available_width()),
        );
    });
    ui.add_space(6.0);
}

/// The resins of one printer, then the menu that adds one.
fn resins(
    ui: &mut Ui,
    window: &Window,
    printer: &str,
    node: &Option<Node>,
    action: &mut Option<Action>,
) {
    let catalogue = &window.machine.slicing.catalogue;
    let mut listed = false;
    for (id, resin) in resins_of(catalogue, printer) {
        let this = Node::Resin {
            printer: printer.to_owned(),
            resin: id.to_owned(),
        };
        let actions = Actions {
            duplicate: Some("Duplicate on this printer"),
            delete: Some("Take off this printer"),
        };
        let lead = Lead::Swatch(theme::resin_swatch(resin.details.color));
        let row = row(
            ui,
            1,
            lead,
            &resin.name,
            node.as_ref() == Some(&this),
            &actions,
        );
        let pair = || (printer.to_owned(), id.to_owned());
        if row.clicked {
            *action = Some(Action::Pick(this.clone()));
        }
        if row.duplicate {
            let (printer, resin) = pair();
            *action = Some(Action::DuplicateResin { printer, resin });
        }
        if row.delete {
            let (printer, resin) = pair();
            *action = Some(Action::Ask(Deleting::ResinOff { printer, resin }));
        }
        listed = true;
    }
    if !listed {
        first_resin(ui, printer, action);
    }
    add_menu(ui, window.machine, printer, action);
    ui.add_space(6.0);
}

/// What a printer with no resin on it offers. An exposure is measured on the machine in
/// the room, so the first resin is the user's to make (ADR 0196).
fn first_resin(ui: &mut Ui, printer: &str, action: &mut Option<Action>) {
    ui.horizontal(|ui| {
        ui.add_space(INDENT + 6.0);
        ui.vertical(|ui| {
            describe(
                ui,
                "No resin on this printer. An exposure is measured on your machine, not \
                 shipped, so the first one is yours to type in from a test print.",
            );
            if secondary_button(ui, icon::RESIN, "Add the first resin").clicked() {
                *action = Some(Action::NewResin(printer.to_owned()));
            }
        });
    });
    ui.add_space(4.0);
}

/// A new resin, or one out of the pool of every resin some printer has.
fn add_menu(ui: &mut Ui, machine: &Machine, printer: &str, action: &mut Option<Action>) {
    ui.horizontal(|ui| {
        ui.add_space(INDENT + 6.0);
        let label = RichText::new(format!("{}  Add resin", icon::ADD))
            .font(theme::label())
            .color(theme::colors().text_mid);
        ui.menu_button(label, |ui| {
            ui.set_min_width(220.0);
            if ui.button("New resin").clicked() {
                *action = Some(Action::NewResin(printer.to_owned()));
            }
            ui.separator();
            ui.label(
                RichText::new("From the pool")
                    .font(theme::small())
                    .color(theme::colors().text_low),
            );
            let mut empty = true;
            for (id, resin) in pool_for(&machine.slicing.catalogue, printer) {
                empty = false;
                ui.horizontal(|ui| {
                    if ui.button(&resin.name).clicked() {
                        *action = Some(Action::AddResin {
                            printer: printer.to_owned(),
                            resin: id.to_owned(),
                        });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, icon::REMOVE, "Delete this resin").clicked() {
                            *action = Some(Action::Ask(Deleting::Resin(id.to_owned())));
                        }
                    });
                });
            }
            if empty {
                describe(ui, "Every resin you have is already on this printer.");
            }
        });
    });
}

fn apply(window: &mut Window, action: Action) {
    let machine = &mut *window.machine;
    // Anything picked anywhere is what the form shows next, so the library gets out of
    // the way whether the pick came from it or from the list beside it.
    machine.settings.library = None;
    let catalogue = &mut machine.slicing.catalogue;
    let status = &mut machine.status;
    let picked = match action {
        Action::Pick(node) => Some(node),
        Action::NewPrinter => {
            report(status, "cannot add a printer", new_printer(catalogue)).map(Node::Printer)
        }
        Action::InstallPrinter(id) => {
            let outcome = install_printer(catalogue, &id);
            report(status, "cannot add the machine", outcome).map(|()| Node::Printer(id))
        }
        Action::NewResin(printer) => {
            let outcome = new_resin(catalogue, &printer);
            report(status, "cannot add a resin", outcome)
                .map(|resin| Node::Resin { printer, resin })
        }
        Action::AddResin { printer, resin } => {
            let outcome = add_resin(catalogue, &printer, &resin);
            report(status, "cannot add the resin", outcome).map(|()| Node::Resin { printer, resin })
        }
        Action::DuplicateResin { printer, resin } => {
            let outcome = duplicate_resin(catalogue, &printer, &resin);
            report(status, "cannot duplicate the resin", outcome)
                .map(|resin| Node::Resin { printer, resin })
        }
        Action::Ask(deleting) => {
            machine.settings.confirm = Some(deleting);
            None
        }
    };
    machine.slicing.reload_resin();
    pick(window, picked);
}

/// Carries out a deletion the question was answered for. Everything here writes the
/// user's directory and cannot be taken back; see `confirm`.
pub(super) fn delete(window: &mut Window, deleting: Deleting) {
    let picked = match deleting {
        Deleting::Printer(id) => {
            let machine = &mut *window.machine;
            // Where the list goes next is read while the machine is still in it.
            let next = neighbour(&machine.slicing.catalogue, &id);
            let outcome = machine
                .slicing
                .catalogue
                .forget_user_copy(Kind::Printer, &id);
            report(&mut machine.status, "cannot remove the machine", outcome);
            let in_hand = machine.slicing.printer_id.as_deref() == Some(id.as_str());
            if in_hand {
                stand_under(window, next.clone());
            }
            next.map(Node::Printer)
        }
        Deleting::ResinOff { printer, resin } => {
            let machine = &mut *window.machine;
            let outcome = take_resin_off(&mut machine.slicing.catalogue, &printer, &resin);
            report(&mut machine.status, "cannot take the resin off", outcome);
            Some(Node::Printer(printer))
        }
        Deleting::Resin(resin) => {
            let machine = &mut *window.machine;
            let outcome = delete_resin(&mut machine.slicing.catalogue, &resin);
            report(&mut machine.status, "cannot delete the resin", outcome);
            // The pool holds what this printer is not set up with, so what the form has
            // open is never the resin that just went.
            None
        }
    };
    window.machine.slicing.reload_resin();
    pick(window, picked);
}

/// Opens `node` in the form, and leaves the form as it is when nothing was picked.
fn pick(window: &mut Window, node: Option<Node>) {
    let machine = &mut *window.machine;
    if let Some(node) = node {
        machine.settings.pick(&machine.slicing.catalogue, node);
    }
}

/// Stands the plate under the machine the list moved to, or under none when the last one
/// has been removed: the window prints with the machine it has, not with one that is gone
/// (BUG-14).
fn stand_under(window: &mut Window, id: Option<String>) {
    let profile = id
        .as_deref()
        .and_then(|id| window.machine.slicing.catalogue.printer(id).ok())
        .map(|entry| entry.profile.clone());
    match profile {
        Some(profile) => crate::profiles::apply_printer(window, profile, id),
        None => crate::profiles::clear_printer(window),
    }
}

/// What the list moves to when `id` leaves it: the machine under it, or the one over it
/// when it was the last.
fn neighbour(catalogue: &Catalogue, id: &str) -> Option<String> {
    let listed = mine(catalogue, "");
    let at = listed.iter().position(|entry| entry.id == id)?;
    listed
        .get(at + 1)
        .or_else(|| at.checked_sub(1).and_then(|over| listed.get(over)))
        .map(|entry| entry.id.clone())
}

/// Takes a machine out of the library: a copy of the shipped profile under its own id,
/// in the user's directory, which is what makes it the user's to edit and to remove.
fn install_printer(catalogue: &mut Catalogue, id: &str) -> Result<(), ProfileError> {
    let entry = catalogue.printer(id)?;
    if installed(&entry.source) {
        return Ok(());
    }
    let profile = entry.profile.clone();
    catalogue.save_printer(id, &profile).map(drop)
}

/// The library while it is open, otherwise the form of what the list has picked.
pub fn form(ui: &mut Ui, window: &mut Window) {
    if window.machine.settings.library.is_some() {
        shop(ui, window);
        return;
    }
    match window.machine.settings.node {
        Some(Node::Printer(_)) => printer::form(ui, window),
        Some(Node::Resin { .. }) => resin::form(ui, window),
        None => describe(ui, "Pick a printer or one of its resins."),
    }
}

/// The library page, and what a pick in it does: either way the library closes, because
/// what was picked is what the user wants to look at next.
fn shop(ui: &mut Ui, window: &mut Window) {
    let machine = &mut *window.machine;
    let Some(library) = machine.settings.library.as_mut() else {
        return;
    };
    let Some(picked) = library::page(ui, &machine.slicing.catalogue, library) else {
        return;
    };
    machine.settings.library = None;
    match picked {
        library::Picked::Custom => apply(window, Action::NewPrinter),
        library::Picked::Printer(id) => apply(window, Action::InstallPrinter(id)),
    }
}

/// The small heading over the list. Answers with its one button, which opens the library.
fn heading(ui: &mut Ui, title: &str) -> egui::Response {
    let response = ui
        .horizontal(|ui| {
            ui.label(
                RichText::new(title)
                    .font(theme::section())
                    .color(theme::colors().text_low),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                icon_button(ui, icon::ADD, "Add a machine")
            })
            .inner
        })
        .inner;
    ui.add_space(4.0);
    response
}

/// What a row starts with: a glyph, or the colour of the resin it is.
enum Lead {
    Glyph(&'static str),
    Swatch(Color32),
}

/// The buttons a row offers on hover, by their tooltips.
struct Actions {
    duplicate: Option<&'static str>,
    delete: Option<&'static str>,
}

#[derive(Default)]
struct Clicked {
    clicked: bool,
    duplicate: bool,
    delete: bool,
}

/// One row of the tree. It takes its click before its buttons are laid over it, so a
/// button wins the press it sits under.
fn row(
    ui: &mut Ui,
    depth: u8,
    lead: Lead,
    label: &str,
    picked: bool,
    actions: &Actions,
) -> Clicked {
    let colors = theme::colors();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let hovered = ui.rect_contains_pointer(rect);
    let (fill, text) = match (picked, hovered) {
        (true, _) => (colors.accent_wash, colors.accent),
        (false, true) => (colors.hover, colors.text_high),
        (false, false) => (Color32::TRANSPARENT, colors.text_high),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::R_CONTROL, fill);
    let left = rect.left_center() + vec2(10.0 + INDENT * f32::from(depth), 0.0);
    match lead {
        Lead::Glyph(glyph) => {
            painter.text(
                left,
                egui::Align2::LEFT_CENTER,
                glyph,
                theme::icon(15.0),
                text,
            );
        }
        Lead::Swatch(colour) => {
            let chip = egui::Rect::from_center_size(left + vec2(6.0, 0.0), vec2(10.0, 10.0));
            painter.rect_filled(chip, 3.0, colour);
        }
    }
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(left.x + 24.0, rect.top()),
        egui::pos2(rect.right() - 64.0, rect.bottom()),
    );
    let galley = painter.layout(label.to_owned(), theme::label(), text, label_rect.width());
    let at = egui::pos2(label_rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter_at(label_rect).galley(at, galley, text);

    let mut clicked = Clicked {
        clicked: response.clicked(),
        ..Clicked::default()
    };
    if hovered || picked {
        let buttons = egui::Rect::from_min_max(rect.center_top(), rect.right_bottom());
        let layout = Layout::right_to_left(Align::Center);
        // A child of its own rather than a scope: a scope allocates what it used in the
        // list, and a row with no buttons would pull the next row up over this one.
        let mut ui = ui.new_child(UiBuilder::new().max_rect(buttons).layout(layout));
        ui.add_space(2.0);
        if let Some(tooltip) = actions.delete {
            clicked.delete = icon_button(&mut ui, icon::REMOVE, tooltip).clicked();
        }
        if let Some(tooltip) = actions.duplicate {
            clicked.duplicate = icon_button(&mut ui, icon::DUPLICATE, tooltip).clicked();
        }
    }
    clicked.clicked &= !clicked.delete && !clicked.duplicate;
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::new_printer;

    /// The shipped catalogue over a directory of its own, which every write lands in.
    fn writable(name: &str) -> Catalogue {
        let dir =
            std::env::temp_dir().join(format!("encrust-machines-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Catalogue::with_root(&dir).expect("a missing directory reads as empty")
    }

    #[test]
    fn nothing_is_installed_until_a_machine_is_taken_out_of_the_library() {
        let mut catalogue = writable("install");
        assert!(
            mine(&catalogue, "").is_empty(),
            "a first run has no machine of its own"
        );
        assert!(
            !sorted(&catalogue, "").is_empty(),
            "the library still offers every shipped machine"
        );

        install_printer(&mut catalogue, "elegoo-mars-4-ultra").expect("the directory is writable");
        let listed: Vec<&str> = mine(&catalogue, "")
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(listed, ["elegoo-mars-4-ultra"]);

        catalogue
            .forget_user_copy(Kind::Printer, "elegoo-mars-4-ultra")
            .expect("the copy is the user's to remove");
        assert!(
            mine(&catalogue, "").is_empty(),
            "removing the copy puts the machine back in the library"
        );
    }

    /// BUG-14: the window prints with the machine the list moved to, so where the list
    /// goes has to be an answer and never the machine that was just removed.
    #[test]
    fn removing_a_machine_moves_the_list_to_the_one_under_it() {
        let mut catalogue = writable("neighbour");
        for id in [
            "elegoo-mars-3-pro",
            "elegoo-mars-4-ultra",
            "elegoo-saturn-4-ultra",
        ] {
            install_printer(&mut catalogue, id).expect("the directory is writable");
        }
        let listed: Vec<String> = mine(&catalogue, "")
            .iter()
            .map(|entry| entry.id.clone())
            .collect();

        assert_eq!(
            neighbour(&catalogue, &listed[0]).as_deref(),
            Some(listed[1].as_str()),
            "the machine under it"
        );
        assert_eq!(
            neighbour(&catalogue, &listed[2]).as_deref(),
            Some(listed[1].as_str()),
            "and the one over it when it was the last"
        );
        assert!(neighbour(&catalogue, "no-such-machine").is_none());

        for id in &listed {
            catalogue
                .forget_user_copy(Kind::Printer, id)
                .expect("the copy is the user's to remove");
        }
        assert!(
            neighbour(&catalogue, &listed[0]).is_none(),
            "nothing installed leaves the window with no machine at all"
        );
    }

    #[test]
    fn a_machine_of_your_own_stands_over_the_shipped_ones() {
        let mut catalogue = writable("own");
        let id = new_printer(&mut catalogue).expect("the directory is writable");
        let listed = sorted(&catalogue, "");
        assert_eq!(listed.first().map(|entry| entry.id.as_str()), Some(&*id));
        let brands: Vec<String> = listed[1..]
            .iter()
            .map(|entry| entry.profile.manufacturer.to_lowercase())
            .collect();
        let mut sorted_brands = brands.clone();
        sorted_brands.sort_unstable();
        assert_eq!(brands, sorted_brands, "the rest are by brand");
    }

    #[test]
    fn the_search_matches_the_brand_as_well_as_the_model() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue parses");
        let elegoo = sorted(&catalogue, "ELEGOO");
        assert!(
            elegoo
                .iter()
                .all(|entry| entry.profile.manufacturer == "Elegoo"),
            "a brand matches whatever it is typed in"
        );
        assert_eq!(sorted(&catalogue, "mars 4 ultra").len(), 1);
        assert!(sorted(&catalogue, "no such machine").is_empty());
    }
}
