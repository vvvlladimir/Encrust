use std::sync::Arc;

use core_geometry::{Mesh, Transform, Vec3, cuboid, diagnose};

use super::*;
use crate::scene::{ImportSummary, Imported};
use crate::tool_settings::ToolSettings;
use crate::undo::Frame;

/// Coarse enough that a drain search's scans take a moment, fine enough to find the cavity.
const LAYER_MM: f32 = 0.5;

/// The window's state Prepare works on, run a frame at a time as the window runs it.
struct Bench {
    doc: Doc,
    tools: Tools,
    machine: Machine,
    /// No button is down, as the window's frames are unless a test holds one.
    settled: bool,
}

impl Bench {
    fn with(mesh: Mesh) -> (Self, ObjectId) {
        let mut machine = Machine::default();
        machine.slicing.material.layer_height_mm = LAYER_MM;
        let mut bench = Self {
            doc: Doc::default(),
            tools: Tools::default(),
            machine,
            settled: true,
        };
        let summary = ImportSummary::made(diagnose(&mesh));
        let imported = Imported::new(
            "model".to_owned(),
            Arc::new(mesh),
            Transform::default(),
            summary,
        );
        let id = bench.doc.scene.insert(imported);
        bench.frame();
        (bench, id)
    }

    /// One frame: the jobs polled in the window's order, then its record of the frame.
    fn frame(&mut self) -> bool {
        let (doc, tools, machine) = (&mut self.doc, &mut self.tools, &mut self.machine);
        let (scene, status) = (&mut doc.scene, &mut machine.status);
        let mut working = doc.repairs.poll(scene, status);
        working |= doc.drilling.poll(scene, status);
        working |= doc.surfaces.poll(scene, status);
        working |= tools.supports.poll(scene, status);
        working |= tools.hollow.poll(scene, status);
        working |= tools.drain.poll(scene, status);
        working |= tools.orient.poll(scene, &doc.plate, status);
        working |= poll(doc, tools, machine, self.settled);
        doc.surfaces.start(&mut doc.scene);
        tools.supports.settle_on_raft(&mut doc.scene);
        doc.drilling.start(&doc.scene, self.settled);
        let settings = ToolSettings::of(tools, &machine.slicing);
        let settled = Frame {
            settled: self.settled,
            typing: false,
        };
        let chosen = machine.slicing.chosen();
        doc.history.observe(&doc.scene, &settings, chosen, settled);
        working
    }

    fn start(&mut self) {
        start(&mut self.doc, &mut self.tools, &mut self.machine).expect("a model to prepare");
    }

    /// Frames until `step`'s job is running.
    fn run_until(&mut self, step: Step) {
        while self.tools.prepare.going() != Some(step) {
            assert!(
                self.tools.prepare.is_running(),
                "the run ended before {step:?}"
            );
            self.frame();
            std::thread::yield_now();
        }
    }

    fn run_to_end(&mut self) {
        while self.frame() || self.doc.drilling.is_running() || self.doc.surfaces.is_running() {
            std::thread::yield_now();
        }
    }

    fn object(&self, id: ObjectId) -> &SceneObject {
        self.doc.scene.get(id).expect("still on the plate")
    }

    fn undo(&mut self) -> bool {
        let Self {
            doc,
            tools,
            machine,
            ..
        } = self;
        doc.history
            .undo(&mut doc.scene, tools, &mut machine.slicing)
    }
}

/// What a plate edit changes about a model, to tell the plate before from after.
fn standing(object: &SceneObject) -> (usize, Transform, bool, usize, bool) {
    (
        Arc::as_ptr(&object.mesh) as usize,
        object.transform,
        object.hollow.is_hollow(),
        object.hollow.drains().len(),
        object.supports.is_empty(),
    )
}

fn large_cube() -> Mesh {
    cuboid(Vec3::ZERO, Vec3::splat(40.0))
}

