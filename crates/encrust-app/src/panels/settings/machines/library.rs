//! The machine library: every profile as brand cards opening onto a model list. It takes the
//! form's place, not a menu, because a catalogue of 150 machines is a page.

use egui::Ui;
use printer_profiles::{Catalogue, Entry, PrinterProfile};

use crate::i18n::tr;
use crate::settings::Library;
use crate::ui::heading;
use crate::ui::{hint, icon, inline_button, list, list_row, summary_button, theme};

/// How many brand cards stand side by side.
const COLUMNS: usize = 4;

/// What the page was asked for, carried out by the caller.
pub enum Picked {
    /// A machine of the user's own, with nothing filled in.
    Custom,
    /// A machine taken in from a profile file.
    Import,
    /// A catalogue profile, by id.
    Printer(String),
}

/// The library over the form. Answers with what was picked, if anything was.
pub fn page(ui: &mut Ui, catalogue: &Catalogue, library: &mut Library) -> Option<Picked> {
    let mut picked = header(ui, library);
    ui.add_space(10.0);

    let needle = library.search.trim().to_lowercase();
    let entries = super::sorted(catalogue, &needle);
    if !needle.is_empty() {
        if entries.is_empty() {
            hint(ui, &tr!("machines-no-match"));
        }
        return picked.or_else(|| models(ui, &entries, true));
    }

    match library.brand.clone() {
        None => {
            if let Some(brand) = brands(ui, catalogue) {
                library.brand = Some(brand);
            }
        }
        Some(brand) => {
            let of_brand: Vec<&Entry<PrinterProfile>> = entries
                .into_iter()
                .filter(|entry| entry.profile.manufacturer == brand)
                .collect();
            picked = picked.or_else(|| models(ui, &of_brand, false));
        }
    }
    picked
}

/// The search over every machine, a profile file and the machine nobody shipped beside it,
/// and inside a brand the way back out of it.
fn header(ui: &mut Ui, library: &mut Library) -> Option<Picked> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if inline_button(ui, icon::ADD, &tr!("library-custom"), false, true).clicked() {
                picked = Some(Picked::Custom);
            }
            if inline_button(ui, icon::IMPORT, &tr!("library-import"), false, true).clicked() {
                picked = Some(Picked::Import);
            }
            ui.add(
                crate::ui::text_box(egui::TextEdit::singleline(&mut library.search))
                    .hint_text(format!("{}  {}", icon::FIND, tr!("library-search")))
                    .desired_width(ui.available_width()),
            );
        });
    });
    if let Some(brand) = library.brand.clone() {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if inline_button(ui, icon::BACK, &tr!("library-all-brands"), false, true).clicked() {
                library.brand = None;
            }
            ui.add_space(theme::ITEM_GAP);
            heading(ui, &brand, None);
        });
    }
    picked
}

/// Every manufacturer in the catalogue and how many machines it has, by name.
fn brand_counts(catalogue: &Catalogue) -> Vec<(String, usize)> {
    let mut counted: Vec<(String, usize)> = Vec::new();
    for entry in super::sorted(catalogue, "") {
        let brand = &entry.profile.manufacturer;
        match counted.iter_mut().find(|(name, _)| name == brand) {
            Some((_, count)) => *count += 1,
            None => counted.push((brand.clone(), 1)),
        }
    }
    counted.sort_by_key(|(name, _)| name.to_lowercase());
    counted
}

/// Every manufacturer in the catalogue, as cards. Answers with the one opened.
fn brands(ui: &mut Ui, catalogue: &Catalogue) -> Option<String> {
    let counted = brand_counts(catalogue);
    let mut opened = None;
    for row in counted.chunks(COLUMNS) {
        ui.columns(COLUMNS, |columns| {
            for (column, (brand, count)) in columns.iter_mut().zip(row) {
                let detail = tr!("library-machines", count = *count);
                let card = summary_button(
                    column,
                    icon::PRINTER,
                    theme::colors().text_mid,
                    brand,
                    &detail,
                );
                if card.clicked() {
                    opened = Some(brand.clone());
                }
            }
        });
        ui.add_space(8.0);
    }
    opened
}

/// The models of one brand, or whatever the search matched. Answers with the one picked.
fn models(ui: &mut Ui, entries: &[&Entry<PrinterProfile>], with_brand: bool) -> Option<Picked> {
    let mut picked = None;
    list(ui, |ui| {
        for entry in entries {
            let label = if with_brand {
                super::label_of(entry)
            } else {
                entry.profile.name.clone()
            };
            if list_row(ui, &label, None, false, None, None, None, None).picked {
                picked = Some(Picked::Printer(entry.id.clone()));
            }
        }
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cards_are_the_brands_by_name_with_what_each_holds() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue parses");
        let counted = brand_counts(&catalogue);
        let names: Vec<&str> = counted.iter().map(|(name, _)| name.as_str()).collect();
        let mut by_name = names.clone();
        by_name.sort_by_key(|name| name.to_lowercase());
        assert_eq!(names, by_name);
        assert_eq!(
            counted.iter().map(|(_, count)| count).sum::<usize>(),
            catalogue.printers().count(),
            "every machine is behind one card"
        );
    }
}
