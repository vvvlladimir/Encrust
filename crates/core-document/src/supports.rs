//! Supports stood under a model by a plate group, a shipped preset or `auto`
//! (`docs/design/document.md`).

use std::sync::Arc;

use core_edit::{AUTO, Edit, ObjectId};
use core_engine::project::{AutoSizing, Group, ModelMeshes, ObjectState, inner_faces, supports_of};
use core_engine::{
    NeedsAsked, ShownAt, SupportPlan, analysis_slicing, needs_on_both, plan_of, shown_on,
};
use core_geometry::{Bvh, Mat4, Mesh, Transform, Vec3, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine};
use core_supports::{Blocked, ModelSupports, Need, Shown, SupportTree, WallSide, low_points};
use core_volume::Cancel;
use printer_profiles::{Catalogue, Raft, SupportProfile};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::orient::orient_plan;

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
    let named = asked.profile.filter(|name| *name != AUTO);
    let (mut groups, group) = group_for(&plate.groups, plate.active, named)?;
    let table = table_of(state, &groups, plate.raft);
    let plan = plan_for(
        asked,
        (plate.sizing.as_ref(), &groups),
        table
            .get(usize::from(group))
            .ok_or(ApplyError::NoGroup(id))?,
    )?;

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
    let report = Report::Supported {
        placed: stood + needs.len(),
        trees: grown.len(),
        contacts_mm: supports.contacts(transform),
        standing: supports.standing(),
        unheld: unheld(&supports, transform),
    };
    document.set_supports(id, (groups, group), (supports.points().to_vec(), grown));
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report,
    })
}

