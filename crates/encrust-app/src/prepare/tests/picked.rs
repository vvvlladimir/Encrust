//! The recipe picked for a run.

use printer_profiles::Catalogue;

use super::*;

/// The bench over a profile directory of its own, holding `recipes` as the user's.
fn with_recipes(name: &str, recipes: &[(&str, &str)]) -> (Bench, ObjectId) {
    let (mut bench, id) = Bench::with(large_cube());
    let dir = std::env::temp_dir().join(format!("encrust-picked-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut catalogue = Catalogue::with_root(&dir).expect("a missing directory reads as empty");
    for (id, text) in recipes {
        catalogue.save_recipe(id, text).expect("writable");
    }
    bench.machine.slicing.catalogue = catalogue;
    (bench, id)
}

const ONLY_ARRANGE: &str = "name = \"Only arrange\"\n[[step]]\ndo = \"arrange\"\n";

#[test]
fn the_picked_recipe_is_remembered_and_is_the_one_run() {
    let (mut bench, _) = with_recipes("remembered", &[("only-arrange", ONLY_ARRANGE)]);
    bench.tools.prepare.recipe = "only-arrange".to_owned();
    let settings = ToolSettings::of(&bench.tools, &bench.machine.slicing);
    let mut tools = Tools::default();
    settings.apply(&mut tools, &mut bench.machine.slicing);
    assert_eq!(tools.prepare.recipe, "only-arrange");

    bench.start();
    let running = bench.tools.prepare.plan().expect("running");
    assert_eq!(running.recipe().name, "Only arrange");
    bench.run_to_end();
}

#[test]
fn a_users_recipe_that_fails_its_check_cannot_be_started_and_says_why() {
    let early = "name = \"Early\"\n[[step]]\ndo = \"orient\"\nwhen = \"trapped > 0\"\n";
    let (mut bench, _) = with_recipes("invalid", &[("early", early)]);
    bench.tools.prepare.recipe = "early".to_owned();
    let why = blocker(&bench.doc, &bench.tools, &bench.machine).expect("cannot run");
    assert!(why.contains("Early") && why.contains("trapped"), "{why}");
    let started = start(&mut bench.doc, &mut bench.tools, &mut bench.machine);
    let error = started.expect_err("refused");
    assert!(matches!(
        error.downcast_ref::<core_recipe::RecipeError>(),
        Some(core_recipe::RecipeError::NotYetKnown { .. })
    ));
    assert!(!bench.tools.prepare.is_running());
}