#[test]
fn a_large_solid_cube_is_oriented_hollowed_drained_and_supported_in_one_undo() {
    let (mut bench, id) = Bench::with(large_cube());
    let before = standing(bench.object(id));
    bench.start();
    bench.run_to_end();

    let object = bench.object(id);
    let wall_mm = object.hollow.asked().map(|asked| asked.thickness_mm);
    assert_eq!(wall_mm, Some(2.0), "hollowed at Hollow's default 2 mm wall");
    assert!(!object.hollow.drains().is_empty(), "drilled to drain");
    assert!(object.traps.found().is_empty(), "nothing left trapped");
    assert!(!object.supports.is_empty(), "stood on supports");
    let prepared = bench.tools.prepare.prepared(object).expect("said of it");
    assert!(!prepared.repaired && prepared.drained);
    let turned = object.transform.rotation.angle_between(Quat::IDENTITY) > UNTURNED_RAD;
    assert_eq!(
        prepared.oriented, turned,
        "said turned only where the search turned it"
    );
    // 40³ less its 6 · 40² surface times the 2 mm wall, as the engine reckons it.
    let saves_mm3 = prepared.hollowed.map(|wall| wall.saves_mm3);
    let off_mm3 = saves_mm3.map(|saves| (saves - 44_800.0).abs());
    assert!(
        off_mm3.is_some_and(|off| off < 1.0),
        "saves 44800 mm³, said {saves_mm3:?}"
    );
    assert!(prepared.supports.is_some(), "by the automatic profile");
    assert!(!bench.doc.history.is_held());

    assert!(bench.undo(), "the run is an entry");
    assert_eq!(
        standing(bench.object(id)),
        before,
        "one undo takes all of it back"
    );
}

#[test]
fn an_open_cube_is_repaired_first_and_then_hollowed() {
    let mut open = large_cube();
    open.faces.pop();
    let (mut bench, id) = Bench::with(open);
    bench.start();
    bench.run_to_end();

    let object = bench.object(id);
    assert!(object.summary.is_sound(), "the hole was closed");
    let prepared = bench.tools.prepare.prepared(object).expect("said of it");
    assert!(prepared.repaired, "its diagnosis was not sound");
    assert!(
        prepared.hollowed.is_some(),
        "diagnosed again once repaired, it is worth hollowing"
    );
}

#[test]
fn a_thin_plate_is_supported_and_never_hollowed() {
    let (mut bench, id) = Bench::with(cuboid(Vec3::ZERO, Vec3::new(60.0, 60.0, 3.0)));
    bench.start();
    bench.run_to_end();

    let object = bench.object(id);
    assert!(!object.hollow.is_hollow(), "a 3 mm plate is all wall");
    assert!(object.hollow.drains().is_empty());
    let prepared = bench.tools.prepare.prepared(object).expect("said of it");
    assert_eq!(prepared.hollowed, None);
    assert!(!prepared.drained);
}

#[test]
fn a_model_turned_by_hand_keeps_its_turn() {
    let (mut bench, id) = Bench::with(cuboid(Vec3::ZERO, Vec3::new(60.0, 60.0, 3.0)));
    let turned = Quat::from_rotation_y(0.3);
    let object = bench.doc.scene.get_mut(id).expect("on the plate");
    object.transform.rotation = turned;
    bench.frame();
    bench.start();
    bench.run_to_end();

    let object = bench.object(id);
    assert_eq!(
        object.transform.rotation, turned,
        "as --rotate keeps a turn"
    );
    let prepared = bench.tools.prepare.prepared(object).expect("said of it");
    assert!(!prepared.oriented);
}

#[test]
fn cancelled_mid_way_the_plate_is_as_it_was() {
    let (mut bench, id) = Bench::with(large_cube());
    let before = standing(bench.object(id));
    bench.start();
    while bench
        .tools
        .prepare
        .progress()
        .is_some_and(|(step, _)| step != Step::Drain)
    {
        bench.frame();
        std::thread::yield_now();
    }
    assert!(
        bench.object(id).hollow.is_hollow(),
        "hollowed before the drains"
    );

    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    cancel(doc, tools, machine);
    bench.run_to_end();
    assert_eq!(standing(bench.object(id)), before, "as before Prepare");
    assert!(
        !bench.tools.drain.auto.is_some(),
        "the drain search stopped"
    );
    assert!(!bench.tools.prepare.is_running());
    assert!(bench.undo(), "the model's arrival");
    assert!(
        bench.doc.scene.objects().is_empty(),
        "and nothing of Prepare's between"
    );
}

