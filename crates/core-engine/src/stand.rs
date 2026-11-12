use std::ops::Range;
use std::sync::Arc;

use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Sliced};
use core_supports::{
    Blocked, Lost, Need, Placed, Profiles, Reason, SupportPoint, WallSide, columns,
    generate_supports, grow, lost, low_points, mesh_trees,
};
use core_volume::Drilled;
use printer_profiles::{AUTO_SIZES, AutoProfile, Catalogue, ProfileError, SupportProfile};

use crate::error::EngineError;
use crate::strengthen::{Underside, strengthen};

mod lift;
mod merge;

use lift::lift_of;
pub use lift::{Lift, planned_lift};
pub(crate) use merge::merge_refusal;
use merge::merged_near;

/// What a run builds supports to: one profile, or the shapes an automatic profile sizes
/// each support into by what it holds (ADR 0325).
#[derive(Debug, Clone, PartialEq)]
pub enum SupportShapes {
    One(Box<SupportProfile>),
    Sized {
        auto: Box<AutoProfile>,
        /// The shapes it names, lightest first, then its steadying one.
        shapes: Box<[SupportProfile; AUTO_SIZES + 1]>,
    },
}

/// What a run stands under a model: the shapes it builds to, and whether it holds up a
/// hollowed model's cavity as well as its outside, as the window's two runs do (ADR 0332).
#[derive(Debug, Clone, PartialEq)]
pub struct SupportPlan {
    pub shapes: SupportShapes,
    pub cavity: bool,
    /// Each a merge, as the window's Merge into one: points, plate mm, naming the supports by
    /// their contacts once they stand (ADR 0440).
    pub merges: Vec<Vec<Vec3>>,
}

impl SupportPlan {
    /// Supports built to `profile` alone, outside only.
    pub fn of(profile: SupportProfile) -> Self {
        Self {
            shapes: SupportShapes::One(Box::new(profile)),
            cavity: false,
            merges: Vec::new(),
        }
    }

    /// Supports sized by `auto` into the shapes `catalogue` has for it, outside only.
    pub fn sized(auto: AutoProfile, catalogue: &Catalogue) -> Result<Self, ProfileError> {
        let shapes = auto.shapes_in(catalogue)?;
        Ok(Self {
            shapes: SupportShapes::Sized {
                auto: Box::new(auto),
                shapes: Box::new(shapes),
            },
            cavity: false,
            merges: Vec::new(),
        })
    }

    /// The profile placement runs with: where supports go, how close, how far a model is
    /// lifted and what raft it stands on.
    pub fn placing(&self) -> SupportProfile {
        match &self.shapes {
            SupportShapes::One(profile) => profile.as_ref().clone(),
            SupportShapes::Sized { auto, shapes } => auto.placing_with(&shapes[0]),
        }
    }

    /// The profiles supports are built to, by group.
    pub fn table(&self) -> Vec<SupportProfile> {
        match &self.shapes {
            SupportShapes::One(profile) => vec![profile.as_ref().clone()],
            SupportShapes::Sized { shapes, .. } => shapes.to_vec(),
        }
    }

    /// Every profile of [`Self::table`], to set what all of them share, a raft or a lift.
    pub fn profiles_mut(&mut self) -> &mut [SupportProfile] {
        match &mut self.shapes {
            SupportShapes::One(profile) => std::slice::from_mut(profile.as_mut()),
            SupportShapes::Sized { shapes, .. } => shapes.as_mut_slice(),
        }
    }

    /// The automatic profile supports are sized by, if any.
    pub fn sizing(&self) -> Option<&AutoProfile> {
        match &self.shapes {
            SupportShapes::One(_) => None,
            SupportShapes::Sized { auto, .. } => Some(auto),
        }
    }

    /// The group of [`Self::table`] a support holding `need` is built to.
    pub fn group_of(&self, need: &Need) -> u16 {
        self.sizing()
            .map_or(0, |auto| u16::try_from(need.size_in(auto)).unwrap_or(0))
    }
}

