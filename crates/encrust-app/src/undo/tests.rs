use super::*;
use crate::drain::DrainTool;
use crate::scene::{ImportSummary, Imported, ObjectId};
use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
use printer_profiles::MaterialProfile;

/// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
fn unit_cube() -> Mesh {
    core_geometry::cuboid(Vec3::ZERO, Vec3::ONE)
}

fn summary() -> ImportSummary {
    ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: diagnose(&unit_cube()),
    }
}

/// The window's state the history watches, called the way a frame calls it.
#[derive(Default)]
struct Bench {
    scene: Scene,
    tools: Tools,
    slicing: Slicing,
    history: History,
}

impl Bench {
    fn observe(&mut self, settled: bool) {
        let tools = ToolSettings::of(&self.tools, &self.slicing);
        self.history.observe(
            &self.scene,
            &tools,
            self.slicing.chosen(),
            Frame {
                settled,
                typing: false,
            },
        );
    }

    /// A frame with a field still being typed into.
    fn observe_while_typing(&mut self) {
        let tools = ToolSettings::of(&self.tools, &self.slicing);
        self.history.observe(
            &self.scene,
            &tools,
            self.slicing.chosen(),
            Frame {
                settled: true,
                typing: true,
            },
        );
    }

    fn undo(&mut self) -> bool {
        self.history
            .undo(&mut self.scene, &mut self.tools, &mut self.slicing)
    }

    fn redo(&mut self) -> bool {
        self.history
            .redo(&mut self.scene, &mut self.tools, &mut self.slicing)
    }

    fn add(&mut self) -> ObjectId {
        self.scene.insert(Imported::new(
            "cube".to_owned(),
            Arc::new(unit_cube()),
            Transform::default(),
            summary(),
        ))
    }

    fn move_to(&mut self, id: ObjectId, x: f32) {
        if let Some(object) = self.scene.get_mut(id) {
            object.transform.translation = Vec3::new(x, 0.0, 0.0);
        }
    }

    fn x_of(&self, id: ObjectId) -> Option<f32> {
        self.scene.get(id).map(|o| o.transform.translation.x)
    }

    /// Lays a plug for the edit numbered `asked_by`, the way a run lands one.
    fn land_plug(&mut self, asked_by: u64) {
        self.history.land(&mut self.scene, asked_by, |scene| {
            let old = scene
                .objects()
                .iter()
                .find(|o| o.name == "plug")
                .map(|o| o.id);
            if let Some(old) = old {
                scene.remove(old);
            }
            scene.insert(Imported::new(
                "plug".to_owned(),
                Arc::new(unit_cube()),
                Transform::default(),
                summary(),
            ));
        });
    }

    fn has_plug(&self) -> bool {
        self.scene.objects().iter().any(|o| o.name == "plug")
    }
}

#[test]
fn an_edit_can_be_taken_back_and_put_again() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);

    bench.move_to(id, 10.0);
    bench.observe(true);

    assert!(bench.undo());
    assert_eq!(bench.x_of(id), Some(0.0));
    assert!(bench.redo());
    assert_eq!(bench.x_of(id), Some(10.0));
}

#[test]
fn what_a_run_lands_after_an_edit_is_taken_back_with_it() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);
    let asked_by = bench.history.next_edit();
    bench.move_to(id, 10.0);
    bench.observe(true);

    // A frame later a run lays a model the edit asked for, as plugs land.
    bench.land_plug(asked_by);
    bench.observe(true);

    assert!(bench.undo(), "the edit is there to undo");
    assert_eq!(bench.x_of(id), Some(0.0), "the move is taken back");
    assert!(!bench.has_plug(), "and what landed with it, in one step");
    assert!(bench.redo());
    assert!(bench.has_plug(), "a redo puts both again");
}

