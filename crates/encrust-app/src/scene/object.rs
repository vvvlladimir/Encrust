use super::import_summary::{ImportSummary, Mapped};
use super::support_surface::SupportShell;
use super::{ObjectId, Scene, Shaped};
use crate::drain::{PlugsLaid, Traps};
use crate::drilling::{PrintedSupports, SupportsUnder};
use core_engine::project::Axis;
use core_engine::project::{DrainState, HollowState};
use core_geometry::{Aabb, Bvh, Mesh, Quat, Region, Scalar, Transform, Vec2, Vec3};
use core_supports::ModelSupports;
use core_volume::ModelHollow;
use printer_profiles::{Raft, SupportProfile};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

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
    /// Its supports as they print, its cuts taken out, and their hierarchies.
    pub printed_supports: PrintedSupports,
    /// What the last drainage check found in it.
    pub traps: Traps,
    /// The texture the file carried, until it is pressed in.
    pub mapped: Option<Arc<Mapped>>,
    /// The faces the Relief brush painted: while any are, only they take the texture.
    pub relief_painted: Region,
    /// The tool settings this model keeps instead of the plate's.
    pub own: OwnSettings,
    /// How high over its raft the user stood the model, millimetres: what auto-orient
    /// lifts it by onto supports in place of the profile's lift.
    pub lift_mm: Option<f32>,
    /// The model this one is a plug for, laid beside it by the Drain tool (ADR 0313).
    pub plug_of: Option<ObjectId>,
    /// What this model's plugs were last laid for; `None` for one read back from a project,
    /// which takes its plugs as they stand.
    pub plugs_laid: Option<PlugsLaid>,
    pub(super) bounds: PlacedBounds,
    /// The printed surface supports stand against and its hierarchy, once built; see
    /// [`SceneObject::support_surface`].
    pub(super) support_shell: Option<SupportShell>,
}

/// The Hollow and Drain settings a model keeps instead of the plate's, where it has its
/// own, and the support profiles it is built to instead of its groups'; see
/// `docs/decisions/0227` and `docs/decisions/0246`.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct OwnSettings {
    pub hollow: Option<HollowState>,
    pub drain: Option<DrainState>,
    /// By the number of the group each one stands in for.
    pub supports: BTreeMap<u16, SupportProfile>,
    /// The raft under this model, where it is not the plate's; see ADR 0253.
    pub raft: Option<Raft>,
}

/// The last bounds measured over a mesh's vertices, and the placement they were for, so
/// that a frame asking again with nothing moved does not walk a million vertices.
#[derive(Debug, Default)]
pub(super) struct PlacedBounds(Mutex<Option<(usize, Transform, Aabb)>>);

impl Clone for PlacedBounds {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.cached()))
    }
}

