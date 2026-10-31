use std::path::Path;

use printer_profiles::{Catalogue, Kind, Preset};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::i18n::tr;
use crate::settings::unique_id;
use crate::status::Status;
use crate::ui::{
    BinLook, DropdownFace, DropdownRow, Lead, bin_button, dropdown, dropdown_menu, dropdown_row,
    icon, quiet_square, theme,
};

/// The named settings of one tool, as the Supports card has them: a picker showing the
/// preset the panel matches, or the one it came from and `· edited`; Save, over that one or
/// as a new one; and a bin that asks before it throws the one in use away. See ADR 0226.
pub fn bar<T>(
    ui: &mut egui::Ui,
    catalogue: &mut Catalogue,
    status: &mut Status,
    kind: Kind,
    current: &mut T,
) where
    T: Serialize + DeserializeOwned + PartialEq,
{
    let read = |entry: &printer_profiles::Entry<Preset>| {
        entry.profile.read::<T>(Path::new(&entry.id)).ok()
    };
    let source_key = egui::Id::new(("preset-source", kind.dir()));
    let (matching, source) = find_source(ui, catalogue, kind, current, source_key);
    let edited = matching.is_none() && source.is_some();
    let shown = match (&source, catalogue.presets(kind).next()) {
        (Some((_, name)), _) => name.clone(),
        (None, Some(_)) => tr!("presets-unsaved"),
        (None, None) => tr!("presets-none-saved"),
    };

    let named = Named {
        matching: matching.as_ref(),
        source: source.as_ref(),
        shown: &shown,
        edited,
    };
    let Pressed {
        picked,
        over,
        as_new,
        forget,
    } = row(ui, catalogue, kind, &named, read);
    if let Some((id, settings)) = picked {
        *current = settings;
        ui.data_mut(|data| data.insert_temp(source_key, id));
    }
    if forget && let Some((id, _)) = &matching {
        let gone = catalogue
            .forget_user_copy(kind, id)
            .map_err(anyhow::Error::new);
        status.report(&tr!("presets-deleted", kind = kind.dir()), gone);
    }
    let saved = save(catalogue, kind, (over, as_new), source.as_ref(), current);
    if let Some(saved) = saved {
        let name = saved
            .as_ref()
            .map_or_else(|_| String::new(), |(_, name)| name.clone());
        if let Some((id, _)) = status.report(&tr!("status-saved", name = name), saved) {
            ui.data_mut(|data| data.insert_temp(source_key, id));
        }
    }
}

/// A preset as (id, name).
type Shown = Option<(String, String)>;

/// The preset the panel matches and the one it came from.
fn find_source<T>(
    ui: &egui::Ui,
    catalogue: &Catalogue,
    kind: Kind,
    current: &T,
    source_key: egui::Id,
) -> (Shown, Shown)
where
    T: DeserializeOwned + PartialEq,
{
    let matching = catalogue
        .presets(kind)
        .find(|entry| {
            let read = entry.profile.read::<T>(Path::new(&entry.id)).ok();
            read.as_ref() == Some(current)
        })
        .map(|entry| (entry.id.clone(), entry.profile.name.clone()));
    let source = matching.clone().or_else(|| {
        let id = ui.data(|data| data.get_temp::<String>(source_key))?;
        let entry = catalogue.presets(kind).find(|entry| entry.id == id)?;
        Some((entry.id.clone(), entry.profile.name.clone()))
    });
    (matching, source)
}

/// The preset a panel shows: the one it matches, the one it came from, and how it reads.
struct Named<'a> {
    matching: Option<&'a (String, String)>,
    source: Option<&'a (String, String)>,
    shown: &'a str,
    edited: bool,
}

/// What a frame of the row was pressed for.
struct Pressed<T> {
    picked: Option<(String, T)>,
    over: bool,
    as_new: bool,
    forget: bool,
}

