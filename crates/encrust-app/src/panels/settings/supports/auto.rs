//! An automatic profile's form: where its supports go, and the sizes, steps and slopes it
//! builds them to. See `docs/decisions/0325` and `0326`.

use egui::Ui;
use printer_profiles::{AUTO_SIZES, AutoProfile, Catalogue, SupportProfile};

use crate::i18n::tr;
use crate::panels::Window;
use crate::panels::settings::jump::{self, Tab};
use crate::panels::settings::{form_card, report};
use crate::panels::support_fields::{held_up, islands, sizes, steadying, steps};
use crate::settings::Draft;
use crate::ui::{row_name, title_line};

/// The cards of the form, one under another.
#[derive(Clone, Copy)]
enum Card {
    Placing,
    Sizes,
    Steps,
    Steadying,
}

const ORDER: [Card; 4] = [Card::Placing, Card::Sizes, Card::Steps, Card::Steadying];

/// The tabs over the form: a card each.
pub(super) fn tabs() -> Vec<Tab> {
    ORDER
        .map(|card| Tab {
            name: card.title(),
            off: false,
        })
        .into()
}

impl Card {
    fn title(self) -> String {
        match self {
            Self::Placing => tr!("supports-placing"),
            Self::Sizes => tr!("supports-sizes"),
            Self::Steps => tr!("support-group-steps"),
            Self::Steadying => tr!("support-group-steadying"),
        }
    }

    fn show(
        self,
        ui: &mut Ui,
        auto: &mut AutoProfile,
        saved: Option<&AutoProfile>,
        shapes: &[String; AUTO_SIZES],
    ) {
        match self {
            Self::Placing => {
                placing(ui, auto, saved);
                islands(ui, auto, saved);
            }
            Self::Sizes => sizes(ui, auto, saved, shapes),
            Self::Steps => steps(ui, auto, saved, shapes),
            Self::Steadying => steadying(ui, auto, saved),
        }
    }
}

/// The automatic profile open in the form, kept until it is saved. Its name is the
/// shipped one's, so it is shown rather than typed.
pub(super) fn form(ui: &mut Ui, window: &mut Window, tabs: egui::Id) {
    let catalogue = &window.machine.slicing.catalogue;
    let Some(draft) = window.machine.settings.auto.as_mut() else {
        return;
    };
    let saved = draft.as_saved().clone();
    let shapes = shape_names(catalogue, &draft.values);
    let auto = &mut draft.values;

    form_card(ui, &tr!("table-profile"), |ui| {
        ui.horizontal(|ui| {
            row_name(ui, &tr!("settings-name"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                title_line(ui, &auto.name);
            });
        });
    });

    let saved = Some(&saved);
    for (index, card) in ORDER.into_iter().enumerate() {
        jump::section(ui, tabs, index, ui.cursor().top());
        form_card(ui, &card.title(), |ui| card.show(ui, auto, saved, &shapes));
    }
}

/// What Reset to defaults puts back in `draft`: the shipped numbers under its own name, or
/// nothing for a profile this build does not ship.
pub(super) fn stock(draft: &Draft<AutoProfile>) -> Option<AutoProfile> {
    let shipped = draft.as_shipped()?;
    Some(AutoProfile {
        name: draft.values.name.clone(),
        ..shipped.clone()
    })
}

/// Where the profile puts supports, read through the fields a support profile has for it.
fn placing(ui: &mut Ui, auto: &mut AutoProfile, saved: Option<&AutoProfile>) {
    let stock = SupportProfile::default();
    let saved = saved.map(|saved| saved.placing_with(&stock));
    let mut placing = auto.placing_with(&stock);
    held_up(ui, &mut placing, saved.as_ref());
    auto.take_placing(&placing);
}

/// The names of the support profiles `auto` sizes into, lightest first, or their ids where
/// the catalogue has none.
fn shape_names(catalogue: &Catalogue, auto: &AutoProfile) -> [String; AUTO_SIZES] {
    auto.shapes.each_ref().map(|id| {
        catalogue
            .support(id)
            .map_or_else(|_| id.clone(), |entry| entry.profile.name.clone())
    })
}

/// Writes the profile open in the form when unsaved, a plate still on the copy it replaces
/// following. Answers whether it is written, the status saying why where it is not.
pub(super) fn save(window: &mut Window) -> bool {
    let machine = &mut *window.machine;
    let Some(draft) = machine.settings.auto.as_mut() else {
        return false;
    };
    if !draft.is_dirty() {
        return true;
    }
    let catalogue = &mut machine.slicing.catalogue;
    let was = catalogue
        .autos()
        .find(|entry| entry.id == draft.id)
        .map(|entry| entry.profile.clone());
    let outcome = catalogue.save_auto(&draft.id, &draft.values);
    let saved = report(&mut machine.status, &tr!("support-cannot-save"), outcome).is_some();
    if saved {
        if let Some(was) = was {
            window.tools.supports.follow_saved_auto(&was, &draft.values);
        }
        draft.mark_saved();
    }
    saved
}

#[cfg(test)]
mod tests {
    use crate::panels::fixtures::Parts;
    use crate::panels::settings::supports::save;
    use crate::settings::fixtures::writable;
    use crate::state::Doc;
    use crate::workspace::Tool;

    fn sized_by_auto(name: &str) -> Parts {
        let mut parts = Parts::of(Doc::default(), Tool::Supports);
        parts.machine.slicing.catalogue = writable(name);
        let catalogue = &parts.machine.slicing.catalogue;
        parts.tools.supports.set_automatic(true, catalogue);
        parts.machine.settings.pick_auto(catalogue, "default");
        parts
    }

    fn save_with_density(parts: &mut Parts, density: f32) {
        let draft = parts.machine.settings.auto.as_mut().expect("Auto is open");
        draft.values.density = density;
        assert!(save(&mut parts.window()), "the profile is written");
    }

    #[test]
    fn a_plate_left_on_auto_as_saved_takes_up_what_settings_saves() {
        let mut parts = sized_by_auto("auto-follows");
        save_with_density(&mut parts, 2.0);
        let plate = parts.tools.supports.auto.as_ref().expect("sizing stays on");
        assert_eq!(plate.density, 2.0, "the plate follows the saved Auto");
    }

    #[test]
    fn a_plate_with_numbers_of_its_own_keeps_them_when_settings_saves() {
        let mut parts = sized_by_auto("auto-keeps");
        let plate = parts.tools.supports.auto.as_mut().expect("sizing is on");
        plate.density = 1.5;
        save_with_density(&mut parts, 2.0);
        let plate = parts.tools.supports.auto.as_ref().expect("sizing stays on");
        assert_eq!(plate.density, 1.5, "the plate's own density 1.5 is kept");
    }
}
