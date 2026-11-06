//! The recipe picked for a run, and the plan shown before and while it goes.

use core_recipe::Foreseen;
use printer_profiles::Catalogue;

use super::recipes::plan;
use super::*;
use crate::ui::PlanMark;

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

/// Frames until the foresight has measured the models in hand.
fn foreseen(bench: &mut Bench, plan: &Plan) -> Vec<Vec<Foreseen>> {
    let ids: Vec<ObjectId> = bench.doc.scene.targets().map(|object| object.id).collect();
    loop {
        let (scene, plate) = (&bench.doc.scene, &bench.doc.plate);
        if let Some(seen) = bench.tools.prepare.foresight.look(plan, scene, plate, &ids) {
            return seen.to_vec();
        }
        std::thread::yield_now();
    }
}

#[test]
fn the_picked_recipe_is_remembered_and_is_the_one_run() {
    let (mut bench, _) = with_recipes("remembered", &[("only-arrange", ONLY_ARRANGE)]);
    bench.tools.prepare.recipe = "only-arrange".to_owned();
    let settings = ToolSettings::of(&bench.tools, &bench.machine.slicing);
    let mut tools = Tools::default();
    settings.apply(&mut tools, &mut bench.machine.slicing);
    assert_eq!(tools.prepare.recipe, "only-arrange");

    bench.start();
    let (running, marks) = bench.tools.prepare.marks().expect("running");
    assert_eq!(running.recipe().name, "Only arrange");
    assert_eq!(marks.len(), 1);
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

#[test]
fn the_plan_marks_hollow_skipped_with_its_reading_on_a_model_not_worth_hollowing() {
    let (mut bench, _) = Bench::with(cuboid(Vec3::ZERO, Vec3::new(60.0, 60.0, 3.0)));
    let plan = core_recipe::built_in("prepare").expect("ships");
    let seen = foreseen(&mut bench, &plan);
    let lines = foreseen_lines(&plan, &seen);
    let hollow = &lines[2];
    assert_eq!(hollow.mark, PlanMark::Skipped);
    let reading = hollow.reading.as_deref().unwrap_or_default();
    assert!(reading.contains("worth_hollowing false"), "{reading}");
    assert_eq!(lines[5].mark, PlanMark::Runs, "supports run on anything");
}

#[test]
fn the_plan_notes_an_orient_after_supports_under_the_orient() {
    let (mut bench, _) = Bench::with(large_cube());
    let plan = plan("[[step]]\ndo = \"supports\"\n[[step]]\ndo = \"orient\"\n");
    let seen = foreseen(&mut bench, &plan);
    let lines = foreseen_lines(&plan, &seen);
    assert_eq!(lines[0].note, None);
    let note = lines[1].note.as_deref().unwrap_or_default();
    assert!(note.contains("supports from step 1"), "{note}");
}

#[test]
fn the_plan_is_measured_again_once_the_model_changes() {
    let (mut bench, id) = Bench::with(large_cube());
    let plan = core_recipe::built_in("prepare").expect("ships");
    let solid = foreseen_lines(&plan, &foreseen(&mut bench, &plan));
    assert_eq!(
        solid[2].mark,
        PlanMark::Runs,
        "a 40 mm cube is worth hollowing"
    );
    let object = bench.doc.scene.get_mut(id).expect("on the plate");
    object.mesh = Arc::new(cuboid(Vec3::ZERO, Vec3::new(60.0, 60.0, 3.0)));
    let thin = foreseen_lines(&plan, &foreseen(&mut bench, &plan));
    assert_eq!(thin[2].mark, PlanMark::Skipped);
}

#[test]
fn while_it_runs_the_plan_marks_steps_done_running_and_skipped() {
    let (mut bench, _) = Bench::with(cuboid(Vec3::ZERO, Vec3::new(60.0, 60.0, 3.0)));
    bench.start();
    bench.run_until(Step::Supports);
    let (plan, marks) = bench.tools.prepare.marks().expect("running");
    let lines = running_lines(plan, marks);
    let marks: Vec<PlanMark> = lines.iter().map(|line| line.mark).collect();
    assert_eq!(marks[2], PlanMark::Skipped, "not worth hollowing");
    assert_eq!(marks[5], PlanMark::Running);
    bench.run_to_end();
}
