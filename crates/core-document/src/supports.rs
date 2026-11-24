//! Supports stood under a model by a plate group, a shipped preset or `auto`
//! (`docs/design/document.md`).

use std::sync::Arc;

use core_edit::{AUTO, Edit, ObjectId};
use core_engine::project::{Group, ModelMeshes, ObjectState, SupportState, inner_faces};
use core_engine::{NeedsAsked, ShownAt, SupportPlan, analysis_slicing, needs_on_both, shown_on};
use core_geometry::{Bvh, Mat4, Mesh, Transform, Vec3, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine};
use core_supports::{Blocked, ModelSupports, Need, Shown, SupportTree, WallSide, low_points};
use core_volume::Cancel;
use printer_profiles::SupportProfile;

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;

/// What a `Supports` edit asks of an object.
pub(crate) struct SupportsAsked<'a> {
    pub profile: Option<&'a str>,
    pub at_mm: &'a [Vec3],
    pub only_at: bool,
    pub cavity: bool,
    pub shown_at: &'a [ShownAt],
    pub clean_surface: bool,
}

impl<'a> SupportsAsked<'a> {
    /// What `edit` asks, or `None` for an edit that is not `Supports`.
    pub(crate) fn of(edit: &'a Edit) -> Option<Self> {
        match edit {
            Edit::Supports {
                profile,
                at_mm,
                only_at,
                cavity,
                shown_at,
                clean_surface,
                ..
            } => Some(Self {
                profile: profile.as_deref(),
                at_mm,
                only_at: *only_at,
                cavity: *cavity,
                shown_at,
                clean_surface: *clean_surface,
            }),
            _ => None,
        }
    }
}

/// Stands supports under object `id`, by the preset's contacts and the points asked for.
pub(crate) fn apply_supports(
    document: &mut Document,
    id: ObjectId,
    asked: &SupportsAsked<'_>,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let transform = state.transform;
    let into_model = transform
        .inverse_matrix()
        .ok_or(ApplyError::BadTransform(id))?;
    let plate = document.support_state();
    let ((mut groups, group), plan) = group_and_plan(plate, state, asked, id)?;
    let table = table_of(state, &groups, plate.raft);
    let surface = meshes.printed();
    let bvh = Arc::new(Bvh::build(surface));
    let mut supports = kept_supports(state, meshes, &bvh, &table);
    let placed = transform_mesh(surface, transform);
    let placed_bvh = Bvh::build(&placed);
    let on = PlacedModel {
        state,
        meshes,
        placed: (&placed, &placed_bvh),
        wall: wall_outside(state, meshes, &bvh, into_model),
        read_painted: asked.clean_surface || !asked.shown_at.is_empty(),
    };
    let stood = stand_asked(&mut supports, &on, asked.at_mm, (transform, group), id)?;
    let needs = needs_asked(&on, &supports, (asked, &plan), (id, cancel))?;
    stand_needs(
        &mut supports,
        &mut groups,
        &plan,
        (&needs, (transform, group)),
    );
    if cancel.asked() {
        return Err(ApplyError::Cancelled);
    }
    let table = table_of(on.state, &groups, plate.raft);
    let grown = grown_trees(&mut supports, &on, (&bvh, &table));
    let report = supported(&supports, (stood + needs.len(), grown.len()), transform);
    document.set_supports(id, (groups, group), (supports.points().to_vec(), grown));
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report,
    })
}

/// The plate's groups with the one `asked` builds in, its number, and the plan it builds to.
fn group_and_plan(
    plate: &SupportState,
    state: &ObjectState,
    asked: &SupportsAsked<'_>,
    id: ObjectId,
) -> Result<((Vec<Group>, u16), SupportPlan), ApplyError> {
    let named = asked.profile.filter(|name| *name != AUTO);
    let (groups, group) = group_for(&plate.groups, plate.active, named)?;
    let table = table_of(state, &groups, plate.raft);
    let profile = table
        .get(usize::from(group))
        .ok_or(ApplyError::NoGroup(id))?;
    let plan = plan_for(asked, (plate.sizing.as_ref(), &groups), profile)?;
    Ok(((groups, group), plan))
}

/// What supports `placed` points and grew `trees` under an object standing at `transform`.
fn supported(
    supports: &ModelSupports,
    (placed, trees): (usize, usize),
    transform: Transform,
) -> Report {
    Report::Supported {
        placed,
        trees,
        contacts_mm: supports.contacts(transform),
        standing: supports.standing(),
        unheld: unheld(supports, transform),
    }
}

/// The contacts of `supports` no tree carries, plate mm, each with why.
fn unheld(supports: &ModelSupports, transform: Transform) -> Vec<(Vec3, core_supports::Unstood)> {
    let matrix = transform.to_matrix();
    (supports.lost().iter())
        .filter_map(|lost| {
            let point = supports.points().get(lost.point)?;
            Some((matrix.transform_point3(point.contact), lost.why))
        })
        .collect()
}

