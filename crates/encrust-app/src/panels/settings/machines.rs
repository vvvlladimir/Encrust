//! The printers, each opening onto the resins set up on it: a printer's resins are its
//! own presets, and any resin ever added to a printer is in the pool for the others.

use egui::{Align, Color32, Layout, RichText, Sense, Ui, UiBuilder, vec2};
use printer_profiles::{Catalogue, Entry, Kind, PrinterProfile, ProfileError};

mod library;

use crate::panels::Window;
use crate::panels::settings::{printer, report, resin};
use crate::settings::{
    Library, Node, add_resin, duplicate_resin, installed, installed_printers, new_printer,
    new_resin, pool_for, remove_resin, resins_of,
};
use crate::state::Machine;
use crate::ui::{describe, icon, icon_button, theme};

const ROW_H: f32 = 32.0;
const INDENT: f32 = 18.0;

/// What a click in the list asked for, carried out once the list has been drawn.
enum Action {
    Pick(Node),
    NewPrinter,
    /// A machine taken out of the library, which is a copy of it in the user's directory.
    InstallPrinter(String),
    DeletePrinter(String),
    NewResin(String),
    AddResin {
        printer: String,
        resin: String,
    },
    DuplicateResin {
        printer: String,
        resin: String,
    },
    RemoveResin {
        printer: String,
        resin: String,
    },
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
            action = Some(Action::DeletePrinter(id.clone()));
        }
        if id == open {
            resins(ui, window, &id, &node, &mut action);
        }
    }

    if let Some(action) = action {
        apply(window.machine, action);
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
            *action = Some(Action::RemoveResin { printer, resin });
        }
    }
    add_menu(ui, window.machine, printer, action);
    ui.add_space(6.0);
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
                if ui.button(&resin.name).clicked() {
                    *action = Some(Action::AddResin {
                        printer: printer.to_owned(),
                        resin: id.to_owned(),
                    });
                }
            }
            if empty {
                describe(ui, "Every resin there is is already on this printer.");
            }
        });
    });
}

fn apply(machine: &mut Machine, action: Action) {
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
        Action::DeletePrinter(id) => {
            let outcome = catalogue.forget_user_copy(Kind::Printer, &id);
            report(status, "cannot remove the machine", outcome);
            installed_printers(catalogue)
                .next()
                .map(|entry| Node::Printer(entry.id.clone()))
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
        Action::RemoveResin { printer, resin } => {
            let outcome = remove_resin(catalogue, &printer, &resin);
            report(status, "cannot take the resin off", outcome);
            Some(Node::Printer(printer))
        }
    };
    machine.slicing.reload_resin();
    if let Some(node) = picked {
        machine.settings.pick(&machine.slicing.catalogue, node);
    }
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
        shop(ui, window.machine);
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
fn shop(ui: &mut Ui, machine: &mut Machine) {
    let Some(library) = machine.settings.library.as_mut() else {
        return;
    };
    let Some(picked) = library::page(ui, &machine.slicing.catalogue, library) else {
        return;
    };
    machine.settings.library = None;
    match picked {
        library::Picked::Custom => apply(machine, Action::NewPrinter),
        library::Picked::Printer(id) => apply(machine, Action::InstallPrinter(id)),
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
