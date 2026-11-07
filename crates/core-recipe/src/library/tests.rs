use super::*;
use crate::fixtures::step;

fn named(name: &str, body: &str) -> String {
    format!("name = \"{name}\"\n{body}")
}

#[test]
fn the_built_in_recipes_come_first_then_the_users_by_id() {
    let busts = named("Busts", &step("orient", ""));
    let anvils = named("Anvils", &step("supports", ""));
    let listed = library([("busts", busts.as_str()), ("anvils", anvils.as_str())]);
    let ids: Vec<&str> = listed.iter().map(|entry| entry.id.as_str()).collect();
    let shipped: Vec<&str> = BUILT_IN.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids[..shipped.len()], shipped[..]);
    assert_eq!(ids[shipped.len()..], ["anvils", "busts"]);
    assert!(listed.iter().all(|entry| entry.plan().is_ok()));
}

#[test]
fn a_built_in_id_wins_over_a_users_recipe_of_the_same_stem() {
    let mine = named("Mine", &step("orient", ""));
    let found = find("prepare", [("prepare", mine.as_str())]).expect("ships");
    assert_eq!(found.origin, Origin::BuiltIn);
    assert_eq!(found.name(), "Prepare for print");
    let listed = library([("prepare", mine.as_str())]);
    assert!(listed.last().is_some_and(Listed::is_shadowed));
}

#[test]
fn a_users_recipe_is_found_by_its_stem() {
    let busts = named("Busts", &step("orient", ""));
    let found = find("busts", [("busts", busts.as_str())]).expect("kept");
    assert_eq!(found.origin, Origin::User);
    assert_eq!(found.name(), "Busts");
    assert!(find("heads", [("busts", busts.as_str())]).is_none());
}

#[test]
fn a_recipe_that_reads_but_fails_its_check_keeps_its_steps_and_says_why() {
    let early = named("Early", &step("orient", "when = \"trapped > 0\""));
    let found = find("early", [("early", early.as_str())]).expect("kept");
    assert!(matches!(
        found.plan(),
        Err(RecipeError::NotYetKnown { step: 0, .. })
    ));
    assert_eq!(found.recipe().map(|recipe| recipe.steps.len()), Some(1));
    assert_eq!(found.name(), "Early");
}

#[test]
fn text_that_is_no_recipe_is_unreadable_and_named_by_its_id() {
    let found = find("odd", [("odd", "name = ")]).expect("kept");
    assert!(matches!(
        found.read,
        Read::Unreadable(RecipeError::Toml { .. })
    ));
    assert_eq!(found.name(), "odd");
}
