use std::sync::{Arc, Mutex};

use rayon::prelude::*;

pub use core_engine::project::Axis;
use core_geometry::{
    Aabb, Bvh, Heightmap, Mesh, MeshDiagnostics, Orientation, Quat, Scalar, Transform, UvMap, Vec3,
    Welded, center_of_mass,
};

use core_supports::ModelSupports;
use core_volume::ModelHollow;

use crate::drain::Traps;

/// Identifies one object for as long as the window is open. Never reused, so a stale
/// identifier resolves to nothing instead of to the wrong object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(u64);

impl ObjectId {
    /// A bare identifier, for the tests of modules that hold one without a scene to take
    /// it from.
    #[cfg(test)]
    pub fn for_test(raw: u64) -> Self {
        Self(raw)
    }
}

/// What mesh repair had to do to an imported model, kept so the Scene panel can show it.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportSummary {
    pub vertices_merged: usize,
    pub faces_removed: usize,
    pub orientation: Orientation,
    pub diagnostics: MeshDiagnostics,
}

impl ImportSummary {
    pub fn new(welded: &Welded, orientation: Orientation, diagnostics: MeshDiagnostics) -> Self {
        Self {
            vertices_merged: welded.vertices_merged,
            faces_removed: welded.faces_removed(),
            orientation,
            diagnostics,
        }
    }

    /// Nothing about this model would make slicing produce garbage.
    pub fn is_sound(&self) -> bool {
        self.diagnostics.is_sound() && self.orientation.orientable
    }
}

/// The texture an imported file carried, kept beside the model until a tool presses it in.
///
/// Both halves are in the model's own space and indexed by its faces, so they hold only
/// while the mesh does: pressing the relief replaces the mesh, and the map goes with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapped {
    /// Where each image came from, for the panel to name.
    pub names: Vec<String>,
    pub uvs: UvMap,
    /// One per image the map names, in the order it indexes them: a model built from
    /// materials textures each part from its own.
    pub heights: Vec<Heightmap>,
}

/// A model that has been loaded, repaired and placed, ready to go into the scene.
///
/// The hierarchy is built here, in [`Imported::new`], and this is the only way a mesh
/// reaches a [`SceneObject`]: the two can therefore never be out of step. Building one is
/// the expensive half of an import, and it happens on the worker thread that made it.
#[derive(Debug, Clone)]
pub struct Imported {
    pub name: String,
    pub mesh: Arc<Mesh>,
    pub bvh: Arc<Bvh>,
    pub transform: Transform,
    pub summary: ImportSummary,
    /// Where the model's mass sits in its own space, for the gizmo to stand on.
    pub center_of_mass: Vec3,
    /// The texture the file carried, if it carried one.
    pub mapped: Option<Arc<Mapped>>,
}

impl Imported {
    pub fn new(
        name: String,
        mesh: Arc<Mesh>,
        transform: Transform,
        summary: ImportSummary,
    ) -> Self {
        Self {
            bvh: Arc::new(Bvh::build(&mesh)),
            center_of_mass: center_of_mass(&mesh).unwrap_or(Vec3::ZERO),
            name,
            mesh,
            transform,
            summary,
            mapped: None,
        }
    }
}

/// One imported model placed on the build plate.
///
/// The mesh stays in model space and is shared behind an `Arc`, so the renderer and,
/// later, a background slicing thread read it without copying. Placement lives in
/// `transform` and reaches the GPU as a matrix.
#[derive(Debug, Clone)]
pub struct SceneObject {
    pub id: ObjectId,
    pub name: String,
    /// Which plate of the project this stands on; see `docs/decisions/0098`.
    pub plate: u32,
    pub mesh: Arc<Mesh>,
    /// The hierarchy of `mesh`, in the model's own space. Built alongside it by
    /// [`Imported::new`], and shared so that a clone of the object is free.
    pub bvh: Arc<Bvh>,
    pub transform: Transform,
    /// Where the model's mass sits in its own space. Measured alongside `bvh` by
    /// [`Imported::new`], and what the gizmo stands on; see `docs/decisions/0110`.
    pub center_of_mass: Vec3,
    pub summary: ImportSummary,
    pub visible: bool,
    /// Where the user put supports on this model, and the columns built from them.
    pub supports: ModelSupports,
    /// The cavity inside this model, and where its wall is to stay solid.
    pub hollow: ModelHollow,
    /// What the last drainage check found in it.
    pub traps: Traps,
    /// The texture the file carried, until it is pressed in.
    pub mapped: Option<Arc<Mapped>>,
    bounds: PlacedBounds,
}

/// The last bounds measured over a mesh's vertices, and the placement they were for, so
/// that a frame asking again with nothing moved does not walk a million vertices.
#[derive(Debug, Default)]
struct PlacedBounds(Mutex<Option<(usize, Transform, Aabb)>>);

impl Clone for PlacedBounds {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.cached()))
    }
}

