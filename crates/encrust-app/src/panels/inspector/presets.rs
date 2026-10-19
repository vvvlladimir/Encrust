use std::path::Path;

use printer_profiles::{Catalogue, Kind, Preset};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::i18n::tr;
use crate::settings::unique_id;
use crate::status::Status;
use crate::ui::{field_label, icon, icon_button, inline_button, theme};

/// The named settings of one tool: which one the panel matches, the way to load another,
/// to keep the panel as a new one, and to throw the one in use away. See ADR 0226.
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
    let matching = catalogue
        .presets(kind)
        .find(|entry| read(entry).as_ref() == Some(current))
        .map(|entry| (entry.id.clone(), entry.profile.name.clone()));
    let shown = match &matching {
        Some((_, name)) => name.clone(),
        None if catalogue.presets(kind).next().is_some() => tr!("presets-unsaved"),
        None => tr!("presets-none-saved"),
    };

    let mut picked = None;
    let mut forget = None;
    ui.horizontal(|ui| {
        field_label(ui, &tr!("table-profile"), theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some((id, name)) = &matching
                && icon_button(
                    ui,
                    icon::REMOVE,
                    &tr!("presets-delete-named", name = name.clone()),
                )
                .clicked()
            {
                forget = Some(id.clone());
            }
            egui::ComboBox::from_id_salt(("tool-preset", kind.dir()))
                .width(ui.available_width())
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for entry in catalogue.presets(kind) {
                        let on = matching.as_ref().is_some_and(|(id, _)| *id == entry.id);
                        if ui.selectable_label(on, &entry.profile.name).clicked() {
                            picked = read(entry);
                        }
                    }
                });
        });
    });
    if let Some(settings) = picked {
        *current = settings;
    }
    if let Some(id) = forget {
        let gone = catalogue
            .forget_user_copy(kind, &id)
            .map_err(anyhow::Error::new);
        status.report(&tr!("presets-deleted", kind = kind.dir()), gone);
    }
    if matching.is_none() {
        save_row(ui, catalogue, status, kind, current);
    }
}

/// A name and a press that keeps the panel's settings under it.
fn save_row<T: Serialize>(
    ui: &mut egui::Ui,
    catalogue: &mut Catalogue,
    status: &mut Status,
    kind: Kind,
    current: &T,
) {
    let id = ui.id().with(("preset-name", kind.dir()));
    let mut name = ui.data(|data| data.get_temp::<String>(id).unwrap_or_default());
    let mut save = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let ready = !name.trim().is_empty();
            save = ui
                .add_enabled_ui(ready, |ui| {
                    inline_button(ui, icon::SAVE, &tr!("shape-save"), false, true)
                })
                .inner
                .clicked();
            let field = crate::ui::text_box(egui::TextEdit::singleline(&mut name))
                .hint_text(tr!("presets-name-hint"))
                .font(theme::label())
                .desired_width(ui.available_width());
            ui.add(field);
        });
    });
    if save {
        let kept = keep(catalogue, kind, name.trim(), current);
        if status
            .report(&tr!("status-saved", name = name.trim().to_owned()), kept)
            .is_some()
        {
            name.clear();
        }
    }
    ui.data_mut(|data| data.insert_temp(id, name));
}

/// Keeps `settings` under `name` as a new preset of `kind`, and answers its id.
pub fn keep<T: Serialize>(
    catalogue: &mut Catalogue,
    kind: Kind,
    name: &str,
    settings: &T,
) -> anyhow::Result<String> {
    let id = unique_id(name, |id| {
        catalogue.presets(kind).any(|entry| entry.id == id)
    });
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