/// What automatic placement put under one model.
#[derive(Debug, Clone, PartialEq)]
pub struct Stood {
    /// How far the model was stood off the plate to make room, millimetres; zero for one
    /// that already stood clear of it.
    pub lifted_mm: Scalar,
    /// Where each contact touches the model, plate coordinates, millimetres.
    pub contacts: Vec<Vec3>,
    /// The way the surface under each contact faces, unit, or zero where none was found.
    pub facing: Vec<Vec3>,
    /// How many contacts each profile of the plan's table was given, by group.
    pub sized: Vec<usize>,
    /// Contacts a column was stood under.
    pub standing: usize,
    /// The contacts no column could stand under, each an index into `contacts` and why.
    pub unheld: Vec<Lost>,
    pub trees: usize,
    /// The supports themselves, in plate coordinates.
    pub supports: Mesh,
}

/// A model to stand supports under, in plate coordinates: its mesh, which of its faces
/// bound its cavity once hollowed, and the bodies its holes cut out of it, which ride along
/// as it is lifted.
pub struct ToStand<'a> {
    pub mesh: &'a mut Mesh,
    pub cavity: Option<Range<usize>>,
    /// Whether the faces before its cavity are the model byte for byte (ADR 0412).
    pub on_its_model: bool,
    pub cuts: &'a mut Option<Mesh>,
    /// Whether the cuts are taken out exactly, as the window takes them out, or left to the
    /// winding rule: a search over poses cannot afford the engine on every one.
    pub drill_exactly: bool,
}

/// Stands supports under the model, lifted first by the plan's lift and raft, against what
/// it prints as, and answers with them: `docs/design/supports.md`.
pub fn stand_under(
    model: ToStand<'_>,
    plan: &SupportPlan,
    layer_height_mm: Scalar,
) -> Result<Stood, EngineError> {
    let lift_mm = plan.placing().lift_mm();
    let (lifted_mm, surface, inner) = touched(model, lift_mm)?;
    let bvh = Arc::new(Bvh::build(&surface));
    let needs = needs_under(&surface, &bvh, inner, plan, (layer_height_mm, true))?;
    grown(lifted_mm, &surface, &bvh, (&needs, &plan.merges), plan)
}

/// A support under each of `contacts` alone, plate mm on the model where it stands, nothing
/// lifted and nothing else placed: Check's Support every island, added beside what stands.
pub fn stand_at(
    model: ToStand<'_>,
    plan: &SupportPlan,
    contacts: &[Vec3],
) -> Result<Stood, EngineError> {
    let (lifted_mm, surface, _) = touched(model, 0.0)?;
    let bvh = Arc::new(Bvh::build(&surface));
    // An island of no area, which an automatic profile builds to its lightest shape.
    let needs: Vec<Need> = contacts
        .iter()
        .map(|contact| Need::new(*contact, Reason::POINT))
        .collect();
    grown(lifted_mm, &surface, &bvh, (&needs, &[]), plan)
}

/// The same for a model left standing on the plate: supports go only under what hangs
/// over it, and the plate holds the rest. A model that needs none gets none, and one that
/// needs some stands on the raft's top, as the window's raft stands it.
pub fn stand_seated(
    model: ToStand<'_>,
    plan: &SupportPlan,
    layer_height_mm: Scalar,
) -> Result<Stood, EngineError> {
    let raft = plan.placing().raft;
    let ToStand {
        mesh,
        cavity,
        on_its_model,
        cuts,
        drill_exactly,
    } = model;
    let seated = ToStand {
        mesh: &mut *mesh,
        cavity,
        on_its_model,
        cuts: &mut *cuts,
        drill_exactly,
    };
    let (mut lifted_mm, mut surface, inner) = touched(seated, 0.0)?;
    let mut bvh = Arc::new(Bvh::build(&surface));
    let mut needs = needs_under(&surface, &bvh, inner, plan, (layer_height_mm, true))?;
    if raft.enabled && !needs.is_empty() {
        let up = Vec3::Z * raft.thickness_mm;
        let moved = Transform::from_translation(up);
        *mesh = transform_mesh(mesh, moved);
        if let Some(cuts) = cuts.as_mut() {
            *cuts = transform_mesh(cuts, moved);
        }
        surface = transform_mesh(&surface, moved);
        bvh = Arc::new(Bvh::build(&surface));
        for need in &mut needs {
            need.contact += up;
        }
        lifted_mm += up.z;
    }
    grown(lifted_mm, &surface, &bvh, (&needs, &plan.merges), plan)
}

