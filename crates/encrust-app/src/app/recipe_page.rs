//! Settings' Recipes page driven through the window: a deletion that asks first, and a
//! condition that does not read marked under its field with Save held off.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::SlicerApp;
use super::states::{SETTLE_FRAMES, app};
use crate::settings::Section;

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// The window on the Recipes page, a recipe of the user's, `mine`, picked in the table.
fn on_the_recipes_page() -> Harness<'static, SlicerApp> {
    let mut app = app(false);
    app.view.options.started = true;
    let text = core_recipe::BUILT_IN[0]
        .1
        .replace("Prepare for print", "Mine");
    let catalogue = &mut app.machine.slicing.catalogue;
    catalogue
        .save_recipe("mine", &text)
        .expect("the test's directory is writable");
    let settings = &mut app.machine.settings;
    settings.open = true;
    settings.turn_to(Section::Recipes);
    settings.recipe_pick = Some("mine".to_owned());
    let mut dressed = false;
    let mut harness = Harness::builder().with_size(SIZE).build_ui_state(
        move |ui, app: &mut SlicerApp| {
            if dressed {
                app.window().show(ui);
            } else {
                crate::ui::theme::apply(ui.ctx());
                dressed = true;
            }
        },
        app,
    );
    harness.run_steps(SETTLE_FRAMES);
    harness
}

fn kept(harness: &Harness<'_, SlicerApp>) -> bool {
    harness
        .state()
        .machine
        .slicing
        .catalogue
        .recipe("mine")
        .is_ok()
}

#[test]
fn deleting_a_recipe_of_yours_asks_by_a_second_press() {
    let mut harness = on_the_recipes_page();
    harness.get_by_label("Delete").click();
    harness.run_steps(4);
    assert!(kept(&harness), "the first press only asks");
    harness.get_by_label("Delete?").click();
    harness.run_steps(4);
    assert!(!kept(&harness), "the second press deletes it");
}

#[test]
fn a_when_that_does_not_read_is_marked_under_its_field_and_save_stays_off() {
    let mut harness = on_the_recipes_page();
    let entry = core_recipe::find("mine", [("mine", core_recipe::BUILT_IN[0].1)]);
    let app = harness.state_mut();
    app.machine.settings.open_recipe(&entry.expect("kept"));
    let draft = app.machine.settings.recipe.as_mut().expect("open");
    draft.values.steps[2].when = "saves_ml >".to_owned();
    harness.run_steps(SETTLE_FRAMES);
    harness.get_by_label_contains("expected at character");
    let text = |harness: &Harness<'_, SlicerApp>| {
        let catalogue = &harness.state().machine.slicing.catalogue;
        catalogue
            .recipe("mine")
            .map(|entry| entry.profile.clone())
            .ok()
    };
    let before = text(&harness);
    harness.get_by_label("Save").click();
    harness.run_steps(4);
    assert_eq!(text(&harness), before, "an invalid recipe is not saved");
}