#[test]
fn it_waits_for_a_job_already_running() {
    let (mut bench, _) = Bench::with(large_cube());
    bench
        .tools
        .hollow
        .start(&bench.doc.scene)
        .expect("a model to hollow");
    assert!(blocker(&bench.doc, &bench.tools, &bench.machine).is_some());
    let started = start(&mut bench.doc, &mut bench.tools, &mut bench.machine);
    assert!(started.is_err());
    assert!(
        !bench.doc.history.is_held(),
        "nothing is held for a run that never began"
    );
}

#[test]
fn the_drain_panels_own_cancel_leaves_prepares_search_running() {
    let (mut bench, id) = Bench::with(large_cube());
    bench.start();
    bench.run_until(Step::Drain);
    let mut told = Status::default();
    bench.tools.drain.cancel_auto(&mut told);
    bench.run_to_end();

    let object = bench.object(id);
    assert!(
        !object.hollow.drains().is_empty(),
        "Prepare's own search drilled it"
    );
    let prepared = bench.tools.prepare.prepared(object).expect("said of it");
    assert!(prepared.drained);
}

impl Bench {
    /// The window's parts as a panel borrows them.
    fn with_window<T>(&mut self, act: impl FnOnce(&mut crate::panels::Window) -> T) -> T {
        let (mut mode, mut tool) = (Default::default(), Default::default());
        let mut view = crate::state::View::framing(&self.doc.plate);
        act(&mut crate::panels::Window {
            mode: &mut mode,
            tool: &mut tool,
            doc: &mut self.doc,
            view: &mut view,
            tools: &mut self.tools,
            machine: &mut self.machine,
        })
    }
}

fn hand_turned_plate() -> (Bench, ObjectId) {
    let (mut bench, id) = Bench::with(cuboid(Vec3::ZERO, Vec3::new(60.0, 60.0, 3.0)));
    let object = bench.doc.scene.get_mut(id).expect("on the plate");
    object.transform.rotation = Quat::from_rotation_y(0.3);
    bench.frame();
    (bench, id)
}

#[test]
fn prepared_again_a_drained_cube_is_not_said_drained() {
    let (mut bench, id) = Bench::with(large_cube());
    bench.start();
    bench.run_to_end();
    let holes = bench.object(id).hollow.drains().len();
    bench.start();
    bench.run_to_end();

    let object = bench.object(id);
    assert_eq!(object.hollow.drains().len(), holes, "nothing left to drain");
    let prepared = bench.tools.prepare.prepared(object).expect("said of it");
    assert!(
        !prepared.drained,
        "a search that drilled nothing drained nothing"
    );
}

#[test]
fn a_drainage_check_running_keeps_prepare_from_starting() {
    let (mut bench, _) = Bench::with(large_cube());
    let layer_mm = bench.machine.slicing.layer_height_mm();
    let checking = bench.tools.drain.start(&bench.doc.scene, layer_mm);
    checking.expect("a model to check");
    assert!(blocker(&bench.doc, &bench.tools, &bench.machine).is_some());
}

#[test]
fn a_button_held_down_keeps_the_next_step_off_an_undrilled_model() {
    let (mut bench, id) = Bench::with(large_cube());
    bench.start();
    bench.run_until(Step::Drain);
    bench.settled = false;
    while bench.tools.prepare.going() == Some(Step::Drain) {
        bench.frame();
        std::thread::yield_now();
    }
    for _ in 0..20 {
        bench.frame();
    }
    assert_eq!(bench.tools.prepare.going(), None, "waiting for the cuts");
    bench.settled = true;
    bench.run_to_end();
    let object = bench.object(id);
    assert!(
        !object.hollow.wants_drilling(object.uncut()),
        "drilled before supports"
    );
    assert!(!object.supports.is_empty());
}