/// The model lifted `lift_mm` clear of the plate with its cuts, then how far it moved, the
/// surface it prints as and which faces of that lie inside it.
fn touched(
    model: ToStand<'_>,
    lift_mm: Scalar,
) -> Result<(Scalar, Mesh, Option<Range<usize>>), EngineError> {
    let lifted_mm = lift(model.mesh, model.cuts, lift_mm)?;
    let cuts = model.cuts.as_ref();
    let cavity = (model.cavity, model.on_its_model);
    let (surface, inner) = printed(model.mesh, cavity, cuts, model.drill_exactly);
    Ok((lifted_mm, surface, inner))
}

/// What `mesh` prints as: its cuts taken out exactly where asked, as the window takes them
/// out, else appended for the winding rule, as they are where the engine refuses them
/// (ADR 0322); and its faces on the cavity and the lattice in it.
fn printed(
    mesh: &Mesh,
    (cavity, on_its_model): (Option<Range<usize>>, bool),
    cuts: Option<&Mesh>,
    exactly: bool,
) -> (Mesh, Option<Range<usize>>) {
    let inside = |faces: usize, cavity: Range<usize>| cavity.start..faces.max(cavity.end);
    let Some(cuts) = cuts else {
        return (
            mesh.clone(),
            cavity.map(|cavity| inside(mesh.faces.len(), cavity)),
        );
    };
    let drill = || Drilled::of_printed(mesh, cavity.clone(), on_its_model, cuts);
    if let Some(Ok(drilled)) = exactly.then(drill) {
        let surface = Arc::unwrap_or_clone(drilled.mesh);
        let inner = drilled
            .cavity
            .map(|cavity| inside(surface.faces.len(), cavity));
        return (surface, inner);
    }
    let mut surface = mesh.clone();
    append(&mut surface, cuts);
    (
        surface,
        cavity.map(|cavity| inside(mesh.faces.len(), cavity)),
    )
}

/// Columns stood on `surface` under each of `needs`, built to the group its size names,
/// and what they came to.
fn grown(
    lifted_mm: Scalar,
    surface: &Mesh,
    bvh: &Bvh,
    (needs, merges): (&[Need], &[Vec<Vec3>]),
    plan: &SupportPlan,
) -> Result<Stood, EngineError> {
    let points: Vec<SupportPoint> = needs
        .iter()
        .map(|need| SupportPoint {
            group: plan.group_of(need),
            ..SupportPoint::new(need.contact)
        })
        .collect();
    let contacts: Vec<Vec3> = points.iter().map(|point| point.contact).collect();
    let table = plan.table();
    let mut sized = vec![0; table.len()];
    for point in &points {
        if let Some(count) = sized.get_mut(usize::from(point.group)) {
            *count += 1;
        }
    }

    let placed = Placed::new(surface, bvh, Transform::default());
    let profiles = Profiles::new(&table).ok_or(EngineError::EmptyPlate)?;
    let columns = columns(&points, &placed, profiles);
    let trees = grow(&columns, &placed, profiles);
    let trees = merged_near(trees, (merges, &contacts), &columns, &placed, profiles)?;
    let facing = facing(surface, bvh, &contacts);
    Ok(Stood {
        lifted_mm,
        contacts,
        facing,
        sized,
        standing: trees
            .iter()
            .map(core_supports::SupportTree::tip_count)
            .sum(),
        unheld: lost(&points, &trees, &placed, profiles),
        trees: trees.len(),
        supports: mesh_trees(&trees, &placed, profiles),
    })
}

/// The way the surface of `drilled` under each contact faces, unit, or zero where the
/// contact finds none.
fn facing(drilled: &Mesh, bvh: &Bvh, contacts: &[Vec3]) -> Vec<Vec3> {
    contacts
        .iter()
        .map(|contact| {
            bvh.closest(*contact)
                .and_then(|hit| drilled.triangle(hit.face))
                .map_or(Vec3::ZERO, |triangle| {
                    triangle.normal_unnormalized().normalize_or_zero()
                })
        })
        .collect()
}