#[test]
fn what_lands_after_a_later_edit_stays_with_the_edit_that_asked_for_it() {
    let mut bench = Bench::default();
    let (drilled, other) = (bench.add(), bench.add());
    bench.observe(true);
    let asked_by = bench.history.next_edit();
    bench.move_to(drilled, 10.0);
    bench.observe(true);
    bench.move_to(other, 30.0);
    bench.observe(true);

    bench.land_plug(asked_by);
    bench.observe(true);

    assert!(bench.undo(), "the later move is there to undo");
    assert_eq!(bench.x_of(other), Some(0.0), "and is taken back");
    assert!(bench.has_plug(), "leaving what the earlier edit asked for");
    assert!(bench.undo());
    assert_eq!(
        bench.x_of(drilled),
        Some(0.0),
        "the edit that asked goes back"
    );
    assert!(!bench.has_plug(), "and takes what landed with it");
    assert!(bench.redo() && bench.redo());
    assert!(bench.has_plug(), "both redone put it again");
}

#[test]
fn what_a_tool_edit_asks_for_is_taken_back_with_it() {
    let mut bench = Bench::default();
    bench.add();
    bench.observe(true);
    let asked_by = bench.history.next_edit();
    bench.land_plug(asked_by);
    bench.observe(true);
    let first = bench.scene.objects().len();

    let asked_by = bench.history.next_edit();
    bench.tools.hollow.state.thickness_mm = 4.0;
    bench.observe(true);
    bench.land_plug(asked_by);
    bench.observe(true);
    let laid = bench.scene.objects().iter().map(|o| o.id).max();

    assert!(bench.undo(), "the tool edit is there to undo");
    assert_ne!(bench.tools.hollow.state.thickness_mm, 4.0);
    assert_eq!(bench.scene.objects().len(), first, "one plug, as before");
    assert_ne!(
        bench.scene.objects().iter().map(|o| o.id).max(),
        laid,
        "the plug laid before the tool edit, not the one it asked for"
    );
    assert!(bench.redo());
    assert_eq!(
        bench.scene.objects().iter().map(|o| o.id).max(),
        laid,
        "a redo puts the plug it asked for again"
    );
}

#[test]
fn what_a_run_lands_during_a_drag_joins_the_drag() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);
    let asked_by = bench.history.next_edit();
    bench.move_to(id, 10.0);
    bench.observe(false);

    bench.land_plug(asked_by);
    bench.observe(true);

    assert!(bench.undo(), "the drag is one edit");
    assert_eq!(bench.x_of(id), Some(0.0), "and is taken back");
    assert!(!bench.has_plug(), "with what landed in it");
}

/// A frame of the window: what follows from an edit by itself, then the history.
fn frame(bench: &mut Bench) {
    bench.tools.supports.settle_on_raft(&mut bench.scene);
    bench.observe(true);
}

#[test]
fn a_raft_switched_on_under_a_supported_model_is_one_step_each_way() {
    let mut bench = Bench::default();
    let id = bench.add();
    if let Some(object) = bench.scene.get_mut(id) {
        object.transform.translation.z = 5.0;
        let transform = object.transform;
        object.supports.add(Vec3::new(0.5, 0.5, 5.0), transform, 0);
    }
    frame(&mut bench);
    let z_of = |bench: &Bench| bench.scene.get(id).map(|o| o.transform.translation.z);

    bench.tools.supports.raft.enabled = true;
    frame(&mut bench);
    let raised = z_of(&bench);
    assert!(raised > Some(5.0), "the raft stood the model up");

    assert!(bench.undo());
    frame(&mut bench);
    assert!(!bench.tools.supports.raft.enabled);
    assert_eq!(z_of(&bench), Some(5.0), "back down with the raft gone");
    assert!(
        bench.history.can_redo(),
        "and nothing recorded over the redo"
    );

    assert!(bench.redo());
    frame(&mut bench);
    assert!(bench.tools.supports.raft.enabled);
    assert_eq!(z_of(&bench), raised);
    assert!(!bench.history.can_redo());
}