/// The supports `state` keeps, restored on the printed surface of `meshes` where it stands.
pub(crate) fn kept_supports(
    state: &ObjectState,
    meshes: &ModelMeshes,
    bvh: &Bvh,
    table: &[SupportProfile],
) -> ModelSupports {
    let (source, drains) = (&meshes.source, &state.hollow.drains);
    supports_of(
        &state.supports,
        drains,
        source,
        meshes.printed(),
        bvh,
        state.transform,
        table,
    )
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

/// The raft under object `id` where it stands, plate mm; none where it stands on no supports.
pub(crate) fn raft_under(document: &Document, id: ObjectId) -> Option<Arc<Mesh>> {
    let (state, meshes) = document.object(id)?;
    let plate = document.support_state();
    let table = table_of(state, &plate.groups, plate.raft);
    let bvh = Bvh::build(meshes.printed());
    kept_supports(state, meshes, &bvh, &table).raft().cloned()
}

/// The meshes of the supports under object `id` where it stands, plate mm, columns and raft.
pub fn standing_meshes(document: &Document, id: ObjectId) -> Vec<Arc<Mesh>> {
    let Some((state, meshes)) = document.object(id) else {
        return Vec::new();
    };
    if state.supports.points.is_empty() {
        return Vec::new();
    }
    let plate = document.support_state();
    let table = table_of(state, &plate.groups, plate.raft);
    let bvh = Bvh::build(meshes.printed());
    kept_supports(state, meshes, &bvh, &table)
        .every_mesh()
        .cloned()
        .collect()
}

/// The plan an orientation of `state` weighs: the plate's kept sizing, sized, where it is on,
/// else the active group's profile; none where the plate has no group.
pub(crate) fn orient_plan_of(
    document: &Document,
    state: &ObjectState,
) -> Result<Option<SupportPlan>, ApplyError> {
    let plate = document.support_state();
    let table = table_of(state, &plate.groups, plate.raft);
    let Some(active) = table.get(usize::from(plate.active)).or(table.first()) else {
        return Ok(None);
    };
    let kept = plate.sizing.as_ref().filter(|sizing| sizing.on);
    let sized = kept
        .map(|sizing| sized_plan((Some(sizing), &plate.groups), active))
        .transpose()?;
    Ok(Some(orient_plan(sized, active, active.raft)))
}

/// The profiles of `groups` as `state` is built to them: its own where it keeps any, on its raft.
fn table_of(state: &ObjectState, groups: &[Group], raft: Option<Raft>) -> Vec<SupportProfile> {
    let profiles: Vec<SupportProfile> = groups.iter().map(|g| g.profile.clone()).collect();
    state.support_table(&profiles, raft)
}

/// Adds each of `needs` to `supports` in the group of its size, which `groups` gains where
/// the plan's shape has none yet; `group` takes a plan of one profile.
fn stand_needs(
    supports: &mut ModelSupports,
    groups: &mut Vec<Group>,
    plan: &SupportPlan,
    (needs, (transform, group)): (&[Need], (Transform, u16)),
) {
    let by_size = group_by_size(groups, plan, needs, group);
    for need in needs {
        let size = usize::from(plan.group_of(need));
        let at = by_size.get(size).copied().unwrap_or(group);
        supports.add(need.contact, transform, at);
    }
}

/// What the run builds to: the plate's own automatic profile, else the catalogue's first, for
/// `auto`, its shapes the plate's groups of their names; the group's `profile` otherwise.
fn plan_for(
    asked: &SupportsAsked<'_>,
    (sizing, groups): (Option<&AutoSizing>, &[Group]),
    profile: &SupportProfile,
) -> Result<SupportPlan, ApplyError> {
    let mut plan = match asked.profile {
        Some(AUTO) => sized_plan((sizing, groups), profile)?,
        _ => plan_of(profile, None),
    };
    plan.cavity = asked.cavity;
    plan.shown_at = asked.shown_at.to_vec();
    Ok(plan)
}

/// Supports sized by `sizing`'s profile where the plate keeps one, else by the shipped
/// catalogue's first; a shape is the plate's group of its name where there is one.
fn sized_plan(
    (sizing, groups): (Option<&AutoSizing>, &[Group]),
    profile: &SupportProfile,
) -> Result<SupportPlan, ApplyError> {
    let catalogue = Catalogue::bundled().map_err(|_| ApplyError::NoProfile(AUTO.into()))?;
    let kept = sizing.map(|sizing| sizing.profile.clone());
    let auto = kept
        .or_else(|| catalogue.autos().next().map(|entry| entry.profile.clone()))
        .ok_or_else(|| ApplyError::NoProfile(AUTO.into()))?;
    let mut plan =
        SupportPlan::sized(auto, &catalogue).map_err(|_| ApplyError::NoProfile(AUTO.into()))?;
    for shape in plan.profiles_mut() {
        if let Some(group) = groups.iter().find(|g| g.profile.name == shape.name) {
            *shape = group.profile.clone();
        }
        shape.raft = profile.raft;
    }
    Ok(plan)
}

/// The group each size of `plan`'s table is built in: `group` alone for one profile, else each
/// shape that holds a need, in the plate's group of its name or a new one put in `groups`.
fn group_by_size(
    groups: &mut Vec<Group>,
    plan: &SupportPlan,
    needs: &[Need],
    group: u16,
) -> Vec<u16> {
    if plan.sizing().is_none() {
        return vec![group];
    }
    let table = plan.table();
    let mut held = vec![false; table.len()];
    for need in needs {
        if let Some(held) = held.get_mut(usize::from(plan.group_of(need))) {
            *held = true;
        }
    }
    let mut by_size = vec![group; table.len()];
    for ((at, shape), held) in by_size.iter_mut().zip(table).zip(held) {
        if !held {
            continue;
        }
        let found = groups.iter().position(|g| g.profile.name == shape.name);
        *at = found.unwrap_or_else(|| {
            groups.push(Group {
                name: shape.name.clone(),
                profile: shape,
            });
            groups.len() - 1
        }) as u16;
    }
    by_size
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

/// The plate's groups with the one `profile` names, and its number: the active group where
/// none is named, a shipped preset put in as a group where the plate has none by its name.
fn group_for(
    groups: &[Group],
    active: u16,
    profile: Option<&str>,
) -> Result<(Vec<Group>, u16), ApplyError> {
    let mut groups = groups.to_vec();
    let Some(name) = profile else {
        let active = if usize::from(active) < groups.len() {
            active
        } else {
            0
        };
        return Ok((groups, active));
    };
    if let Some(at) = groups.iter().position(|group| group.profile.name == name) {
        return Ok((groups, at as u16));
    }
    let shipped = SupportProfile::shipped(name).map_err(|_| ApplyError::NoProfile(name.into()))?;
    groups.push(Group {
        name: shipped.name.clone(),
        profile: shipped,
    });
    let at = (groups.len() - 1) as u16;
    Ok((groups, at))
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
mod merge;

pub(crate) use carried::stand_at;
pub(crate) use merge::apply_merge;
pub use merge::support_near;

#[cfg(test)]
mod tests;