impl PlacedBounds {
    fn cached(&self) -> Option<(usize, Transform, Aabb)> {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn get(&self, mesh: &Arc<Mesh>, transform: Transform) -> Option<Aabb> {
        let key = Arc::as_ptr(mesh) as usize;
        if let Some((cached_key, at, bounds)) = self.cached()
            && cached_key == key
            && at == transform
        {
            return Some(bounds);
        }
        let matrix = transform.to_matrix();
        let (lo, hi) = mesh
            .vertices
            .par_iter()
            .map(|vertex| {
                let placed = matrix.transform_point3(*vertex);
                (placed, placed)
            })
            .reduce_with(|(lo_a, hi_a), (lo_b, hi_b)| (lo_a.min(lo_b), hi_a.max(hi_b)))?;
        let bounds = Aabb::new(lo, hi);
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((key, transform, bounds));
        Some(bounds)
    }
}

impl SceneObject {
    /// Bounds of the placed model, in plate coordinates: of its vertices as placed, not
    /// of its own box turned, which grows by up to √3 at an angle. Cached per transform.
    pub fn world_bounds(&self) -> Option<Aabb> {
        self.bounds.get(&self.mesh, self.transform)
    }

    /// A direction in the model's own space, as the placement turns it. Scale bends a
    /// normal the other way from a point, hence the division.
    pub fn world_normal(&self, local: Vec3) -> Vec3 {
        (self.transform.rotation * (local / self.transform.scale)).normalize_or_zero()
    }

    /// Where the handles stand: the model's centre of mass in plate coordinates, turned
    /// and stretched the way the model is. See `docs/decisions/0110`.
    pub fn pivot(&self) -> Transform {
        Transform {
            translation: self
                .transform
                .to_matrix()
                .transform_point3(self.center_of_mass),
            ..self.transform
        }
    }

    /// Replaces the model's own geometry, as a relief pressed into it does.
    ///
    /// Everything indexed by the old surface goes: its supports, its cavity and the map
    /// that was pressed in are all about a mesh that no longer exists.
    pub fn reshape(&mut self, mesh: Arc<Mesh>) {
        self.bvh = Arc::new(Bvh::build(&mesh));
        self.center_of_mass = center_of_mass(&mesh).unwrap_or(Vec3::ZERO);
        self.summary = ImportSummary {
            diagnostics: core_geometry::diagnose(&mesh),
            ..self.summary.clone()
        };
        self.supports = ModelSupports::default();
        self.hollow = ModelHollow::on(Arc::clone(&mesh), Arc::clone(&self.bvh));
        self.traps.clear();
        self.mapped = None;
        self.mesh = mesh;
    }

    /// Places the model under a pivot the gizmo has dragged, turned or stretched, so
    /// that its centre of mass lands back on that pivot.
    pub fn settle(&mut self, pivot: Transform) {
        self.transform = Transform {
            translation: pivot.translation - pivot.rotation * (pivot.scale * self.center_of_mass),
            ..pivot
        };
    }

    /// Turns the model about its centre of mass until the face whose plate-space normal
    /// is `normal` looks straight down, and stands it on the plate. False for a zero
    /// normal, which a degenerate face reports.
    pub fn lay_face_down(&mut self, normal: Vec3) -> bool {
        let Some(normal) = normal.try_normalize() else {
            return false;
        };
        let pivot = self.pivot();
        let turn = Quat::from_rotation_arc(normal, Vec3::NEG_Z);
        self.settle(Transform {
            rotation: (turn * pivot.rotation).normalize(),
            ..pivot
        });
        self.stand_on_plate();
        true
    }

    /// Moves the model straight down, or up, until its lowest point touches the plate.
    pub fn stand_on_plate(&mut self) {
        if let Some(bounds) = self.world_bounds() {
            self.transform.translation += core_geometry::drop_to_plate(&bounds);
        }
    }
}

/// The scale that flips `axis` and leaves the other two alone.
fn flip(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(-1.0, 1.0, 1.0),
        Axis::Y => Vec3::new(1.0, -1.0, 1.0),
        Axis::Z => Vec3::new(1.0, 1.0, -1.0),
    }
}

/// Every model currently loaded, which plate each stands on, and which one is selected.
///
/// The plates are one scene rather than one scene each, so an undo snapshot costs the
/// same whether there is one plate or six; see `docs/decisions/0098`.
#[derive(Debug, Clone)]
pub struct Scene {
    objects: Vec<SceneObject>,
    /// One name per plate, and never empty: plate 0 always exists.
    plates: Vec<String>,
    active: u32,
    /// What the tools work on, in the order it was picked. The last one is the primary:
    /// what the inspector reads and writes. Empty means the tools take the whole plate;
    /// see `docs/decisions/0101`.
    selected: Vec<ObjectId>,
    /// Where a span of the list starts: the model picked without a modifier, which a
    /// shift-click reaches from.
    anchor: Option<ObjectId>,
    next_id: u64,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            plates: vec![plate_name(0)],
            active: 0,
            selected: Vec::new(),
            anchor: None,
            next_id: 0,
        }
    }
}

/// What a plate nobody has renamed is called.
fn plate_name(index: usize) -> String {
    format!("Plate {}", index + 1)
}

impl Scene {
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Every model in the project, on every plate. A caller that means "what is being
    /// edited" wants [`Scene::here`], and one that means "what will be printed" wants
    /// [`Scene::printable`].
    pub fn objects(&self) -> &[SceneObject] {
        &self.objects
    }

