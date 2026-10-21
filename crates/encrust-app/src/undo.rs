use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::scene::Scene;
use crate::slicing::Slicing;
use crate::state::Tools;
use crate::tool_settings::ToolSettings;

/// How many edits the plate remembers. A snapshot is a `Scene` clone, and every mesh in
/// one is behind an `Arc`, so the depth costs placements rather than geometry.
const DEPTH: usize = 64;

/// What one entry puts back: the plate as it stood, or the values the tools were set to.
///
/// Two kinds rather than one snapshot of both, because a tool's values are not part of
/// the scene: a slider moved while a run is landing a mesh must not put the plate back.
/// See `docs/decisions/0192`.
#[derive(Debug)]
enum Step {
    Plate(Scene),
    Tools(Box<ToolSettings>),
    /// Both changed in one frame, by one action: a raft switched on stands the supported
    /// models up by its thickness. Two entries would be taken back one at a time, and the
    /// half left behind would redo the other.
    Both(Scene, Box<ToolSettings>),
}

/// What the frame the history is looking at was like.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// No mouse button is down, so a drag across the plate is over.
    pub settled: bool,
    /// A field has the keyboard, so a number is still being typed into it.
    pub typing: bool,
}

/// The edits made to the plate and to the tools, and the ones taken back.
///
/// A snapshot is the whole scene rather than a pair of do and undo operations, so a tool
/// added later is undoable without writing its inverse; see `docs/decisions/0086`.
#[derive(Debug)]
pub struct History {
    past: VecDeque<Step>,
    future: Vec<Step>,
    /// The scene as it stood after the last recorded edit, which is what goes onto `past`
    /// when the next one lands.
    last: Scene,
    fingerprint: u64,
    /// The tool values as they stood after the last recorded change, or `None` until a
    /// frame has been seen: what the window opens a plate with is not an edit of it.
    tools: Option<Box<ToolSettings>>,
    /// How many printers and resins had been chosen when those values were read.
    chosen: u64,
}

impl Default for History {
    fn default() -> Self {
        let last = Scene::default();
        Self {
            past: VecDeque::new(),
            future: Vec::new(),
            fingerprint: fingerprint(&last),
            last,
            tools: None,
            chosen: 0,
        }
    }
}

impl History {
    /// Records an edit once the plate or a tool has changed and the gesture that changed
    /// it is over: a drag across the plate is one entry rather than one per frame, and a
    /// number typed into a field one rather than one per digit.
    ///
    /// `chosen` is `Slicing::chosen`: the layer height and exposures a newly picked resin
    /// brings with it are the profile's, not an edit, so they are taken as the baseline
    /// instead of recorded.
    pub fn observe(&mut self, scene: &Scene, tools: &ToolSettings, chosen: u64, frame: Frame) {
        if !frame.settled || frame.typing {
            return;
        }
        let now = fingerprint(scene);
        let plate = (now != self.fingerprint).then(|| {
            self.fingerprint = now;
            std::mem::replace(&mut self.last, scene.snapshot())
        });

        if chosen != self.chosen {
            // What a newly chosen printer or resin brought with it is what the next
            // change is measured against, not an edit of its own.
            self.chosen = chosen;
            self.tools = Some(Box::new(tools.clone()));
            if let Some(was) = plate {
                self.record(Step::Plate(was));
            }
            return;
        }
        let tools_were = match self.tools.as_deref_mut() {
            Some(last) if *last != *tools => Some(Box::new(std::mem::replace(last, tools.clone()))),
            Some(_) => None,
            None => {
                self.tools = Some(Box::new(tools.clone()));
                None
            }
        };
        match (plate, tools_were) {
            (Some(scene), Some(tools)) => self.record(Step::Both(scene, tools)),
            (Some(scene), None) => self.record(Step::Plate(scene)),
            (None, Some(tools)) => self.record(Step::Tools(tools)),
            (None, None) => {}
        }
    }

    /// Puts one entry on the stack, dropping the oldest when it is full, and with it the
    /// branch an undo had opened.
    fn record(&mut self, step: Step) {
        self.past.push_back(step);
        if self.past.len() > DEPTH {
            self.past.pop_front();
        }
        self.future.clear();
    }

