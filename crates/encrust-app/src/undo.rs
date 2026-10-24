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

impl Step {
    fn scene_mut(&mut self) -> Option<&mut Scene> {
        match self {
            Self::Plate(scene) | Self::Both(scene, _) => Some(scene),
            Self::Tools(_) => None,
        }
    }

    fn scene(&self) -> Option<&Scene> {
        match self {
            Self::Plate(scene) | Self::Both(scene, _) => Some(scene),
            Self::Tools(_) => None,
        }
    }
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
    /// Each entry with the number it was recorded under, which [`Self::land`] finds it by.
    past: VecDeque<(u64, Step)>,
    future: Vec<(u64, Step)>,
    /// The number the next recorded edit takes.
    next: u64,
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
            next: 0,
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
        self.past.push_back((self.next, step));
        self.next += 1;
        if self.past.len() > DEPTH {
            self.past.pop_front();
        }
        self.future.clear();
    }

    /// The number the next edit is recorded under: a run started now is asked for by it.
    pub fn next_edit(&self) -> u64 {
        self.next
    }

    /// Lets `change` put into the scene what a run built for edit `asked_by`, its
    /// [`Self::next_edit`] when it started, as part of that edit, so one undo takes back both.
    ///
    /// The plates held for the edits recorded since get it too, so undoing one of those keeps
    /// it; with `asked_by` not recorded yet, it joins the edit under way, as in a drag.
    /// Answers with what `change` does to `scene`.
    pub fn land<T>(
        &mut self,
        scene: &mut Scene,
        asked_by: u64,
        mut change: impl FnMut(&mut Scene) -> T,
    ) -> T {
        let caught_up = fingerprint(scene) == self.fingerprint;
        if let Some(index) = self.past.iter().position(|(id, _)| *id == asked_by) {
            self.carry_since(index, caught_up, &mut change);
        }
        let answer = change(scene);
        if caught_up {
            self.settle(scene);
        }
        answer
    }

    /// Hands `change` to every plate held after the edit at `index` of the past, and makes a
    /// tool edit there hold the plate it left, so its undo takes back what lands as well.
    fn carry_since<T>(
        &mut self,
        index: usize,
        caught_up: bool,
        change: &mut impl FnMut(&mut Scene) -> T,
    ) {
        if matches!(self.past[index].1, Step::Tools(_)) {
            let left = self
                .past
                .range(index + 1..)
                .find_map(|(_, step)| step.scene())
                .unwrap_or(&self.last)
                .clone();
            if let Some((id, Step::Tools(tools))) = self.past.remove(index) {
                self.past.insert(index, (id, Step::Both(left, tools)));
            }
        }
        for (_, step) in self.past.range_mut(index + 1..) {
            if let Some(scene) = step.scene_mut() {
                change(scene);
            }
        }
        // Caught up, it is settled from the scene; otherwise the edit under way undoes to it.
        if !caught_up {
            change(&mut self.last);
            self.fingerprint = fingerprint(&self.last);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Takes back the last edit, of the plate or of a tool.
    pub fn undo(&mut self, scene: &mut Scene, tools: &mut Tools, slicing: &mut Slicing) -> bool {
        let Some((id, previous)) = self.past.pop_back() else {
            return false;
        };
        let held = self.put_back(previous, scene, tools, slicing);
        self.follow_up(scene, tools);
        self.future.push((id, held));
        true
    }

    /// Puts back the edit the last undo took away.
    pub fn redo(&mut self, scene: &mut Scene, tools: &mut Tools, slicing: &mut Slicing) -> bool {
        let Some((id, next)) = self.future.pop() else {
            return false;
        };
        let held = self.put_back(next, scene, tools, slicing);
        self.follow_up(scene, tools);
        self.past.push_back((id, held));
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
                node.head.hash(&mut hasher);
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
mod tests;