    pub fn plates(&self) -> &[String] {
        &self.plates
    }

    pub fn active_plate(&self) -> u32 {
        self.active
    }

    /// Models standing on the plate being edited, hidden ones included.
    pub fn here(&self) -> impl Iterator<Item = &SceneObject> {
        self.on_plate(self.active)
    }

    /// The same, to be edited.
    pub fn here_mut(&mut self) -> impl Iterator<Item = &mut SceneObject> {
        let active = self.active;
        self.objects
            .iter_mut()
            .filter(move |object| object.plate == active)
    }

    pub fn on_plate(&self, plate: u32) -> impl Iterator<Item = &SceneObject> {
        self.objects
            .iter()
            .filter(move |object| object.plate == plate)
    }

    /// What slicing `plate` would take: what stands on it and is not hidden.
    pub fn printable(&self, plate: u32) -> impl Iterator<Item = &SceneObject> {
        self.on_plate(plate).filter(|object| object.visible)
    }

    /// Whether anything at all would be sliced off `plate`.
    pub fn has_printable(&self, plate: u32) -> bool {
        self.printable(plate).next().is_some()
    }

    /// Makes `plate` the one being edited. A selection left on another plate is dropped,
    /// because every tool works on what is in front of the user.
    pub fn show_plate(&mut self, plate: u32) {
        if plate as usize >= self.plates.len() || plate == self.active {
            return;
        }
        self.active = plate;
        // A pick left on another plate would have the tools working out of sight.
        let here: Vec<ObjectId> = self.here().map(|object| object.id).collect();
        self.selected.retain(|id| here.contains(id));
    }

    /// Adds an empty plate at the end and makes it the one being edited.
    ///
    /// Its name is the lowest default nobody is using: counting off the plate count
    /// hands out a name already taken as soon as one has been removed.
    pub fn add_plate(&mut self) -> u32 {
        let name = (0..)
            .map(plate_name)
            .find(|name| !self.plates.contains(name))
            .unwrap_or_else(|| plate_name(self.plates.len()));
        self.plates.push(name);
        let plate = (self.plates.len() - 1) as u32;
        self.active = plate;
        self.selected.clear();
        plate
    }

    pub fn rename_plate(&mut self, plate: u32, name: String) {
        if let Some(slot) = self.plates.get_mut(plate as usize) {
            *slot = name;
        }
    }

    /// Takes a plate away with everything on it, and shifts the plates above it down. The
    /// last plate cannot go: a project always has somewhere to put a model.
    pub fn remove_plate(&mut self, plate: u32) {
        if self.plates.len() < 2 || plate as usize >= self.plates.len() {
            return;
        }
        self.plates.remove(plate as usize);
        self.objects.retain(|object| object.plate != plate);
        for object in &mut self.objects {
            if object.plate > plate {
                object.plate -= 1;
            }
        }
        self.active = self.active.min(self.plates.len() as u32 - 1);
        let left: Vec<ObjectId> = self.objects.iter().map(|object| object.id).collect();
        self.selected.retain(|id| left.contains(id));
    }

    /// Moves a model to another plate. It keeps where it stands, so a plate it does not
    /// fit is the arranger's problem rather than this one's.
    pub fn move_to_plate(&mut self, id: ObjectId, plate: u32) {
        if plate as usize >= self.plates.len() {
            return;
        }
        if let Some(object) = self.get_mut(id) {
            object.plate = plate;
        }
        if plate != self.active {
            self.selected.retain(|picked| *picked != id);
        }
    }

    pub fn objects_mut(&mut self) -> &mut [SceneObject] {
        &mut self.objects
    }

