//! Recipes other than the built-in one: steps in another order, repeated, gated and skipped.

use core_recipe::{Recipe, validate};

use super::*;

pub(super) fn plan(steps: &str) -> Plan {
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
fn a_hollow_step_keeps_a_model_thinner_than_keep_solid_under_mm_solid() {
    let cavity_mm3 = |kept: &str| {
        let (mut bench, id) = Bench::with(large_cube());
        bench.start_by(&format!(
            "[[step]]\ndo = \"hollow\"\nset = {{ wall_mm = 2.0{kept} }}\n"
        ));
        bench.run_to_end();
        bench.object(id).hollow.cavity_mm3()
    };
    // The 40 mm cube's cavity is 36 mm across; the whole cube is under 50 mm.
    let open = cavity_mm3("");
    let expected = 36.0_f32.powi(3);
    assert!(
        (open - expected).abs() < expected * 0.02,
        "{expected} mm3 open, got {open}"
    );
    let kept = cavity_mm3(", keep_solid_under_mm = 50.0");
    assert_eq!(kept, 0.0, "a 40 mm cube is under 50 mm, so 0 mm3 of cavity");
}

#[test]
fn a_drain_at_points_measures_what_stays_trapped() {
    let (mut bench, _) = Bench::with(large_cube());
    bench.start_by(
        "[[step]]\ndo = \"hollow\"\n\
         [[step]]\ndo = \"drain\"\nset = { at = \"points\", points = [[0.5, 0.5, 0.0]] }\n\
         [[step]]\ndo = \"gate\"\nstop_if = \"trapped >= 0\"\nmessage = \"known\"\n",
    );
    bench.run_to_end();
    assert_eq!(
        bench.machine.status,
        Status::Error("known".into()),
        "the gate read trapped"
    );
}

#[test]
fn a_wall_worked_out_by_an_expression_is_the_one_saves_ml_reads() {
    let (mut bench, _) = Bench::with(large_cube());
    // 40³ less 6 · 40² times the 5 mm wall is 16 ml; at Hollow's 2 mm it would be 44.8 ml.
    bench.start_by(
        "[[step]]\ndo = \"gate\"\nstop_if = \"saves_ml < 20\"\nmessage = \"thick\"\n\
         [[step]]\ndo = \"hollow\"\nset = { wall_mm = \"= clamp(height_mm / 8, 1, 6)\" }\n",
    );
    bench.run_to_end();
    assert_eq!(bench.machine.status, Status::Error("thick".into()));
}

/// An egg lying tilted: no flat face to lay it on, so the search weighs what it samples.
fn tilted_egg() -> Mesh {
    let ball = core_geometry::ball(Vec3::ZERO, 10.0, 12, 24);
    let turn = Transform {
        rotation: Quat::from_rotation_x(1.0) * Quat::from_rotation_y(0.4),
        scale: Vec3::new(3.0, 1.5, 1.0),
        ..Transform::default()
    };
    core_geometry::transform_mesh(&ball, turn)
}

#[test]
fn orient_takes_the_engines_settings_whatever_the_orient_panel_holds() {
    let turned = |panel: core_plate::OrientSettings| {
        let (mut bench, id) = Bench::with(tilted_egg());
        bench.tools.orient.settings = panel;
        bench.start_by("[[step]]\ndo = \"orient\"\nset = { goal = \"fewest-supports\" }\n");
        bench.run_to_end();
        bench.object(id).transform.rotation
    };
    let engine = turned(core_plate::OrientSettings::default());
    let sampling_nothing = core_plate::OrientSettings {
        samples: 0,
        shortlist: 0,
        ..core_plate::OrientSettings::default()
    };
    let changed = turned(sampling_nothing);
    assert!(
        engine.angle_between(changed) < UNTURNED_RAD,
        "turned alike, {engine} and {changed}"
    );
}
