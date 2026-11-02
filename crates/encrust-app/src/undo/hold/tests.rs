use core_geometry::{Vec3, cuboid, diagnose};

use super::*;
use crate::scene::{ImportSummary, Imported, ObjectId};
use crate::slicing::Slicing;
use crate::tool_settings::ToolSettings;
use crate::undo::Frame;

/// The window's state the history watches, a frame at a time.
#[derive(Default)]
struct Bench {
    scene: Scene,
    tools: Tools,
    slicing: Slicing,
    history: History,
}

impl Bench {
    fn frame(&mut self) {
        let tools = ToolSettings::of(&self.tools, &self.slicing);
        let settled = Frame {
            settled: true,
            typing: false,
        };
        let chosen = self.slicing.chosen();
        self.history.observe(&self.scene, &tools, chosen, settled);
    }

    fn hold(&mut self) {
        let tools = ToolSettings::of(&self.tools, &self.slicing);
        let chosen = self.slicing.chosen();
        self.history.hold(&self.scene, &tools, chosen);
    }

    fn add(&mut self) -> ObjectId {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let summary = ImportSummary::made(diagnose(&mesh));
        let imported = Imported::new("cube".to_owned(), mesh.into(), Default::default(), summary);
        self.scene.insert(imported)
    }

    fn x_of(&self, id: ObjectId) -> Option<f32> {
        Some(self.scene.get(id)?.transform.translation.x)
    }

    /// Three landings a frame apart, as a run of jobs lands them.
    fn land_three(&mut self, id: ObjectId) {
        for x in [1.0, 2.0, 3.0] {
            if let Some(object) = self.scene.get_mut(id) {
                object.transform.translation.x = x;
            }
            self.frame();
        }
    }
}

#[test]
fn landings_under_a_hold_are_one_undo() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.frame();
    bench.hold();
    bench.land_three(id);
    assert!(!bench.history.can_undo(), "undo waits while the hold runs");
    bench.history.release();
    bench.frame();

    let (scene, tools, slicing) = (&mut bench.scene, &mut bench.tools, &mut bench.slicing);
    assert!(bench.history.undo(scene, tools, slicing));
    assert_eq!(
        bench.x_of(id),
        Some(0.0),
        "one undo goes back past all three"
    );
    let (scene, tools, slicing) = (&mut bench.scene, &mut bench.tools, &mut bench.slicing);
    assert!(bench.history.redo(scene, tools, slicing));
    assert_eq!(bench.x_of(id), Some(3.0), "one redo brings all three back");
}

#[test]
fn an_abandoned_hold_puts_the_plate_back_and_records_nothing() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.frame();
    bench.hold();
    bench.land_three(id);
    bench
        .history
        .abandon(&mut bench.scene, &mut bench.tools, &mut bench.slicing);
    bench.frame();

    assert_eq!(bench.x_of(id), Some(0.0), "the plate as the hold found it");
    assert!(!bench.history.is_held());
    let (scene, tools, slicing) = (&mut bench.scene, &mut bench.tools, &mut bench.slicing);
    assert!(
        bench.history.undo(scene, tools, slicing),
        "the model's arrival"
    );
    assert!(
        bench.scene.objects().is_empty(),
        "nothing between it and the hold"
    );
}

#[test]
fn a_job_landing_under_a_hold_joins_its_entry() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.frame();
    let asked_by = bench.history.next_edit();
    bench.hold();
    bench.history.land(&mut bench.scene, asked_by, |scene| {
        if let Some(object) = scene.get_mut(id) {
            object.transform.translation.x = 5.0;
        }
    });
    bench.history.release();
    bench.frame();

    let (scene, tools, slicing) = (&mut bench.scene, &mut bench.tools, &mut bench.slicing);
    assert!(bench.history.undo(scene, tools, slicing));
    assert_eq!(
        bench.x_of(id),
        Some(0.0),
        "taken back with the hold's entry"
    );
}

#[test]
fn an_edit_still_being_typed_when_the_hold_begins_is_an_entry_of_its_own() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.frame();
    if let Some(object) = bench.scene.get_mut(id) {
        object.transform.translation.x = 7.0;
    }
    let tools = ToolSettings::of(&bench.tools, &bench.slicing);
    let typing = Frame {
        settled: true,
        typing: true,
    };
    let chosen = bench.slicing.chosen();
    bench.history.observe(&bench.scene, &tools, chosen, typing);
    bench.hold();
    bench.land_three(id);
    bench
        .history
        .abandon(&mut bench.scene, &mut bench.tools, &mut bench.slicing);
    assert_eq!(bench.x_of(id), Some(7.0), "the typed move stays");
}