/// The picker, Save and the bin, laid right to left.
fn row<T>(
    ui: &mut egui::Ui,
    catalogue: &Catalogue,
    kind: Kind,
    named: &Named<'_>,
    read: impl Fn(&printer_profiles::Entry<Preset>) -> Option<T>,
) -> Pressed<T> {
    let mut pressed = Pressed {
        picked: None,
        over: false,
        as_new: false,
        forget: false,
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::BUTTON_GAP;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let tip = named.matching.map_or_else(String::new, |(_, name)| {
                tr!("presets-delete-named", name = name.clone())
            });
            let held = named.matching.is_some();
            pressed.forget = bin_button(ui, theme::BUTTON_H, BinLook::Quiet, &tip, held);
            let save_tip = tr!("shape-save-tooltip");
            let save = quiet_square(ui, icon::SAVE, &save_tip, theme::BUTTON_H, !held);
            egui::Popup::menu(&save).show(|ui| {
                let over = egui::Button::new(tr!("shape-save"));
                pressed.over = ui.add_enabled(named.edited, over).clicked();
                pressed.as_new = ui.button(tr!("shape-save-as-new")).clicked();
            });
            let face = DropdownFace {
                lead: Some(Lead::Glyph(icon::PRESET)),
                label: named.shown,
                edited: named.edited,
                count: None,
                empty: catalogue.presets(kind).next().is_none(),
            };
            let response = dropdown(ui, &face, ui.available_width());
            dropdown_menu(&response, |ui| {
                for entry in catalogue.presets(kind) {
                    let on = named.source.is_some_and(|(id, _)| *id == entry.id);
                    let row = DropdownRow::named(&entry.profile.name, on);
                    if dropdown_row(ui, &row).clicked() {
                        pressed.picked = read(entry).map(|settings| (entry.id.clone(), settings));
                        ui.close();
                    }
                }
            });
        });
    });
    pressed
}

/// Keeps `current` over the preset it came from, or as a new one named after it.
fn save<T: Serialize>(
    catalogue: &mut Catalogue,
    kind: Kind,
    (over, as_new): (bool, bool),
    source: Option<&(String, String)>,
    current: &T,
) -> Option<anyhow::Result<(String, String)>> {
    match (over, as_new, source) {
        (true, _, Some((id, name))) => Some(overwrite(catalogue, kind, id, name, current)),
        (_, true, _) => {
            let base = source.map_or_else(|| tr!("presets-new-name"), |(_, name)| name.clone());
            let name = free_name(catalogue, kind, &base);
            Some(keep(catalogue, kind, &name, current).map(|id| (id, name)))
        }
        _ => None,
    }
}

/// Keeps `settings` over the preset `id`, under its own `name`.
fn overwrite<T: Serialize>(
    catalogue: &mut Catalogue,
    kind: Kind,
    id: &str,
    name: &str,
    settings: &T,
) -> anyhow::Result<(String, String)> {
    let preset = Preset::of(name, settings, Path::new(id))?;
    catalogue.save_preset(kind, id, &preset)?;
    Ok((id.to_owned(), name.to_owned()))
}

/// `base`, or `base 2`, `base 3` and on: the first no preset of `kind` is named yet.
fn free_name(catalogue: &Catalogue, kind: Kind, base: &str) -> String {
    let taken = |name: &str| {
        catalogue
            .presets(kind)
            .any(|entry| entry.profile.name == name)
    };
    // A catalogue holds a few dozen presets at most; the bound only guards the loop.
    (1..=1000)
        .map(|n| match n {
            1 => base.to_owned(),
            n => format!("{base} {n}"),
        })
        .find(|name| !taken(name))
        .unwrap_or_else(|| base.to_owned())
}

/// Keeps `settings` under `name` as a new preset of `kind`, and answers its id.
pub fn keep<T: Serialize>(
    catalogue: &mut Catalogue,
    kind: Kind,
    name: &str,
    settings: &T,
) -> anyhow::Result<String> {
    let id = unique_id(name, |id| catalogue.is_taken(kind, id));
    let preset = Preset::of(name, settings, Path::new(&id))?;
    catalogue.save_preset(kind, &id, &preset)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_engine::project::DrainState;

    #[test]
    fn a_drain_preset_comes_back_as_the_numbers_it_was_saved_with() {
        let dir = std::env::temp_dir().join("encrust-app-drain-preset");
        let _ = std::fs::remove_dir_all(&dir);
        let mut catalogue = Catalogue::with_root(&dir).expect("the shipped catalogue is valid");
        let state = DrainState::round(3.5, 2.0, 0.8);
        let id = keep(&mut catalogue, Kind::Drain, "Standard 3.5 mm", &state).expect("writable");
        assert_eq!(id, "standard-3-5-mm");
        let again = keep(&mut catalogue, Kind::Drain, "Standard 3.5 mm", &state).expect("writable");
        assert_eq!(
            again, "standard-3-5-mm-2",
            "a second of one name is numbered"
        );

        let entry = catalogue
            .presets(Kind::Drain)
            .find(|entry| entry.id == id)
            .expect("kept");
        let read: DrainState = entry.profile.read(Path::new(&id)).expect("same shape");
        assert_eq!(read, state);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