    pub fn get(&self, id: ObjectId) -> Option<&SceneObject> {
        self.objects.iter().find(|object| object.id == id)
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut SceneObject> {
        self.objects.iter_mut().find(|object| object.id == id)
    }

    /// The object the inspector is talking about: the last one picked.
    pub fn selected(&self) -> Option<ObjectId> {
        self.selected.last().copied()
    }

    /// Everything picked, in the order it was picked.
    pub fn selection(&self) -> &[ObjectId] {
        &self.selected
    }

    pub fn is_selected(&self, id: ObjectId) -> bool {
        self.selected.contains(&id)
    }

    /// What a tool works on: the selection, or the whole plate when nothing is picked.
    ///
    /// Only what is on the plate being edited and not hidden ever comes back, so a stale
    /// pick cannot reach a model the user is not looking at. See `docs/decisions/0101`.
    pub fn targets(&self) -> impl Iterator<Item = &SceneObject> {
        let taking_all = self.selected.is_empty();
        self.printable(self.active)
            .filter(move |object| taking_all || self.selected.contains(&object.id))
    }

    /// The same, to be edited.
    pub fn targets_mut(&mut self) -> impl Iterator<Item = &mut SceneObject> {
        let active = self.active;
        let selected = self.selected.clone();
        let taking_all = selected.is_empty();
        self.objects.iter_mut().filter(move |object| {
            object.plate == active
                && object.visible
                && (taking_all || selected.contains(&object.id))
        })
    }

    /// How many models a tool would act on.
    pub fn target_count(&self) -> usize {
        self.targets().count()
    }

    /// Whether a tool would act on a subset of the plate rather than all of it, which is
    /// what the buttons say out loud.
    pub fn has_selection(&self) -> bool {
        !self.selected.is_empty()
    }

    /// What a tool is about to work on, for the line under its button.
    pub fn scope(&self) -> String {
        let count = self.target_count();
        match (self.has_selection(), count) {
            (true, 1) => "the selected model".to_owned(),
            (true, count) => format!("the {count} selected models"),
            (false, 1) => "the one model on the plate".to_owned(),
            (false, count) => format!("all {count} models on the plate"),
        }
    }

    /// Replaces the selection. Picking an object that is not in the scene clears it.
    pub fn select(&mut self, id: Option<ObjectId>) {
        self.selected = id
            .filter(|id| self.get(*id).is_some())
            .into_iter()
            .collect();
        self.anchor = self.selected.first().copied();
    }

    /// Adds an object to the selection, or takes it out if it is already in: what a
    /// modifier-click does in every application that has more than one thing to pick.
    pub fn toggle_selected(&mut self, id: ObjectId) {
        if self.get(id).is_none() {
            return;
        }
        match self.selected.iter().position(|picked| *picked == id) {
            Some(at) => {
                self.selected.remove(at);
            }
            None => self.selected.push(id),
        }
        self.anchor = Some(id);
    }

    /// Picks everything standing between the model a span was last started from and `id`,
    /// in the order the plate lists them: what a shift-click does in a list.
    ///
    /// The span is measured from the same model however often it is redrawn, so dragging
    /// the shift-click back over the list shrinks the selection rather than growing it.
    pub fn select_span_to(&mut self, id: ObjectId) {
        let Some(anchor) = self.anchor.filter(|anchor| self.get(*anchor).is_some()) else {
            return self.select(Some(id));
        };
        let here: Vec<ObjectId> = self.here().map(|object| object.id).collect();
        let (Some(from), Some(to)) = (
            here.iter().position(|other| *other == anchor),
            here.iter().position(|other| *other == id),
        ) else {
            return self.select(Some(id));
        };
        let span = if from <= to { from..=to } else { to..=from };
        self.selected = here[span].to_vec();
        self.anchor = Some(anchor);
    }

    /// Picks exactly these, dropping anything that is not in the scene.
    pub fn select_many(&mut self, ids: &[ObjectId]) {
        self.selected = ids
            .iter()
            .copied()
            .filter(|id| self.get(*id).is_some())
            .collect();
        self.anchor = self.selected.first().copied();
    }

    /// Takes every picked model off the plate. Returns how many went.
    pub fn remove_selected(&mut self) -> usize {
        self.anchor = None;
        let going = std::mem::take(&mut self.selected);
        self.objects.retain(|object| !going.contains(&object.id));
        going.len()
    }

    /// Picks everything on the plate being edited.
    pub fn select_here(&mut self) {
        self.selected = self.here().map(|object| object.id).collect();
        self.anchor = self.selected.first().copied();
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// Adds a model, gives it the next identifier, selects it and returns that identifier.
    pub fn insert(&mut self, imported: Imported) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;
        self.objects.push(SceneObject {
            id,
            name: imported.name,
            plate: self.active,
            mesh: Arc::clone(&imported.mesh),
            bvh: Arc::clone(&imported.bvh),
            transform: imported.transform,
            center_of_mass: imported.center_of_mass,
            summary: imported.summary,
            visible: true,
            supports: ModelSupports::default(),
            hollow: ModelHollow::on(imported.mesh, imported.bvh),
            traps: Traps::default(),
            mapped: imported.mapped,
            bounds: PlacedBounds::default(),
        });
        self.selected = vec![id];
        id
    }

    /// Copies an object with everything placed on it — its supports, its blockers, its
    /// cuts and its cavity — and stands the copy `offset` away. The copy is selected.
    ///
    /// The meshes are shared rather than copied: a copy costs a placement.
    pub fn duplicate(&mut self, id: ObjectId, offset: Vec3) -> Option<ObjectId> {
        let mut copy = self.get(id)?.clone();
        copy.id = ObjectId(self.next_id);
        self.next_id += 1;
        copy.transform.translation += offset;

        let copy_id = copy.id;
        self.objects.push(copy);
        self.selected = vec![copy_id];
        Some(copy_id)
    }

    /// Lays a grid of copies out beside an object, each one a footprint and `gap_mm`
    /// from the last, with the original in the near left corner.
    ///
    /// Nothing here knows the plate, so copies are laid out wherever the original stands;
    /// fitting them onto it is what arranging is for.
    pub fn array(
        &mut self,
        id: ObjectId,
        columns: usize,
        rows: usize,
        gap_mm: Scalar,
    ) -> Vec<ObjectId> {
        let Some(bounds) = self.get(id).and_then(SceneObject::world_bounds) else {
            return Vec::new();
        };
        let step = Vec3::new(
            bounds.maxs.x - bounds.mins.x + gap_mm,
            bounds.maxs.y - bounds.mins.y + gap_mm,
            0.0,
        );

        let mut copies = Vec::new();
        for row in 0..rows.max(1) {
            for column in 0..columns.max(1) {
                if row == 0 && column == 0 {
                    continue;
                }
                let offset = Vec3::new(step.x * column as Scalar, step.y * row as Scalar, 0.0);
                copies.extend(self.duplicate(id, offset));
            }
        }
        copies
    }

    /// Flips an object on one of its own axes, leaving it where it stands.
    ///
    /// The flip is a negative scale, which reverses winding; `transform_mesh` puts the
    /// faces back the right way round, so the copy still slices as a solid.
    pub fn mirror(&mut self, id: ObjectId, axis: Axis) -> bool {
        let Some(before) = self.get(id).and_then(SceneObject::world_bounds) else {
            return false;
        };
        let Some(object) = self.get_mut(id) else {
            return false;
        };
        object.transform.scale *= flip(axis);

        let Some(after) = self.get(id).and_then(SceneObject::world_bounds) else {
            return false;
        };
        let center = |bounds: &Aabb| (bounds.mins + bounds.maxs) / 2.0;
        let back = center(&before) - center(&after);
        if let Some(object) = self.get_mut(id) {
            object.transform.translation += back;
        }
        true
    }

    pub fn remove(&mut self, id: ObjectId) {
        self.objects.retain(|object| object.id != id);
        self.selected.retain(|picked| *picked != id);
    }

    /// Bounds covering every visible model on the plate being edited, or `None` when
    /// there is nothing on it.
    pub fn world_bounds(&self) -> Option<Aabb> {
        self.printable(self.active)
            .filter_map(SceneObject::world_bounds)
            .reduce(|a, b| Aabb::new(a.mins.min(b.mins), a.maxs.max(b.maxs)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Quat;
    use std::f32::consts::FRAC_PI_2;

    fn summary() -> ImportSummary {
        ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: core_geometry::diagnose(&unit_cube()),
        }
    }

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

    fn scene_with_cube() -> (Scene, ObjectId) {
        let mut scene = Scene::default();
        let id = scene.insert(crate::scene::Imported::new(
            "cube".to_owned(),
            Arc::new(unit_cube()),
            Transform::default(),
            summary(),
        ));
        (scene, id)
    }

    #[test]
    fn the_handles_stand_on_the_centre_of_mass_not_the_models_origin() {
        let (mut scene, id) = scene_with_cube();
        let object = scene.get_mut(id).expect("just added");
        object.transform.translation = Vec3::new(10.0, 20.0, 0.0);

        let pivot = object.pivot();
        // The cube spans 0..1 in its own space, so its mass sits half a millimetre in.
        assert!(
            pivot
                .translation
                .abs_diff_eq(Vec3::new(10.5, 20.5, 0.5), 1e-5),
            "expected the middle of the placed cube, got {:?}",
            pivot.translation
        );
    }

    /// The six points of an octahedron of radius one: its own box is a cube of side two.
    fn octahedron() -> Mesh {
        let vertices = vec![Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        let faces = vec![
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ];
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_turned_model_is_bounded_by_its_vertices_not_by_its_turned_box() {
        let mut scene = Scene::default();
        let id = scene.insert(crate::scene::Imported::new(
            "octahedron".to_owned(),
            Arc::new(octahedron()),
            Transform {
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
                ..Transform::default()
            },
            summary(),
        ));
        let bounds = scene.get(id).and_then(SceneObject::world_bounds);
        let width = bounds.map(|bounds| bounds.maxs.x - bounds.mins.x);
        // (1, 0) and (0, 1) turned 45° land at x = ±√2/2; the turned box reaches ±√2.
        assert!(
            width.is_some_and(|width| (width - std::f32::consts::SQRT_2).abs() < 1e-5),
            "expected √2 across, got {width:?}"
        );
    }

    #[test]
    fn a_face_laid_down_looks_at_the_plate_and_rests_on_it() {
        let (mut scene, id) = scene_with_cube();
        let object = scene.get_mut(id).expect("just added");
        object.transform.translation = Vec3::new(10.0, 20.0, 7.0);

        assert!(object.lay_face_down(Vec3::X));

        let facing = object.transform.rotation * Vec3::X;
        assert!(
            facing.abs_diff_eq(Vec3::NEG_Z, 1e-5),
            "the +X face of the cube now points down, got {facing:?}"
        );
        let bounds = object.world_bounds().expect("the cube has vertices");
        assert!(
            bounds.mins.z.abs() < 1e-5,
            "resting on z = 0, got {bounds:?}"
        );
    }

    #[test]
    fn a_degenerate_face_turns_nothing() {
        let (mut scene, id) = scene_with_cube();
        let object = scene.get_mut(id).expect("just added");
        let before = object.transform;
        assert!(!object.lay_face_down(Vec3::ZERO));
        assert_eq!(object.transform, before);
    }

    #[test]
    fn dragging_the_handles_moves_the_model_by_the_same_amount() {
        let (mut scene, id) = scene_with_cube();
        let object = scene.get_mut(id).expect("just added");
        let mut pivot = object.pivot();
        pivot.translation += Vec3::new(3.0, -4.0, 5.0);
        object.settle(pivot);

        assert!(
            object
                .transform
                .translation
                .abs_diff_eq(Vec3::new(3.0, -4.0, 5.0), 1e-5)
        );
    }

    #[test]
    fn turning_the_handles_turns_the_model_about_its_centre_of_mass() {
        let (mut scene, id) = scene_with_cube();
        let object = scene.get_mut(id).expect("just added");
        object.transform.translation = Vec3::new(10.0, 20.0, 0.0);
        let before = object.pivot().translation;

        let mut pivot = object.pivot();
        pivot.rotation = Quat::from_rotation_z(FRAC_PI_2);
        object.settle(pivot);

        assert!(
            object.pivot().translation.abs_diff_eq(before, 1e-5),
            "a turn about the centre of mass leaves it where it was, got {:?}",
            object.pivot().translation
        );
        let bounds = object.world_bounds().expect("the cube has vertices");
        assert!(bounds.mins.abs_diff_eq(Vec3::new(10.0, 20.0, 0.0), 1e-5));
    }

    #[test]
    fn stretching_the_handles_grows_the_model_about_its_centre_of_mass() {
        let (mut scene, id) = scene_with_cube();
        let object = scene.get_mut(id).expect("just added");
        let before = object.pivot().translation;

        let mut pivot = object.pivot();
        pivot.scale = Vec3::splat(4.0);
        object.settle(pivot);

        assert!(object.pivot().translation.abs_diff_eq(before, 1e-5));
        let bounds = object.world_bounds().expect("the cube has vertices");
        // A unit cube centred on (0.5, 0.5, 0.5) and grown four times spans -1.5..2.5.
        assert!(bounds.mins.abs_diff_eq(Vec3::splat(-1.5), 1e-5));
        assert!(bounds.maxs.abs_diff_eq(Vec3::splat(2.5), 1e-5));
    }

    #[test]
    fn a_model_lands_on_the_plate_being_edited() {
        let (mut scene, first) = scene_with_cube();
        scene.add_plate();
        let second = scene.insert(crate::scene::Imported::new(
            "cube".to_owned(),
            Arc::new(unit_cube()),
            Transform::default(),
            summary(),
        ));

        assert_eq!(scene.get(first).expect("still there").plate, 0);
        assert_eq!(scene.get(second).expect("just added").plate, 1);
        assert_eq!(scene.here().count(), 1, "a plate shows only its own models");
        assert_eq!(scene.objects().len(), 2, "both are still in the project");
    }

    #[test]
    fn a_new_plate_takes_a_name_nobody_is_using() {
        let (mut scene, _) = scene_with_cube();
        scene.add_plate();
        scene.add_plate();
        assert_eq!(scene.plates(), ["Plate 1", "Plate 2", "Plate 3"]);

        // Removing the middle one and adding another must not hand out "Plate 3" twice:
        // two tabs reading the same is what a duplicate name looks like.
        scene.remove_plate(1);
        scene.add_plate();
        assert_eq!(scene.plates(), ["Plate 1", "Plate 3", "Plate 2"]);
    }

    #[test]
    fn switching_plates_drops_a_selection_left_behind() {
        let (mut scene, first) = scene_with_cube();
        scene.add_plate();
        assert_eq!(
            scene.selected(),
            None,
            "a new plate starts with nothing on it"
        );

        scene.show_plate(0);
        scene.select(Some(first));
        scene.show_plate(1);
        assert_eq!(scene.selected(), None);
    }

    #[test]
    fn a_removed_plate_takes_its_models_and_shifts_the_ones_above_it() {
        let (mut scene, ground) = scene_with_cube();
        scene.add_plate();
        let middle = scene.insert(crate::scene::Imported::new(
            "middle".to_owned(),
            Arc::new(unit_cube()),
            Transform::default(),
            summary(),
        ));
        scene.add_plate();
        let top = scene.insert(crate::scene::Imported::new(
            "top".to_owned(),
            Arc::new(unit_cube()),
            Transform::default(),
            summary(),
        ));

        scene.remove_plate(1);
        assert!(scene.get(middle).is_none(), "what stood on it went with it");
        assert_eq!(scene.get(ground).expect("plate 0 is untouched").plate, 0);
        assert_eq!(
            scene.get(top).expect("still there").plate,
            1,
            "shifted down"
        );
        assert_eq!(scene.plates().len(), 2);
    }

    #[test]
    fn the_last_plate_cannot_be_removed() {
        let (mut scene, id) = scene_with_cube();
        scene.remove_plate(0);
        assert_eq!(
            scene.plates().len(),
            1,
            "a project always has somewhere to put a model"
        );
        assert!(scene.get(id).is_some());
    }

    #[test]
    fn a_model_moved_to_another_plate_leaves_this_one() {
        let (mut scene, id) = scene_with_cube();
        scene.add_plate();
        scene.show_plate(0);
        scene.select(Some(id));

        scene.move_to_plate(id, 1);
        assert_eq!(scene.get(id).expect("still there").plate, 1);
        assert_eq!(scene.here().count(), 0);
        assert_eq!(
            scene.selected(),
            None,
            "it is no longer in front of the user"
        );
    }

    #[test]
    fn only_what_stands_on_a_plate_counts_as_printable() {
        let (mut scene, id) = scene_with_cube();
        assert!(scene.has_printable(0));
        scene.get_mut(id).expect("still there").visible = false;
        assert!(!scene.has_printable(0), "a hidden model is not printed");
        assert_eq!(scene.here().count(), 1, "but it is still listed");
    }

    /// A plate with `count` cubes on it, and the identifier of each.
    fn plate_of(count: usize) -> (Scene, Vec<ObjectId>) {
        let mut scene = Scene::default();
        let ids = (0..count)
            .map(|index| {
                scene.insert(crate::scene::Imported::new(
                    format!("cube {index}"),
                    Arc::new(unit_cube()),
                    Transform::default(),
                    summary(),
                ))
            })
            .collect();
        scene.clear_selection();
        (scene, ids)
    }

    #[test]
    fn with_nothing_picked_a_tool_takes_the_whole_plate() {
        let (scene, _) = plate_of(3);
        assert!(!scene.has_selection());
        assert_eq!(scene.target_count(), 3);
    }

    #[test]
    fn with_something_picked_a_tool_takes_only_that() {
        let (mut scene, ids) = plate_of(3);
        scene.select(Some(ids[1]));
        assert_eq!(scene.target_count(), 1);

        scene.toggle_selected(ids[2]);
        let targets: Vec<ObjectId> = scene.targets().map(|object| object.id).collect();
        assert_eq!(
            targets,
            [ids[1], ids[2]],
            "in the order they stand on the plate"
        );
    }

    /// Shift-click in a list takes everything between what was last picked and this,
    /// the way every list on the desktop reads it.
    #[test]
    fn a_shift_click_takes_the_span_between_two_rows() {
        let (mut scene, ids) = plate_of(5);
        scene.select(Some(ids[1]));
        scene.select_span_to(ids[3]);
        assert_eq!(scene.selection(), [ids[1], ids[2], ids[3]]);

        scene.select_span_to(ids[0]);
        assert_eq!(
            scene.selection(),
            [ids[0], ids[1]],
            "a span is measured from the same row, up the list as well as down"
        );
    }

    #[test]
    fn a_span_from_nothing_picks_the_row_it_landed_on() {
        let (mut scene, ids) = plate_of(3);
        scene.select_span_to(ids[2]);
        assert_eq!(scene.selection(), [ids[2]]);
    }

    /// A span starts at the row a modifier-click landed on, so holding cmd and then
    /// shift reaches from the one just added.
    #[test]
    fn a_modifier_click_moves_where_the_next_span_starts() {
        let (mut scene, ids) = plate_of(4);
        scene.select(Some(ids[0]));
        scene.toggle_selected(ids[3]);
        scene.select_span_to(ids[2]);
        assert_eq!(scene.selection(), [ids[2], ids[3]]);
    }

    #[test]
    fn a_modifier_click_on_something_already_picked_takes_it_back_out() {
        let (mut scene, ids) = plate_of(3);
        scene.select(Some(ids[0]));
        scene.toggle_selected(ids[1]);
        scene.toggle_selected(ids[0]);

        assert_eq!(scene.selection(), [ids[1]]);
        assert_eq!(
            scene.selected(),
            Some(ids[1]),
            "the last one left is the primary"
        );
    }

    #[test]
    fn a_hidden_model_is_no_target_even_when_it_is_picked() {
        let (mut scene, ids) = plate_of(2);
        scene.select_here();
        scene.get_mut(ids[0]).expect("still there").visible = false;

        assert_eq!(
            scene.target_count(),
            1,
            "a tool does not touch what is not drawn"
        );
        assert_eq!(
            scene.selection().len(),
            2,
            "but the pick is still the user's"
        );
    }

    #[test]
    fn a_pick_does_not_follow_the_user_to_another_plate() {
        let (mut scene, _) = plate_of(2);
        scene.select_here();
        scene.add_plate();

        assert!(scene.selection().is_empty());
        assert_eq!(scene.target_count(), 0, "and the new plate is empty");
    }

    #[test]
    fn deleting_takes_every_picked_model_at_once() {
        let (mut scene, ids) = plate_of(3);
        scene.select(Some(ids[0]));
        scene.toggle_selected(ids[2]);

        assert_eq!(scene.remove_selected(), 2);
        assert_eq!(scene.objects().len(), 1);
        assert_eq!(scene.objects()[0].id, ids[1]);
        assert!(scene.selection().is_empty());
    }

    #[test]
    fn what_a_tool_says_it_will_do_follows_what_is_picked() {
        let (mut scene, ids) = plate_of(3);
        assert_eq!(scene.scope(), "all 3 models on the plate");

        scene.select(Some(ids[0]));
        assert_eq!(scene.scope(), "the selected model");

        scene.toggle_selected(ids[1]);
        assert_eq!(scene.scope(), "the 2 selected models");
    }

    #[test]
    fn inserting_selects_the_new_object() {
        let (scene, id) = scene_with_cube();
        assert_eq!(scene.selected(), Some(id));
        assert_eq!(scene.objects().len(), 1);
    }

    #[test]
    fn removing_the_selected_object_clears_the_selection() {
        let (mut scene, id) = scene_with_cube();
        scene.remove(id);
        assert!(scene.is_empty());
        assert_eq!(scene.selected(), None);
    }

    #[test]
    fn identifiers_are_never_reused() {
        let (mut scene, first) = scene_with_cube();
        scene.remove(first);
        let second = scene.insert(crate::scene::Imported::new(
            "cube".to_owned(),
            Arc::new(unit_cube()),
            Transform::default(),
            summary(),
        ));
        assert_ne!(first, second);
        assert!(scene.get(first).is_none());
    }

    #[test]
    fn selecting_a_missing_object_clears_the_selection() {
        let (mut scene, id) = scene_with_cube();
        scene.remove(id);
        scene.select(Some(id));
        assert_eq!(scene.selected(), None);
    }

    #[test]
    fn world_bounds_follow_the_transform() {
        let (mut scene, id) = scene_with_cube();
        scene.objects_mut()[0].transform = Transform {
            translation: Vec3::new(10.0, 20.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(2.0),
        };

        let bounds = scene.get(id).and_then(SceneObject::world_bounds);
        let bounds = bounds.expect("the cube has vertices");
        assert!(bounds.mins.abs_diff_eq(Vec3::new(10.0, 20.0, 0.0), 1e-5));
        assert!(bounds.maxs.abs_diff_eq(Vec3::new(12.0, 22.0, 2.0), 1e-5));
    }

    #[test]
    fn rotating_the_cube_keeps_its_footprint_square() {
        let (mut scene, _) = scene_with_cube();
        scene.objects_mut()[0].transform = Transform {
            rotation: Quat::from_rotation_z(FRAC_PI_2),
            ..Transform::default()
        };

        // A quarter turn about Z maps the unit cube onto x in -1..0, y in 0..1.
        let bounds = scene.world_bounds().expect("one object is visible");
        let size = bounds.maxs - bounds.mins;
        assert!((size.x - 1.0).abs() < 1e-5, "a quarter turn keeps width 1");
        assert!((size.y - 1.0).abs() < 1e-5, "a quarter turn keeps depth 1");
    }

    #[test]
    fn hidden_objects_are_outside_the_world_bounds() {
        let (mut scene, _) = scene_with_cube();
        scene.objects_mut()[0].visible = false;
        assert!(scene.world_bounds().is_none());
    }

    #[test]
    fn empty_scene_has_no_bounds() {
        let scene = Scene::default();
        assert!(scene.world_bounds().is_none());
    }

    #[test]
    fn a_copy_carries_the_supports_of_what_it_was_copied_from() {
        let (mut scene, id) = scene_with_cube();
        if let Some(object) = scene.get_mut(id) {
            object
                .supports
                .add(Vec3::new(0.5, 0.5, 1.0), Transform::default(), 0);
        }

        let copy = scene
            .duplicate(id, Vec3::new(10.0, 0.0, 0.0))
            .expect("the cube is on the plate");
        assert_eq!(
            scene.selected(),
            Some(copy),
            "a copy is what you go on to move"
        );
        assert_ne!(copy, id);

        let copied = scene.get(copy).expect("the copy is on the plate");
        assert_eq!(copied.supports.point_count(), 1);
        assert_eq!(copied.transform.translation, Vec3::new(10.0, 0.0, 0.0));
    }

    #[test]
    fn an_array_lays_out_every_cell_but_the_one_already_filled() {
        let (mut scene, id) = scene_with_cube();
        let copies = scene.array(id, 3, 2, 1.0);

        assert_eq!(copies.len(), 5, "six cells, one of them the original");
        assert_eq!(scene.objects().len(), 6);

        // The unit cube with a 1 mm gap steps 2 mm, so the far corner stands at (4, 2).
        let last = scene.get(copies[4]).expect("the last copy is on the plate");
        assert_eq!(last.transform.translation, Vec3::new(4.0, 2.0, 0.0));
    }

    #[test]
    fn an_array_of_something_that_is_not_there_lays_out_nothing() {
        let (mut scene, id) = scene_with_cube();
        scene.remove(id);
        assert!(scene.array(id, 2, 2, 1.0).is_empty());
    }

    #[test]
    fn mirroring_leaves_the_model_where_it_stood() {
        let (mut scene, id) = scene_with_cube();
        scene.objects_mut()[0].transform.translation = Vec3::new(10.0, 20.0, 0.0);
        let before = scene.world_bounds().expect("the cube has vertices");

        assert!(scene.mirror(id, Axis::X));
        let after = scene
            .world_bounds()
            .expect("a mirrored cube still has vertices");
        assert!(after.mins.abs_diff_eq(before.mins, 1e-5));
        assert!(after.maxs.abs_diff_eq(before.maxs, 1e-5));
    }

    #[test]
    fn a_mirrored_model_is_still_solid() {
        let (mut scene, id) = scene_with_cube();
        assert!(scene.mirror(id, Axis::Y));

        let object = scene.get(id).expect("the cube is on the plate");
        let placed = core_geometry::transform_mesh(&object.mesh, object.transform);
        assert!(
            core_geometry::signed_volume(&placed) > 0.0,
            "the flip reverses winding, and the faces are put back the right way round"
        );
    }
}
