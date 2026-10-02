use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::scene::Scene;

/// How many edits the plate remembers. A snapshot is a `Scene` clone, and every mesh in
/// one is behind an `Arc`, so the depth costs placements rather than geometry.
const DEPTH: usize = 64;

/// The edits made to the plate, and the ones taken back.
///
/// A snapshot is the whole scene rather than a pair of do and undo operations, so a tool
/// added later is undoable without writing its inverse; see `docs/decisions/0086`.
#[derive(Debug)]
pub struct History {
    past: VecDeque<Scene>,
    future: Vec<Scene>,
    /// The scene as it stood after the last recorded edit, which is what goes onto `past`
    /// when the next one lands.
    last: Scene,
    fingerprint: u64,
}

impl Default for History {
    fn default() -> Self {
        let last = Scene::default();
        Self {
            past: VecDeque::new(),
            future: Vec::new(),
            fingerprint: fingerprint(&last),
            last,
        }
    }
}

impl History {
    /// Records an edit once the scene has changed and the gesture that changed it is
    /// over. `settled` is false while a button is held, so a drag across the plate is one
    /// entry rather than one per frame.
    pub fn observe(&mut self, scene: &Scene, settled: bool) {
        if !settled {
            return;
        }
        let now = fingerprint(scene);
        if now == self.fingerprint {
            return;
        }

        self.past
            .push_back(std::mem::replace(&mut self.last, scene.clone()));
        if self.past.len() > DEPTH {
            self.past.pop_front();
        }
        self.future.clear();
        self.fingerprint = now;
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Puts the plate back the way it was before the last edit.
    pub fn undo(&mut self, scene: &mut Scene) -> bool {
        let Some(previous) = self.past.pop_back() else {
            return false;
        };
        self.future.push(std::mem::replace(scene, previous));
        self.settle(scene);
        true
    }

    /// Puts back the edit the last undo took away.
    pub fn redo(&mut self, scene: &mut Scene) -> bool {
        let Some(next) = self.future.pop() else {
            return false;
        };
        self.past.push_back(std::mem::replace(scene, next));
        self.settle(scene);
        true
    }

    /// The scene the window now holds is the one to measure the next edit against.
    fn settle(&mut self, scene: &Scene) {
        self.last = scene.clone();
        self.fingerprint = fingerprint(scene);
    }
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
        ] {
            value.to_bits().hash(&mut hasher);
        }

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
    use crate::scene::{ImportSummary, Imported, ObjectId};
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};

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

    fn add(scene: &mut Scene) -> ObjectId {
        scene.insert(Imported::new(
            "cube".to_owned(),
            Arc::new(unit_cube()),
            Transform::default(),
            summary(),
        ))
    }

    fn move_to(scene: &mut Scene, id: ObjectId, x: f32) {
        if let Some(object) = scene.get_mut(id) {
            object.transform.translation = Vec3::new(x, 0.0, 0.0);
        }
    }

    #[test]
    fn an_edit_can_be_taken_back_and_put_again() {
        let mut scene = Scene::default();
        let mut history = History::default();
        let id = add(&mut scene);
        history.observe(&scene, true);

        move_to(&mut scene, id, 10.0);
        history.observe(&scene, true);

        assert!(history.undo(&mut scene));
        assert_eq!(scene.get(id).map(|o| o.transform.translation.x), Some(0.0));
        assert!(history.redo(&mut scene));
        assert_eq!(scene.get(id).map(|o| o.transform.translation.x), Some(10.0));
    }

    #[test]
    fn undoing_an_import_empties_the_plate() {
        let mut scene = Scene::default();
        let mut history = History::default();
        add(&mut scene);
        history.observe(&scene, true);

        assert!(history.undo(&mut scene));
        assert!(scene.is_empty());
    }

    #[test]
    fn a_gesture_in_flight_is_one_entry() {
        let mut scene = Scene::default();
        let mut history = History::default();
        let id = add(&mut scene);
        history.observe(&scene, true);

        for step in 1..=5 {
            move_to(&mut scene, id, step as f32);
            history.observe(&scene, false);
        }
        history.observe(&scene, true);
        assert_eq!(
            history.past.len(),
            2,
            "the import and the drag, nothing else"
        );

        assert!(history.undo(&mut scene));
        assert_eq!(
            scene.get(id).map(|o| o.transform.translation.x),
            Some(0.0),
            "the whole drag goes back, not its last frame"
        );
    }

    #[test]
    fn a_frame_that_changed_nothing_records_nothing() {
        let mut scene = Scene::default();
        let mut history = History::default();
        add(&mut scene);
        history.observe(&scene, true);
        history.observe(&scene, true);

        assert!(history.undo(&mut scene));
        assert!(scene.is_empty());
        assert!(!history.can_undo());
    }

    #[test]
    fn a_new_edit_drops_what_was_undone() {
        let mut scene = Scene::default();
        let mut history = History::default();
        let id = add(&mut scene);
        history.observe(&scene, true);
        move_to(&mut scene, id, 10.0);
        history.observe(&scene, true);

        history.undo(&mut scene);
        assert!(history.can_redo());

        move_to(&mut scene, id, 20.0);
        history.observe(&scene, true);
        assert!(!history.can_redo(), "the redo branch is gone");
    }

    #[test]
    fn the_stack_stops_at_its_depth() {
        let mut scene = Scene::default();
        let mut history = History::default();
        let id = add(&mut scene);
        history.observe(&scene, true);

        for step in 1..=(DEPTH + 10) {
            move_to(&mut scene, id, step as f32);
            history.observe(&scene, true);
        }
        assert_eq!(history.past.len(), DEPTH);
    }

    #[test]
    fn nothing_to_undo_or_redo_is_not_a_failure() {
        let mut scene = Scene::default();
        let mut history = History::default();
        assert!(!history.undo(&mut scene));
        assert!(!history.redo(&mut scene));
    }

    #[test]
    fn painting_a_patch_is_an_edit_of_its_own() {
        let mut scene = Scene::default();
        let mut history = History::default();
        let id = add(&mut scene);
        history.observe(&scene, true);

        if let Some(object) = scene.get_mut(id) {
            let mesh = Arc::clone(&object.mesh);
            let adjacency = core_geometry::Adjacency::of(&mesh);
            object
                .supports
                .paint_face(&mesh, &adjacency, 0, 30.0, false, true);
        }
        history.observe(&scene, true);

        assert!(history.undo(&mut scene));
        assert!(
            scene
                .get(id)
                .is_some_and(|object| object.supports.painted().is_empty()),
            "the paint went back off the model"
        );
    }

    #[test]
    fn a_support_point_is_an_edit_of_its_own() {
        let mut scene = Scene::default();
        let mut history = History::default();
        let id = add(&mut scene);
        history.observe(&scene, true);

        if let Some(object) = scene.get_mut(id) {
            object
                .supports
                .add(Vec3::new(0.5, 0.5, 1.0), Transform::default(), 0);
        }
        history.observe(&scene, true);

        assert!(history.undo(&mut scene));
        assert_eq!(scene.get(id).map(|o| o.supports.point_count()), Some(0));
    }
}