impl PlacedBounds {
    pub(super) fn cached(&self) -> Option<(usize, Transform, Aabb)> {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn get(&self, mesh: &Arc<Mesh>, transform: Transform) -> Option<Aabb> {
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

/// A model as a background job took it, so what the job makes replaces only that model,
/// unmoved and unchanged.
#[derive(Debug, Clone)]
pub struct Taken {
    pub id: ObjectId,
    pub name: String,
    /// The plate it stood on, which is where what replaces it goes down.
    pub plate: u32,
    pub(super) mesh: Arc<Mesh>,
    pub(super) transform: Transform,
}

impl Taken {
    pub fn of(object: &SceneObject) -> Self {
        Self {
            id: object.id,
            name: object.name.clone(),
            plate: object.plate,
            mesh: Arc::clone(&object.mesh),
            transform: object.transform,
        }
    }

    /// Whether the model is still on the plate as it was taken. A new project reuses ids,
    /// so the mesh itself is compared, not only the id.
    pub fn is_current(&self, scene: &Scene) -> bool {
        scene.get(self.id).is_some_and(|object| {
            self.is_model(object)
                && object.transform == self.transform
                && object.plate == self.plate
        })
    }

    /// Whether `object` still stands turned and sized as the model was taken: what a job
    /// that rewrites the mesh in its own space depends on, where a move does not matter.
    pub fn is_shaped_alike_to(&self, object: &SceneObject) -> bool {
        self.is_model(object) && object.transform.linear() == self.transform.linear()
    }

    /// Whether `object` is the model taken, still with the mesh it was taken with.
    pub fn is_model(&self, object: &SceneObject) -> bool {
        object.id == self.id && Arc::ptr_eq(&object.mesh, &self.mesh)
    }
}

/// How far a grid whose near corner is at `min` and which spans `span` has to move to
/// stand on a plate `plate` across. Zero for a grid already on it, and the move that puts
/// its near corner in the plate's own corner for one too big to fit.
pub(super) fn onto_plate(min: Vec2, span: Vec2, plate: Vec2) -> Vec2 {
    let axis = |min: Scalar, span: Scalar, plate: Scalar| {
        if span >= plate {
            return -min;
        }
        (0.0 as Scalar).clamp(-min, plate - span - min)
    };
    Vec2::new(axis(min.x, span.x, plate.x), axis(min.y, span.y, plate.y))
}

/// The scale that flips `axis` and leaves the other two alone.
pub(super) fn flip(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(-1.0, 1.0, 1.0),
        Axis::Y => Vec3::new(1.0, -1.0, 1.0),
        Axis::Z => Vec3::new(1.0, 1.0, -1.0),
    }
}

impl SceneObject {
    /// The mesh the model's cuts are taken out of: its shell once hollowed.
    pub fn uncut(&self) -> &Arc<Mesh> {
        self.hollow.shell().unwrap_or(&self.mesh)
    }

    /// What its printed supports are built from: its groups, its cuts and where it stands.
    pub fn supports_under(&self) -> SupportsUnder<'_> {
        SupportsUnder {
            groups: self.supports.drawn(),
            cuts: self.hollow.cut_bodies(),
            transform: self.transform,
        }
    }

    /// The support groups as they print: drilled by the model's cuts where that has been
    /// done, as they stand otherwise.
    pub fn supports_shown(&self) -> &[core_supports::Shaded] {
        self.printed_supports.shown(self.supports_under())
    }

    /// The support groups as they print and their hierarchies, once built for them.
    pub fn supports_built(&self) -> Option<(&[core_supports::Shaded], &[Arc<Bvh>])> {
        self.printed_supports.built(self.supports_under())
    }

    /// The mesh the model prints as: its cuts taken out exactly where they have been, its
    /// shell or its own mesh while they are cut by the winding rule.
    pub fn printed(&self) -> &Arc<Mesh> {
        let uncut = self.uncut();
        self.hollow
            .drilled(uncut)
            .map_or(uncut, |drilled| &drilled.mesh)
    }

    /// Bounds of the placed model, in plate coordinates: of its vertices as placed, not
    /// of its own box turned, which grows by up to √3 at an angle. Cached per transform.
    pub fn world_bounds(&self) -> Option<Aabb> {
        self.bounds.get(&self.mesh, self.transform)
    }

    /// A direction in the model's own space, as the placement turns it. A scale or a
    /// stretch bends a normal the other way from a point, hence the inverse transpose.
    pub fn world_normal(&self, local: Vec3) -> Vec3 {
        (self.transform.linear().inverse().transpose() * local).normalize_or_zero()
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
    pub fn reshape(&mut self, shaped: Shaped) {
        let Shaped {
            mesh,
            bvh,
            center_of_mass,
            diagnostics,
        } = shaped;
        self.bvh = bvh;
        self.center_of_mass = center_of_mass;
        self.summary = ImportSummary {
            diagnostics,
            ..self.summary.clone()
        };
        self.supports = ModelSupports::default();
        self.hollow = ModelHollow::default();
        self.traps.clear();
        self.mapped = None;
        self.relief_painted.clear();
        self.mesh = mesh;
    }

    /// Places the model under a pivot the gizmo has dragged, turned or stretched, so
    /// that its centre of mass lands back on that pivot.
    pub fn settle(&mut self, pivot: Transform) {
        self.transform = Transform {
            translation: pivot.translation - pivot.linear() * self.center_of_mass,
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
        let turn = Quat::from_rotation_arc(normal, Vec3::NEG_Z);
        self.settle(self.pivot().turned(turn));
        self.stand_on_plate();
        true
    }

    /// Slides the model along the plate only as far as stands it inside a plate `plate_mm`
    /// across, or into the plate's near corner where it is too big to.
    pub fn slide_onto_plate(&mut self, plate_mm: Vec2) {
        if let Some(bounds) = self.world_bounds() {
            let min = bounds.mins.truncate();
            let onto = onto_plate(min, bounds.maxs.truncate() - min, plate_mm);
            self.transform.translation += onto.extend(0.0);
        }
    }

    /// How high the model's lowest point stands over its raft, millimetres.
    pub fn lift(&self) -> Option<f32> {
        let bounds = self.world_bounds()?;
        Some(bounds.mins.z - self.supports.raft_lift_mm())
    }

    /// Stands the model `lift_mm` over its raft and keeps that as the lift asked for, or
    /// forgets it at nought, where auto-orient goes back to the profile's.
    pub fn lift_to(&mut self, lift_mm: f32) {
        if let Some(now_mm) = self.lift() {
            self.transform.translation.z += lift_mm - now_mm;
        }
        self.lift_mm = (lift_mm > 0.0).then_some(lift_mm);
    }

    /// Moves the model straight down, or up, until its lowest point touches the plate.
    /// Nothing lifts it there, so no raft's lift is left to be taken back off it later.
    pub fn stand_on_plate(&mut self) {
        if let Some(bounds) = self.world_bounds() {
            self.transform.translation += core_geometry::drop_to_plate(&bounds);
            self.supports.set_raft_lift_mm(0.0);
        }
    }
}
