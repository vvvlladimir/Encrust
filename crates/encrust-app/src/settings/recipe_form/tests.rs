use core_recipe::{Recipe, built_in};
use printer_profiles::Catalogue;

use super::*;
use crate::settings::{Settings, delete_recipe, duplicate_recipe, save_recipe};

/// A catalogue over a profile directory of its own, which every write lands in.
fn writable(name: &str) -> Catalogue {
    let dir =
        std::env::temp_dir().join(format!("encrust-recipe-form-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Catalogue::with_root(&dir).expect("a missing directory reads as empty")
}

/// The user's recipe `id` as the catalogue keeps it, read back.
fn kept(catalogue: &Catalogue, id: &str) -> Recipe {
    let text = &catalogue.recipe(id).expect("kept").profile;
    Recipe::from_toml(text).expect("what was saved reads")
}

/// Settings with a copy of the built-in `prepare` open as the user's `mine`.
fn editing_a_copy(catalogue: &mut Catalogue) -> Settings {
    let mut settings = Settings::default();
    let shipped = core_recipe::find("prepare", []).expect("ships");
    duplicate_recipe(&mut settings, catalogue, &shipped).expect("a built-in recipe copies");
    settings
}

#[test]
fn a_step_moved_up_then_saved_reads_back_in_that_order() {
    let mut catalogue = writable("moved");
    let mut settings = editing_a_copy(&mut catalogue);
    let draft = settings.recipe.as_mut().expect("the copy is open");
    draft.values.shift(4, true);
    let mut chosen = String::new();
    let id = save_recipe(&mut settings, &mut catalogue, &mut chosen).expect("still valid");
    let kinds: Vec<StepKind> = kept(&catalogue, &id).steps.iter().map(|s| s.kind).collect();
    use StepKind::{Drain, Hollow, Orient, Repair, Supports};
    assert_eq!(kinds, [Orient, Repair, Hollow, Supports, Drain]);
}

#[test]
fn moving_the_first_step_up_or_the_last_down_leaves_the_order() {
    let mut form = RecipeForm::of(built_in("prepare").expect("ships").recipe());
    let before = form.clone();
    form.shift(0, true);
    form.shift(4, false);
    assert_eq!(form, before);
}

#[test]
fn a_when_that_does_not_read_marks_its_field_and_blocks_saving() {
    let mut catalogue = writable("invalid");
    let mut settings = editing_a_copy(&mut catalogue);
    let id = settings.recipe.as_ref().expect("open").id.clone();
    let before = catalogue.recipe(&id).expect("kept").profile.clone();
    let draft = settings.recipe.as_mut().expect("open");
    draft.values.steps[2].when = "saves_ml >".to_owned();
    let checked = draft.values.check().expect_err("half a comparison");
    assert!(matches!(
        checked.at(&Field::When(2)),
        Some(RecipeError::Syntax { .. })
    ));
    let mut chosen = String::new();
    let refused = save_recipe(&mut settings, &mut catalogue, &mut chosen);
    assert!(matches!(
        refused
            .expect_err("not saved")
            .downcast_ref::<RecipeError>(),
        Some(RecipeError::Syntax { .. })
    ));
    assert_eq!(catalogue.recipe(&id).expect("kept").profile, before);
}

#[test]
fn a_name_measured_only_later_is_the_recipes_error_at_its_step() {
    let mut form = RecipeForm::of(built_in("prepare").expect("ships").recipe());
    form.steps[0].when = "trapped > 0".to_owned();
    let checked = form.check().expect_err("trapped is measured by a drain");
    assert!(matches!(
        checked.whole,
        Some((Some(0), RecipeError::NotYetKnown { .. }))
    ));
}

#[test]
fn a_built_in_recipe_opens_to_read_and_its_duplicate_can_be_changed() {
    let mut catalogue = writable("built-in");
    let mut settings = Settings::default();
    let shipped = core_recipe::find("prepare", []).expect("ships");
    assert!(settings.open_recipe(&shipped));
    assert!(settings.recipe_is_built_in());
    let mut chosen = "prepare".to_owned();
    assert!(save_recipe(&mut settings, &mut catalogue, &mut chosen).is_err());
    assert!(delete_recipe(&mut settings, &mut catalogue, "prepare", &mut chosen).is_err());
    assert_eq!(catalogue.recipes().count(), 0);

    let id = duplicate_recipe(&mut settings, &mut catalogue, &shipped).expect("copies");
    assert!(!settings.recipe_is_built_in());
    assert_eq!(kept(&catalogue, &id).name, "Prepare for print copy");
    let draft = settings.recipe.as_mut().expect("the copy is open");
    draft.values.steps[0].enabled = false;
    save_recipe(&mut settings, &mut catalogue, &mut chosen).expect("the copy is the user's");
    assert!(!kept(&catalogue, &id).steps[0].enabled);
}

#[test]
fn a_renamed_recipe_moves_to_its_new_stem_and_the_picker_follows() {
    let mut catalogue = writable("renamed");
    let mut settings = editing_a_copy(&mut catalogue);
    let was = settings.recipe.as_ref().expect("open").id.clone();
    let mut chosen = was.clone();
    settings.recipe.as_mut().expect("open").values.name = "Busts".to_owned();
    let id = save_recipe(&mut settings, &mut catalogue, &mut chosen).expect("valid");
    assert_eq!(id, "busts");
    assert_eq!(chosen, "busts");
    assert!(catalogue.recipe(&was).is_err(), "the old stem is gone");
}

#[test]
fn deleting_the_picked_recipe_puts_the_picker_back_on_the_built_in_one() {
    let mut catalogue = writable("deleted");
    let mut settings = editing_a_copy(&mut catalogue);
    let id = settings.recipe.as_ref().expect("open").id.clone();
    let mut chosen = id.clone();
    delete_recipe(&mut settings, &mut catalogue, &id, &mut chosen).expect("the user's");
    assert_eq!(chosen, crate::prepare::DEFAULT_RECIPE);
    assert!(catalogue.recipe(&id).is_err());
}

#[test]
fn an_expression_setting_that_does_not_read_marks_its_own_field() {
    let mut form = RecipeForm::of(built_in("prepare").expect("ships").recipe());
    form.steps[2].set.insert(
        "wall_mm".to_owned(),
        SettingForm::Expr("clamp(height_mm".to_owned()),
    );
    let checked = form.check().expect_err("the call is not closed");
    let field = Field::Setting(2, None, "wall_mm".to_owned());
    assert!(checked.at(&field).is_some(), "{checked:?}");
}

#[test]
fn a_form_read_from_a_recipe_writes_the_same_recipe_back() {
    for (id, _) in core_recipe::BUILT_IN {
        let plan = built_in(id).expect("ships");
        let again = RecipeForm::of(plan.recipe()).check().expect("still valid");
        assert_eq!(again.recipe(), plan.recipe(), "{id}");
    }
}

#[test]
fn nothing_open_saves_nothing_and_a_recipe_that_does_not_read_is_not_copied() {
    let mut catalogue = writable("refused");
    let mut settings = Settings::default();
    let mut chosen = "prepare".to_owned();
    assert!(save_recipe(&mut settings, &mut catalogue, &mut chosen).is_err());
    let odd = core_recipe::find("odd", [("odd", "name = ")]).expect("kept");
    assert!(duplicate_recipe(&mut settings, &mut catalogue, &odd).is_err());
    assert_eq!(catalogue.recipes().count(), 0);
    assert!(settings.recipe.is_none());
}