#[test]
fn cancel_puts_back_a_tool_value_changed_while_it_ran() {
    let (mut bench, _) = Bench::with(large_cube());
    let wall_mm = bench.tools.hollow.state.thickness_mm;
    bench.start();
    bench.run_until(Step::Hollow);
    bench.tools.hollow.state.thickness_mm = wall_mm + 3.0;
    bench.frame();
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    cancel(doc, tools, machine);
    assert_eq!(bench.tools.hollow.state.thickness_mm, wall_mm);
}

#[test]
fn a_new_project_stops_the_run() {
    let (mut bench, _) = Bench::with(large_cube());
    bench.start();
    bench.with_window(crate::project::clear);
    assert!(!bench.tools.prepare.is_running());
}

#[test]
fn auto_orient_waits_while_prepare_runs() {
    let (mut bench, id) = Bench::with(large_cube());
    bench.start();
    bench.with_window(|window| crate::panels::auto_orient(window, Some(id)));
    assert!(
        !bench.tools.orient.is_running(),
        "the tool's own run is not started"
    );
    let why = Status::Info(tr!("prepare-running-why"));
    assert_eq!(bench.machine.status, why);
}

#[test]
fn a_model_removed_while_it_ran_is_not_counted() {
    let (mut bench, gone) = Bench::with(large_cube());
    let summary = ImportSummary::made(diagnose(&large_cube()));
    let model = Imported::new(
        "other".to_owned(),
        Arc::new(large_cube()),
        Transform::default(),
        summary,
    );
    bench.doc.scene.insert(model);
    bench.doc.scene.select(None);
    bench.frame();
    bench.start();
    bench.run_until(Step::Hollow);
    bench.doc.scene.remove(gone);
    bench.run_to_end();
    let said = Status::Info(tr!("prepare-done", count = 1));
    assert_eq!(bench.machine.status, said);
}

#[test]
fn the_bar_starts_at_nought_where_the_first_steps_are_not_taken() {
    let (mut bench, _) = hand_turned_plate();
    bench.start();
    let (step, share) = bench.tools.prepare.progress().expect("running");
    assert_eq!(
        step,
        Step::Supports,
        "nothing to turn, mend, hollow or drain"
    );
    assert!(
        share < 0.5,
        "the one step it takes is the whole bar, at {share}"
    );
}

#[test]
fn a_step_that_cannot_start_puts_the_plate_back() {
    let (mut bench, id) = Bench::with(large_cube());
    let before = standing(bench.object(id));
    bench.start();
    bench.run_until(Step::Hollow);
    bench.machine.slicing.catalogue = printer_profiles::Catalogue::default();
    bench.run_to_end();
    assert!(
        matches!(bench.machine.status, Status::Error(_)),
        "the step's error is said"
    );
    assert_eq!(standing(bench.object(id)), before, "as before Prepare");
    assert!(!bench.doc.history.is_held());
}

#[test]
fn the_history_replaced_mid_way_ends_the_run() {
    let (mut bench, _) = Bench::with(large_cube());
    bench.start();
    bench.doc.history = crate::undo::History::default();
    bench.frame();
    assert!(!bench.tools.prepare.is_running());
}

#[test]
fn nothing_on_the_plate_or_no_automatic_profile_starts_nothing() {
    let mut machine = Machine::default();
    let (mut doc, mut tools) = (Doc::default(), Tools::default());
    assert!(
        start(&mut doc, &mut tools, &mut machine).is_err(),
        "nothing visible"
    );
    let (mut bench, _) = Bench::with(large_cube());
    bench.machine.slicing.catalogue = printer_profiles::Catalogue::default();
    let Bench {
        doc,
        tools,
        machine,
        ..
    } = &mut bench;
    let started = start(doc, tools, machine);
    let said = started.expect_err("no automatic profile").to_string();
    assert_eq!(said, tr!("supports-no-automatic"));
    assert!(!bench.doc.history.is_held());
}

mod recipes;
