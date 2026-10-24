use std::sync::Arc;

pub use core_engine::project::Axis;
use core_geometry::{Aabb, Bvh, Mesh, MeshDiagnostics, Region, Scalar, Vec2, Vec3, center_of_mass};

use core_supports::ModelSupports;
use core_volume::ModelHollow;

use crate::drain::{PlugsLaid, Traps};

mod import_summary;
mod object;
mod plates;
mod selection;
mod support_surface;
pub use import_summary::{ImportSummary, Imported, Mapped};
pub use object::{OwnSettings, SceneObject, Taken};
use object::{PlacedBounds, flip, onto_plate};
use plates::plate_name;

/// A mesh with what the window keeps beside it: its hierarchy, its centre of mass and what
/// is wrong with it. Built where the mesh is made, on a worker, since on a million faces it
/// is a third of a second the window would otherwise stand still for.
#[derive(Clone)]
pub struct Shaped {
    pub mesh: Arc<Mesh>,
    bvh: Arc<Bvh>,
    center_of_mass: Vec3,
    diagnostics: MeshDiagnostics,
}

impl Shaped {
    pub fn of(mesh: Arc<Mesh>) -> Self {
        Self {
            bvh: Arc::new(Bvh::build(&mesh)),
            center_of_mass: center_of_mass(&mesh).unwrap_or(Vec3::ZERO),
            diagnostics: core_geometry::diagnose(&mesh),
            mesh,
        }
    }
}

/// Two shapes are one when their surfaces are: the rest is worked out from it.
impl PartialEq for Shaped {
    fn eq(&self, other: &Self) -> bool {
        self.mesh == other.mesh
    }
}

impl std::fmt::Debug for Shaped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shaped")
            .field("faces", &self.mesh.faces.len())
            .finish()
    }
}

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

impl SceneObject {}

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

    /// Whether a model sliced off `plate` still waits on its supports, which a stack cut
    /// now would leave out.
    pub fn supports_pending(&self, plate: u32) -> bool {
        self.printable(plate).any(SceneObject::supports_pending)
    }

    /// Whether anything at all would be sliced off `plate`.
    pub fn has_printable(&self, plate: u32) -> bool {
        self.printable(plate).next().is_some()
    }

    /// The scene as an undo step keeps it: without the cuts taken out exactly, which are
    /// models' worth of mesh each and are taken out again when it is put back (ADR 0322).
    pub fn snapshot(&self) -> Self {
        let mut kept = self.clone();
        kept.forget_drilled();
        kept
    }

    /// Lets go of every model's cuts taken out exactly.
    pub fn forget_drilled(&mut self) {
        for object in &mut self.objects {
            object.hollow.forget_drilled();
        }
    }

    /// Takes over what `other` drilled for each model whose cuts are still the ones it was
    /// drilled for.
    pub fn adopt_drilled(&mut self, other: &Self) {
        for object in &mut self.objects {
            if let Some(was) = other.get(object.id) {
                object.hollow.adopt_drilled(&was.hollow);
            }
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
            hollow: ModelHollow::default(),
            traps: Traps::default(),
            mapped: imported.mapped,
            relief_painted: Region::default(),
            own: OwnSettings::default(),
            lift_mm: None,
            plug_of: None,
            plugs_laid: Some(PlugsLaid::default()),
            bounds: PlacedBounds::default(),
            support_shell: None,
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
        // A copy lays plugs of its own, and a copied plug is the user's, not its model's.
        copy.plugs_laid = Some(PlugsLaid::default());
        copy.plug_of = None;

        let copy_id = copy.id;
        self.objects.push(copy);
        self.selected = vec![copy_id];
        Some(copy_id)
    }

    /// Lays a grid of copies out beside an object, each one a footprint and `gap_mm` from
    /// the last, and slides the whole grid onto a plate `plate_mm` across.
    ///
    /// The grid keeps its shape: everything in it, the original included, moves by one
    /// offset, and only far enough to stand inside the plate. A grid with no room on it
    /// starts at the plate's near corner and runs off the far one.
    pub fn array(
        &mut self,
        id: ObjectId,
        columns: usize,
        rows: usize,
        gap_mm: Scalar,
        plate_mm: Vec2,
    ) -> Vec<ObjectId> {
        let Some(bounds) = self.get(id).and_then(SceneObject::world_bounds) else {
            return Vec::new();
        };
        let (columns, rows) = (columns.max(1), rows.max(1));
        let footprint = Vec2::new(bounds.maxs.x - bounds.mins.x, bounds.maxs.y - bounds.mins.y);
        let step = footprint + Vec2::splat(gap_mm);
        let span = footprint
            + Vec2::new(
                step.x * (columns - 1) as Scalar,
                step.y * (rows - 1) as Scalar,
            );
        let onto = onto_plate(Vec2::new(bounds.mins.x, bounds.mins.y), span, plate_mm);
        // The near cell is moved first, so every copy is laid out from where the grid
        // ended up rather than from where the model was.
        if let Some(object) = self.get_mut(id) {
            object.transform.translation += Vec3::new(onto.x, onto.y, 0.0);
        }

        let mut copies = Vec::new();
        for row in 0..rows {
            for column in (0..columns).skip(usize::from(row == 0)) {
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
mod tests;