/// Stands a support in `group` at the surface point nearest each of `at_mm`, which the run
/// then takes as seeds; answers how many.
fn stand_asked(
    supports: &mut ModelSupports,
    on: &PlacedModel<'_>,
    at_mm: &[Vec3],
    (transform, group): (Transform, u16),
    id: ObjectId,
) -> Result<usize, ApplyError> {
    for at in at_mm {
        let closest = on.placed.1.closest(*at).ok_or(ApplyError::NoSurface(id))?;
        supports.add(closest.point, transform, group);
    }
    Ok(at_mm.len())
}

/// The needs the plan finds, plate mm, unless `only_at`, with the points asked standing.
fn needs_asked(
    on: &PlacedModel<'_>,
    supports: &ModelSupports,
    (asked, plan): (&SupportsAsked<'_>, &SupportPlan),
    (id, cancel): (ObjectId, Cancel<'_>),
) -> Result<Vec<Need>, ApplyError> {
    if asked.only_at {
        return Ok(Vec::new());
    }
    let seeds = supports.contacts(on.state.transform);
    found(on, &seeds, plan, (id, cancel))
}

/// Grows the trees of `supports` against the printed surface, whose hierarchy is `bvh`, and
/// answers them in the object's own space.
fn grown_trees(
    supports: &mut ModelSupports,
    on: &PlacedModel<'_>,
    (bvh, table): (&Bvh, &[SupportProfile]),
) -> Vec<SupportTree> {
    let transform = on.state.transform;
    if let Some(growth) = supports.growth(transform, table) {
        let surface = on.meshes.printed();
        supports.take_grown(growth.grow(&on.meshes.source, surface, bvh));
    }
    supports.grown(transform)
}

/// One object as support placement reads it: its state and meshes, its printed surface on the
/// plate with that surface's hierarchy, and the wall side held up.
struct PlacedModel<'a> {
    state: &'a ObjectState,
    meshes: &'a ModelMeshes,
    placed: (&'a Mesh, &'a Bvh),
    wall: Option<WallSide>,
    read_painted: bool,
}

/// Where the model needs holding up by `plan`, plate mm, with `seeds` standing already: its
/// outside, then its cavity where the plan asks, held up with the outside's tips standing.
fn found(
    on: &PlacedModel<'_>,
    seeds: &[Vec3],
    plan: &SupportPlan,
    (id, cancel): (ObjectId, Cancel<'_>),
) -> Result<Vec<Need>, ApplyError> {
    let failed = |reason: String| ApplyError::Failed { object: id, reason };
    let (placed, bvh) = on.placed;
    let transform = on.state.transform;
    let settings = analysis_slicing();
    let stack = PlaneSliceEngine
        .slice(placed, &settings)
        .map_err(|error| failed(error.to_string()))?;
    let source = &on.meshes.source;
    let painted = on
        .read_painted
        .then(|| Shown::new(source, &on.state.shown_painted, transform));
    let shown = painted
        .flatten()
        .or_else(|| shown_on(placed, bvh, &plan.shown_at));
    let lows = low_points(on.meshes.printed(), transform);
    let asked = |inside: bool| NeedsAsked {
        stack: &stack,
        surface: placed,
        bvh,
        layer_height_mm: settings.layer_height,
        plan,
        lows: &lows,
        painted: Blocked::new(source, &on.state.supports.blocked, transform),
        shown: shown.as_ref(),
        seeds,
        side: on.wall.clone().map(|wall| WallSide { inside, ..wall }),
    };
    let mut tick = |_: usize| !cancel.asked();
    let cavity = plan.cavity && on.wall.is_some();
    let needs = needs_on_both(&asked, cavity, seeds, &mut tick)
        .map_err(|error| failed(error.to_string()))?;
    if cancel.asked() {
        return Err(ApplyError::Cancelled);
    }
    Ok(needs)
}

/// The outside of a hollow object's wall, where its shell has a cavity.
fn wall_outside(
    state: &ObjectState,
    meshes: &ModelMeshes,
    bvh: &Arc<Bvh>,
    into_model: Mat4,
) -> Option<WallSide> {
    let shell = meshes.shell.as_ref()?;
    let cavity = &state.hollow.built.as_ref()?.cavity_faces;
    (!cavity.is_empty()).then(|| WallSide {
        bvh: Arc::clone(bvh),
        inner: inner_faces(cavity, shell.faces.len()),
        into_model,
        inside: false,
    })
}

mod carried;
mod groups;
mod merge;
mod standing;

pub(crate) use carried::stand_at;
use groups::{group_for, plan_for, stand_needs, table_of};
pub(crate) use merge::apply_merge;
pub use merge::support_near;
pub use standing::standing_meshes;
pub(crate) use standing::{kept_supports, orient_plan_of, raft_under};

#[cfg(test)]
mod tests;