#[test]
fn taking_the_supports_off_a_rafted_model_comes_back_in_one_undo() {
    let mut bench = Bench::default();
    bench.tools.supports.raft.enabled = true;
    let id = bench.add();
    if let Some(object) = bench.scene.get_mut(id) {
        object.transform.translation.z = 5.0;
        let transform = object.transform;
        object.supports.add(Vec3::new(0.5, 0.5, 5.0), transform, 0);
    }
    frame(&mut bench);
    frame(&mut bench);

    if let Some(object) = bench.scene.get_mut(id) {
        object.supports.clear();
    }
    // Recorded before the model settles, the way a frame used to.
    bench.observe(true);
    frame(&mut bench);

    // Bounded: a history that redoes its own undo would go round for ever.
    for _ in 0..4 {
        if !bench.scene.get(id).is_some_and(|o| o.supports.is_empty()) || !bench.undo() {
            break;
        }
        frame(&mut bench);
    }
    let object = bench.scene.get(id).expect("the cube is still there");
    assert_eq!(object.supports.point_count(), 1, "the support came back");
}

#[test]
fn undoing_an_import_empties_the_plate() {
    let mut bench = Bench::default();
    bench.add();
    bench.observe(true);

    assert!(bench.undo());
    assert!(bench.scene.is_empty());
}

#[test]
fn a_gesture_in_flight_is_one_entry() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);

    for step in 1..=5 {
        bench.move_to(id, step as f32);
        bench.observe(false);
    }
    bench.observe(true);
    assert_eq!(
        bench.history.past.len(),
        2,
        "the import and the drag, nothing else"
    );

    assert!(bench.undo());
    assert_eq!(
        bench.x_of(id),
        Some(0.0),
        "the whole drag goes back, not its last frame"
    );
}

#[test]
fn a_frame_that_changed_nothing_records_nothing() {
    let mut bench = Bench::default();
    bench.add();
    bench.observe(true);
    bench.observe(true);

    assert!(bench.undo());
    assert!(bench.scene.is_empty());
    assert!(!bench.history.can_undo());
}

#[test]
fn a_new_edit_drops_what_was_undone() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);
    bench.move_to(id, 10.0);
    bench.observe(true);

    bench.undo();
    assert!(bench.history.can_redo());

    bench.move_to(id, 20.0);
    bench.observe(true);
    assert!(!bench.history.can_redo(), "the redo branch is gone");
}

#[test]
fn the_stack_stops_at_its_depth() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);

    for step in 1..=(DEPTH + 10) {
        bench.move_to(id, step as f32);
        bench.observe(true);
    }
    assert_eq!(bench.history.past.len(), DEPTH);
}

#[test]
fn nothing_to_undo_or_redo_is_not_a_failure() {
    let mut bench = Bench::default();
    assert!(!bench.undo());
    assert!(!bench.redo());
}

#[test]
fn painting_a_patch_is_an_edit_of_its_own() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);

    if let Some(object) = bench.scene.get_mut(id) {
        let mesh = Arc::clone(&object.mesh);
        let adjacency = core_geometry::Adjacency::of(&mesh);
        object
            .supports
            .paint_face(&mesh, &adjacency, 0, 30.0, false, true);
    }
    bench.observe(true);

    assert!(bench.undo());
    assert!(
        bench
            .scene
            .get(id)
            .is_some_and(|object| object.supports.painted().is_empty()),
        "the paint went back off the model"
    );
}

#[test]
fn a_support_point_is_an_edit_of_its_own() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);

    if let Some(object) = bench.scene.get_mut(id) {
        object
            .supports
            .add(Vec3::new(0.5, 0.5, 1.0), Transform::default(), 0);
    }
    bench.observe(true);

    assert!(bench.undo());
    assert_eq!(
        bench.scene.get(id).map(|o| o.supports.point_count()),
        Some(0)
    );
}

#[test]
fn a_tool_setting_is_taken_back_and_put_again() {
    let mut bench = Bench::default();
    let opened_with = bench.tools.hollow.state.thickness_mm;
    bench.observe(true);

    bench.tools.hollow.state.thickness_mm = 2.5;
    bench.observe(true);
    bench.tools.hollow.state.thickness_mm = 4.0;
    bench.observe(true);

    assert!(bench.undo());
    assert_eq!(bench.tools.hollow.state.thickness_mm, 2.5);
    assert!(bench.undo());
    assert_eq!(bench.tools.hollow.state.thickness_mm, opened_with);
    assert!(bench.redo());
    assert_eq!(bench.tools.hollow.state.thickness_mm, 2.5);
}