    /// Lets `change` put into the scene what a run built for the edit last recorded, as part
    /// of that edit, so one undo takes back both; with an edit not recorded yet, as in a
    /// drag, it joins that one instead. Answers with what `change` does.
    pub fn land<T>(&mut self, scene: &mut Scene, change: impl FnOnce(&mut Scene) -> T) -> T {
        let caught_up = fingerprint(scene) == self.fingerprint;
        let answer = change(scene);
        if caught_up {
            self.settle(scene);
        }
        answer
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Takes back the last edit, of the plate or of a tool.
    pub fn undo(&mut self, scene: &mut Scene, tools: &mut Tools, slicing: &mut Slicing) -> bool {
        let Some(previous) = self.past.pop_back() else {
            return false;
        };
        let held = self.put_back(previous, scene, tools, slicing);
        self.follow_up(scene, tools);
        self.future.push(held);
        true
    }

    /// Puts back the edit the last undo took away.
    pub fn redo(&mut self, scene: &mut Scene, tools: &mut Tools, slicing: &mut Slicing) -> bool {
        let Some(next) = self.future.pop() else {
            return false;
        };
        let held = self.put_back(next, scene, tools, slicing);
        self.follow_up(scene, tools);
        self.past.push_back(held);
        true
    }

    /// Does what the window does by itself after an edit, to the plate just put back, and
    /// measures the next edit from there: done on the next frame instead, it would be an
    /// edit of its own, recorded over the redo and undoing the undo.
    fn follow_up(&mut self, scene: &mut Scene, tools: &mut Tools) {
        tools.supports.settle_on_raft(scene);
        self.settle(scene);
    }

    /// Puts `step` back, and answers with what the window was holding instead, which is
    /// the entry the other half of the stack takes.
    fn put_back(
        &mut self,
        step: Step,
        scene: &mut Scene,
        tools: &mut Tools,
        slicing: &mut Slicing,
    ) -> Step {
        match step {
            Step::Plate(previous) => {
                let held = put_scene_back(scene, previous);
                self.settle(scene);
                Step::Plate(held)
            }
            Step::Tools(previous) => Step::Tools(self.put_tools_back(*previous, tools, slicing)),
            Step::Both(previous, settings) => {
                let held = put_scene_back(scene, previous);
                let held_settings = self.put_tools_back(*settings, tools, slicing);
                Step::Both(held, held_settings)
            }
        }
    }

    /// Puts the tool values back, and answers with the ones the window was holding.
    fn put_tools_back(
        &mut self,
        previous: ToolSettings,
        tools: &mut Tools,
        slicing: &mut Slicing,
    ) -> Box<ToolSettings> {
        let held = ToolSettings::of(tools, slicing);
        previous.apply(tools, slicing);
        // A value can land clamped or rescaled, so the next frame is what the next change
        // is measured against rather than what was asked for.
        self.tools = None;
        Box::new(held)
    }

    /// The scene the window now holds is the one to measure the next edit against.
    fn settle(&mut self, scene: &Scene) {
        self.last = scene.snapshot();
        self.fingerprint = fingerprint(scene);
    }
}

/// Puts `previous` in place of `scene`, keeping what was drilled for cuts it shares, and
/// answers with the scene it held as a step keeps it.
fn put_scene_back(scene: &mut Scene, previous: Scene) -> Scene {
    let mut held = std::mem::replace(scene, previous);
    scene.adopt_drilled(&held);
    held.forget_drilled();
    held
}

/// Hashes what the user can edit: the plates, which models stand on which and where, and
/// the points and patches each tool has placed on them.
///
/// Meshes built from those inputs — the support columns, the cavity, the cut bodies — are
/// left out on purpose. They are rebuilt a frame or two after the edit that asked for
/// them, and hashing them would make one drag two entries on the stack.
fn fingerprint(scene: &Scene) -> u64 {
    let mut hasher = DefaultHasher::new();
    scene.objects().len().hash(&mut hasher);
    scene.plates().hash(&mut hasher);
    scene.active_plate().hash(&mut hasher);

    for object in scene.objects() {
        object.id.hash(&mut hasher);
        object.name.hash(&mut hasher);
        object.plate.hash(&mut hasher);
        object.visible.hash(&mut hasher);
        Arc::as_ptr(&object.mesh).hash(&mut hasher);
        object.hollow.is_hollow().hash(&mut hasher);
        serde_json::to_string(&object.own)
            .unwrap_or_default()
            .hash(&mut hasher);

        let transform = object.transform;
        for value in [
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
            transform.rotation.w,
            transform.scale.x,
            transform.scale.y,
            transform.scale.z,
        ]
        .into_iter()
        .chain(transform.stretch.to_cols_array())
        {
            value.to_bits().hash(&mut hasher);
        }

        object.relief_painted.hash(&mut hasher);
        object.supports.painted().hash(&mut hasher);
        object.supports.blocked().hash(&mut hasher);
        for tree in object.supports.frozen() {
            tree.group().hash(&mut hasher);
            for node in tree.nodes() {
                for value in [node.position.x, node.position.y, node.position.z] {
                    value.to_bits().hash(&mut hasher);
                }
            }
            let base = tree.landing().base;
            for value in [base.x, base.y, base.z] {
                value.to_bits().hash(&mut hasher);
            }
        }
        for point in object.supports.points() {
            for value in [point.contact.x, point.contact.y, point.contact.z] {
                value.to_bits().hash(&mut hasher);
            }
            point.group.hash(&mut hasher);
        }
        for blocker in object.hollow.blockers() {
            for value in [
                blocker.from.x,
                blocker.from.y,
                blocker.from.z,
                blocker.to.x,
                blocker.to.y,
                blocker.to.z,
                blocker.radius_mm,
            ] {
                value.to_bits().hash(&mut hasher);
            }
        }
        let (drains, channels) = object.hollow.cut();
        for drain in drains {
            for value in [
                drain.at.x,
                drain.at.y,
                drain.at.z,
                drain.diameter_mm,
                drain.depth_mm,
            ] {
                value.to_bits().hash(&mut hasher);
            }
        }
        for channel in channels {
            channel.points.len().hash(&mut hasher);
            for point in &channel.points {
                for value in [point.x, point.y, point.z] {
                    value.to_bits().hash(&mut hasher);
                }
            }
        }
        object.hollow.pending().len().hash(&mut hasher);
    }

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drain::DrainTool;
    use crate::scene::{ImportSummary, Imported, ObjectId};
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
    use printer_profiles::MaterialProfile;

    /// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
    fn unit_cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
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
        bench.move_to(id, 10.0);
        bench.observe(true);

        // A frame later a run lays a model the edit asked for, as plugs land.
        let landed = bench.history.land(&mut bench.scene, |scene| {
            scene.insert(Imported::new(
                "plug".to_owned(),
                Arc::new(unit_cube()),
                Transform::default(),
                summary(),
            ))
        });
        bench.observe(true);

        assert!(bench.undo(), "the edit is there to undo");
        assert_eq!(bench.x_of(id), Some(0.0), "the move is taken back");
        assert!(
            bench.scene.get(landed).is_none(),
            "and what landed with it, in one step"
        );
        assert!(bench.redo());
        assert!(bench.scene.get(landed).is_some(), "a redo puts both again");
    }

    #[test]
    fn what_a_run_lands_during_a_drag_joins_the_drag() {
        let mut bench = Bench::default();
        let id = bench.add();
        bench.observe(true);
        bench.move_to(id, 10.0);
        bench.observe(false);

        let landed = bench.history.land(&mut bench.scene, |scene| {
            scene.insert(Imported::new(
                "plug".to_owned(),
                Arc::new(unit_cube()),
                Transform::default(),
                summary(),
            ))
        });
        bench.observe(true);

        assert!(bench.undo(), "the drag is one edit");
        assert_eq!(bench.x_of(id), Some(0.0), "and is taken back");
        assert!(bench.scene.get(landed).is_none(), "with what landed in it");
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
}
