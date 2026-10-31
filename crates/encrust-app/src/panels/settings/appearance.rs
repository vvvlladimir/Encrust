//! The Appearance page: light or dark, the language, and whether the window moves.

use egui::{Theme, Ui};

use crate::i18n::{self, Language, tr};
use crate::panels::settings::form_card;
use crate::prefs::Appearance;
use crate::state::Machine;
use crate::ui::{
    CardLook, DropdownFace, DropdownRow, describe, dropdown, dropdown_menu, dropdown_row, hint,
    switch, theme_card,
};

pub fn form(ui: &mut Ui, machine: &mut Machine) {
    let general = &mut machine.general;
    form_card(ui, &tr!("general-appearance"), |ui| {
        themes(ui, &mut general.appearance);
    });
    form_card(ui, &tr!("general-language"), |ui| {
        language(ui, &mut general.language);
    });
    form_card(ui, &tr!("general-motion"), |ui| {
        switch(
            ui,
            &mut general.reduce_motion,
            &tr!("general-reduce-motion"),
        );
    });
}

/// The three choices as pictures of the window, the system's naming what it is now.
fn themes(ui: &mut Ui, choice: &mut Appearance) {
    let now = match ui.ctx().system_theme() {
        Some(Theme::Light) => tr!("general-appearance-now-light"),
        Some(Theme::Dark) | None => tr!("general-appearance-now-dark"),
    };
    let cards = [
        (
            Appearance::System,
            CardLook::Both,
            tr!("general-appearance-system"),
            now,
        ),
        (
            Appearance::Light,
            CardLook::One(Theme::Light),
            tr!("general-appearance-light"),
            tr!("general-appearance-light-sub"),
        ),
        (
            Appearance::Dark,
            CardLook::One(Theme::Dark),
            tr!("general-appearance-dark"),
            tr!("general-appearance-dark-sub"),
        ),
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = CARD_GAP;
        for (value, look, name, sub) in &cards {
            if theme_card(ui, *look, name, sub, *choice == *value).clicked() {
                *choice = *value;
            }
        }
    });
    describe(ui, &tr!("general-appearance-hint"));
}

/// The language the window is written in. Each is named in itself, so a user who cannot
/// read the current one still finds theirs.
fn language(ui: &mut Ui, choice: &mut Option<Language>) {
    let system = tr!(
        "general-language-system",
        name = i18n::system().native_name()
    );
    let shown = choice.map_or_else(
        || system.clone(),
        |language| language.native_name().to_owned(),
    );
    let face = DropdownFace {
        lead: None,
        label: &shown,
        edited: false,
        count: None,
        empty: false,
    };
    let response = dropdown(ui, &face, ui.available_width());
    dropdown_menu(&response, |ui| {
        let offered = std::iter::once((None, system.as_str()))
            .chain(Language::ALL.map(|language| (Some(language), language.native_name())));
        for (language, name) in offered {
            if dropdown_row(ui, &DropdownRow::named(name, *choice == language)).clicked() {
                *choice = language;
                ui.close();
            }
        }
    });
    if choice.is_none()
        && let Some(tag) = i18n::system_untranslated()
    {
        hint(ui, &tr!("general-language-untranslated", tag = tag));
    }
}

/// Between two theme cards, points.
const CARD_GAP: f32 = 10.0;