/// The wall the panel shows is the Hollow tool's, and the plate is the models on it:
/// a run that lands a shell must not be undone by a slider moved after it.
#[test]
fn taking_back_a_setting_leaves_the_plate_alone() {
    let mut bench = Bench::default();
    let id = bench.add();
    bench.observe(true);
    bench.move_to(id, 7.0);
    bench.observe(true);

    bench.tools.drain.state.diameter_mm = 5.0;
    bench.observe(true);

    assert!(bench.undo());
    assert_eq!(
        bench.tools.drain.state.diameter_mm,
        DrainTool::default().state.diameter_mm
    );
    assert_eq!(bench.x_of(id), Some(7.0), "the model stayed where it was");
}

#[test]
fn what_the_window_opens_with_is_not_an_edit() {
    let mut bench = Bench::default();
    bench.tools.cut.state.height_mm = 42.0;
    bench.observe(true);
    assert!(
        !bench.history.can_undo(),
        "the first frame is the baseline, not a change"
    );
}

/// A resin brings its own layer height and exposures. Taking them back would leave
/// the plate cut at a height the resin in hand was never measured at.
#[test]
fn choosing_a_resin_is_not_an_edit_of_the_tools() {
    let mut bench = Bench::default();
    bench.observe(true);

    bench.slicing.set_material(MaterialProfile {
        layer_height_mm: 0.08,
        ..MaterialProfile::default()
    });
    bench.observe(true);

    assert!(!bench.history.can_undo());
    assert!((bench.slicing.layer_height_mm() - 0.08).abs() < 1e-6);
}

/// A field parses what is in it on every keystroke, so "4.25" would otherwise be
/// three entries: 4, then 4.2, then 4.25.
#[test]
fn a_number_typed_in_digit_by_digit_is_one_entry() {
    let mut bench = Bench::default();
    bench.observe(true);

    for value in [4.0, 4.2, 4.25] {
        bench.tools.drain.state.diameter_mm = value;
        bench.observe_while_typing();
    }
    bench.observe(true);

    assert_eq!(bench.history.past.len(), 1);
    assert!(bench.undo());
    assert_eq!(
        bench.tools.drain.state.diameter_mm,
        DrainTool::default().state.diameter_mm,
        "the whole number goes back, not its last digit"
    );
}

#[test]
fn a_layer_height_typed_into_the_panel_is_taken_back() {
    let mut bench = Bench::default();
    bench.observe(true);
    let before = bench.slicing.layer_height_mm();

    bench.slicing.set_layer_height(0.02);
    bench.observe(true);

    assert!(bench.undo());
    assert!(
        (bench.slicing.layer_height_mm() - before).abs() < 1e-6,
        "the height goes back to what the resin was read at"
    );
}

#[test]
fn an_exposure_typed_into_the_print_settings_is_taken_back_and_put_again() {
    let mut bench = Bench::default();
    bench.observe(true);
    let resin = bench.slicing.material.clone();

    bench.slicing.material.exposure_s = 3.5;
    bench.slicing.material.waits.after_lift_s = 1.25;
    bench.observe(true);

    assert!(bench.undo());
    assert_eq!(bench.slicing.material.exposure_s, resin.exposure_s);
    assert_eq!(
        bench.slicing.material.waits.after_lift_s,
        resin.waits.after_lift_s
    );
    assert!(bench.redo());
    assert_eq!(bench.slicing.material.exposure_s, 3.5);
    assert_eq!(bench.slicing.material.waits.after_lift_s, 1.25);
}

#[test]
fn a_chosen_resin_brings_its_edge_smoothing_without_an_edit() {
    let mut bench = Bench::default();
    bench.observe(true);

    let mut resin = MaterialProfile::default();
    resin.edges.anti_alias = false;
    bench.slicing.set_material(resin);
    bench.observe(true);

    assert!(!bench.slicing.edges.anti_alias, "the resin's own default");
    assert!(!bench.history.can_undo());
}
