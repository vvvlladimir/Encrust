//! The machine library: every profile there is, as brand cards that open onto a model
//! list. It stands in the form's place rather than hanging off the `+` as a menu, because
//! a catalogue of a hundred and fifty machines is a page, not a dropdown.

use egui::{RichText, Ui};
use printer_profiles::{Catalogue, Entry, PrinterProfile};

use crate::settings::Library;
use crate::ui::{hint, icon, inline_button, list, list_row, summary_button, theme};

/// How many brand cards stand side by side.
const COLUMNS: usize = 3;

/// What the page was asked for, carried out by the caller.
pub enum Picked {
    /// A machine of the user's own, with nothing filled in.
    Custom,
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
            hint(ui, "No machine of that name.");
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

/// The title, the way back out of a brand, and the machine nobody shipped.
fn header(ui: &mut Ui, library: &mut Library) -> Option<Picked> {
    let mut picked = None;
    ui.horizontal(|ui| {
        let title = library
            .brand
            .clone()
            .unwrap_or_else(|| "Add a machine".to_owned());
        if library.brand.is_some() {
            if inline_button(ui, icon::BACK, "All brands", false, true).clicked() {
                library.brand = None;
            }
            ui.add_space(8.0);
        }
        ui.label(
            RichText::new(title)
                .font(theme::section())
                .color(theme::colors().text_mid),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if inline_button(ui, icon::ADD, "Custom printer", false, true).clicked() {
                picked = Some(Picked::Custom);
            }
        });
    });
    ui.add_space(8.0);
    ui.add(
        egui::TextEdit::singleline(&mut library.search)
            .hint_text("Search every machine")
            .desired_width(ui.available_width()),
    );
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
                let detail = match count {
                    1 => "1 machine".to_owned(),
                    many => format!("{many} machines"),
                };
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
            if list_row(ui, &label, None, false, None, None).picked {
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