/// Where `surface`, standing where it is, needs holding up: what automatic placement would
/// touch from outside its wall, then inside its cavity where the plan asks, each contact
/// with what it holds, strengthened where `strengthening` asks.
fn needs_under(
    surface: &Mesh,
    bvh: &Arc<Bvh>,
    inner: Option<Range<usize>>,
    plan: &SupportPlan,
    (layer_height_mm, strengthening): (Scalar, bool),
) -> Result<Vec<Need>, EngineError> {
    let stack = PlaneSliceEngine
        .slice(
            surface,
            &SliceSettings {
                layer_height: layer_height_mm,
                ..SliceSettings::default()
            },
        )
        .map_err(EngineError::Supports)?;
    let run = Run {
        stack: &stack,
        surface,
        bvh,
        layer_height_mm,
        plan,
        lows: low_points(surface, Transform::default()),
    };
    let inner = inner.filter(|inner| !inner.is_empty());
    let side = |inside| {
        inner.clone().map(|inner| WallSide {
            bvh: Arc::clone(bvh),
            inner,
            into_model: core_geometry::Mat4::IDENTITY,
            inside,
        })
    };
    let mut needs = run.place(side(false));
    let mut inside = if plan.cavity && inner.is_some() {
        run.place(side(true))
    } else {
        Vec::new()
    };
    if strengthening {
        // Both sides hold the one shell, so each is read with the other's tips standing.
        let contacts = |needs: &[Need]| needs.iter().map(|need| need.contact).collect::<Vec<_>>();
        run.strengthen(side(false), &mut needs, &contacts(&inside))?;
        if !inside.is_empty() {
            run.strengthen(side(true), &mut inside, &contacts(&needs))?;
        }
    }
    needs.extend(inside);
    Ok(needs)
}

/// One cut stack and what placement on it is asked with.
struct Run<'a> {
    stack: &'a Sliced,
    surface: &'a Mesh,
    bvh: &'a Bvh,
    layer_height_mm: Scalar,
    plan: &'a SupportPlan,
    lows: Vec<Vec3>,
}

impl Run<'_> {
    /// What automatic placement touches, kept to `side` of the wall where there is one.
    fn place(&self, side: Option<WallSide>) -> Vec<Need> {
        let blocked = Blocked::keeping(None, side.clone());
        let mut needs = generate_supports(
            self.stack,
            self.layer_height_mm,
            &self.plan.placing(),
            self.plan.sizing(),
            &[],
            &self.lows,
            blocked.as_ref(),
            None,
            &mut |_| true,
        );
        if let Some(side) = side {
            needs.retain(|need| side.keeps(need.contact));
        }
        needs
    }

    /// Strengthens `needs`, placed on `side` of the wall, with `held` standing beside them.
    fn strengthen(
        &self,
        side: Option<WallSide>,
        needs: &mut Vec<Need>,
        held: &[Vec3],
    ) -> Result<(), EngineError> {
        let blocked = Blocked::keeping(None, side);
        let underside = Underside {
            stack: self.stack,
            layer_mm: self.layer_height_mm,
            surface: self.surface,
            bvh: self.bvh,
            blocked: blocked.as_ref(),
        };
        strengthen(underside, needs, held, self.plan, &mut || true).map(|_| ())
    }
}

/// Stands `mesh` and what is cut out of it `lift_mm` clear of the plate, and answers how
/// far they moved, by [`lift_of`].
fn lift(mesh: &mut Mesh, cuts: &mut Option<Mesh>, lift_mm: Scalar) -> Result<Scalar, EngineError> {
    let bounds = mesh.aabb().ok_or(EngineError::EmptyPlate)?;
    let up = lift_of(&bounds, lift_mm);
    if up <= 0.0 {
        return Ok(0.0);
    }
    let moved = Transform::from_translation(Vec3::Z * up);
    *mesh = transform_mesh(mesh, moved);
    if let Some(cuts) = cuts.as_mut() {
        *cuts = transform_mesh(cuts, moved);
    }
    Ok(up)
}

/// Adds `other`'s geometry to `mesh`, keeping each face pointing at its own vertices.
fn append(mesh: &mut Mesh, other: &Mesh) {
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&other.vertices);
    mesh.faces.extend(
        other
            .faces
            .iter()
            .map(|face| [face[0] + offset, face[1] + offset, face[2] + offset]),
    );
}

#[cfg(test)]
mod tests;
