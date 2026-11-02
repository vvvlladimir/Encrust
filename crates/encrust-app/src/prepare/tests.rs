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
}

impl Bench {
    fn with(mesh: Mesh) -> (Self, ObjectId) {
        let mut machine = Machine::default();
        machine.slicing.material.layer_height_mm = LAYER_MM;
        let mut bench = Self {
            doc: Doc::default(),
            tools: Tools::default(),
            machine,
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
        working |= poll(doc, tools, machine);
        doc.surfaces.start(&mut doc.scene);
        tools.supports.settle_on_raft(&mut doc.scene);
        doc.drilling.start(&doc.scene, true);
        let settings = ToolSettings::of(tools, &machine.slicing);
        let settled = Frame {
            settled: true,
            typing: false,
        };
        let chosen = machine.slicing.chosen();
        doc.history.observe(&doc.scene, &settings, chosen, settled);
        working
    }

    fn start(&mut self) {
        start(&mut self.doc, &mut self.tools, &mut self.machine).expect("a model to prepare");
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
    assert!(prepared.oriented && !prepared.repaired && prepared.drained);
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
    } = &mut bench;
    cancel(doc, tools, &mut machine.status);
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
    assert!(blocker(&bench.doc, &bench.tools).is_some());
    let started = start(&mut bench.doc, &mut bench.tools, &mut bench.machine);
    assert!(started.is_err());
    assert!(
        !bench.doc.history.is_held(),
        "nothing is held for a run that never began"
    );
}
