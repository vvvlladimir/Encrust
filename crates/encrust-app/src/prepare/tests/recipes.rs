//! Recipes other than the built-in one: steps in another order, repeated, gated and skipped.

use core_recipe::{Recipe, validate};

use super::*;

fn plan(steps: &str) -> Plan {
    let text = format!("name = \"test\"\n{steps}");
    validate(Recipe::from_toml(&text).expect("the fixture reads")).expect("the fixture is valid")
}

impl Bench {
    fn start_by(&mut self, steps: &str) {
        let started = start_with(
            plan(steps),
            &mut self.doc,
            &mut self.tools,
            &mut self.machine,
        );
        started.expect("a model to prepare");
    }
}

const DRAIN_THEN_HOLLOW: &str = "[[step]]\ndo = \"drain\"\n\
    set = { at = \"points\", points = [[0.5, 0.5, 0.0]] }\n\
    [[step]]\ndo = \"hollow\"\nset = { wall_mm = 2.0 }\n";

#[test]
fn a_drain_before_hollow_drills_before_hollowing() {
    let (mut bench, id) = Bench::with(large_cube());
    bench.start_by(DRAIN_THEN_HOLLOW);
    bench.run_until(Step::Hollow);
    let object = bench.object(id);
    assert_eq!(object.hollow.drains().len(), 1, "drilled first");
    assert!(!object.hollow.is_hollow(), "while still solid");
    bench.run_to_end();

    let object = bench.object(id);
    assert!(object.hollow.is_hollow());
    assert_eq!(object.hollow.drains().len(), 1, "the one hole, kept");
    let hole = object.hollow.drains()[0].at;
    assert!(
        hole.z.abs() < 1e-3 && (hole.x - 20.0).abs() < 1e-3 && (hole.y - 20.0).abs() < 1e-3,
        "at the middle of the floor, (20, 20, 0), not {hole}"
    );
}

#[test]
fn a_gate_that_stops_puts_the_plate_back() {
    let (mut bench, id) = Bench::with(large_cube());
    let before = standing(bench.object(id));
    bench.start_by(
        "[[step]]\ndo = \"hollow\"\n\
         [[step]]\ndo = \"gate\"\nstop_if = \"hollow\"\nmessage = \"hollow, so stop\"\n",
    );
    bench.run_to_end();
    assert_eq!(
        bench.machine.status,
        Status::Error("hollow, so stop".into())
    );
    assert_eq!(standing(bench.object(id)), before, "as before Prepare");
    assert!(!bench.doc.history.is_held());
}

#[test]
fn a_step_that_fails_with_on_fail_skip_lets_the_run_go_on() {
    let (mut bench, id) = Bench::with(large_cube());
    bench.start_by(
        "[[step]]\ndo = \"supports\"\non_fail = \"skip\"\nset = { profile = \"no-such\" }\n\
         [[step]]\ndo = \"hollow\"\n",
    );
    bench.run_to_end();
    assert_eq!(
        bench.machine.status,
        Status::Info(tr!("prepare-done", count = 1))
    );
    let object = bench.object(id);
    assert!(object.supports.is_empty(), "the failed step placed nothing");
    assert!(object.hollow.is_hollow(), "the next step ran");
}

#[test]
fn a_step_whose_when_is_false_skips_its_model() {
    let (mut bench, tall) = Bench::with(large_cube());
    let low = cuboid(Vec3::new(60.0, 0.0, 0.0), Vec3::new(100.0, 40.0, 20.0));
    let summary = ImportSummary::made(diagnose(&low));
    let low = Imported::new(
        "low".to_owned(),
        Arc::new(low),
        Transform::default(),
        summary,
    );
    let low = bench.doc.scene.insert(low);
    bench.doc.scene.select(None);
    bench.frame();
    bench.start_by("[[step]]\ndo = \"hollow\"\nwhen = \"height_mm > 30\"\n");
    bench.run_to_end();
    assert!(bench.object(tall).hollow.is_hollow(), "40 mm tall");
    assert!(
        !bench.object(low).hollow.is_hollow(),
        "20 mm tall is skipped"
    );
}

/// The large cube held 10 mm over the plate, so that it needs supports.
fn floating_cube() -> (Bench, ObjectId) {
    let (mut bench, id) = Bench::with(large_cube());
    let object = bench.doc.scene.get_mut(id).expect("on the plate");
    object.transform.translation.z = 10.0;
    bench.frame();
    (bench, id)
}

#[test]
fn supports_again_replace_the_earlier_ones() {
    let (mut bench, id) = floating_cube();
    bench.start_by(
        "[[step]]\ndo = \"supports\"\n[[step]]\ndo = \"supports\"\nset = { profile = \"heavy\" }\n",
    );
    bench.run_to_end();
    let heavy = bench.machine.slicing.catalogue.support("heavy");
    let heavy = heavy.expect("the catalogue ships it").profile.name.clone();
    let points = bench.object(id).supports.points();
    assert!(!points.is_empty(), "the cube stands on supports");
    let groups = &bench.tools.supports.groups;
    let named = |point: &core_supports::SupportPoint| &groups[point.group as usize].profile.name;
    assert!(
        points.iter().all(|point| *named(point) == heavy),
        "every support is the later step's {heavy}"
    );
}

#[test]
fn a_step_the_window_cannot_run_yet_puts_the_plate_back() {
    let (mut bench, id) = Bench::with(large_cube());
    let before = standing(bench.object(id));
    bench.start_by("[[step]]\ndo = \"hollow\"\n[[step]]\ndo = \"check\"\n");
    bench.run_to_end();
    let said = Status::Error(tr!("prepare-step-not-yet", step = "check"));
    assert_eq!(bench.machine.status, said);
    assert_eq!(standing(bench.object(id)), before);
}
