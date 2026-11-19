//! Supports stood under a model by the shipped preset it is asked for (`docs/design/document.md`).

use std::sync::Arc;

use core_edit::{Edit, ObjectId};
use core_engine::project::{Group, ModelMeshes, ObjectState, inner_faces, supports_of};
use core_engine::{NeedsAsked, needs_on, plan_of};
use core_geometry::{Bvh, Mat4, Mesh, Vec3, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{Blocked, ModelSupports, Shown, SupportTree, WallSide, low_points};
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
}

impl<'a> SupportsAsked<'a> {
    /// What `edit` asks, or `None` for an edit that is not `Supports`.
    pub(crate) fn of(edit: &'a Edit) -> Option<Self> {
        match edit {
            Edit::Supports {
                profile,
                at_mm,
                only_at,
                ..
            } => Some(Self {
                profile: profile.as_deref(),
                at_mm,
                only_at: *only_at,
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
    let (groups, group) = group_for(&plate.groups, plate.active, asked.profile)?;
    let profiles: Vec<SupportProfile> = groups.iter().map(|g| g.profile.clone()).collect();
    let table = state.support_table(&profiles, plate.raft);

    let surface = meshes.printed();
    let bvh = Arc::new(Bvh::build(surface));
    let mut supports = supports_of(
        &state.supports,
        &state.hollow.drains,
        &meshes.source,
        surface,
        &bvh,
        transform,
        &table,
    );
    let placed = transform_mesh(surface, transform);
    let placed_bvh = Bvh::build(&placed);
    let on = PlacedModel {
        state,
        meshes,
        placed: (&placed, &placed_bvh),
        wall: wall_outside(state, meshes, &bvh, into_model),
    };
    let profile = &table[usize::from(group)];
    let contacts = contacts_asked(&on, &supports, (asked, profile), (id, cancel))?;
    for contact in &contacts {
        supports.add(*contact, transform, group);
    }
    if cancel.asked() {
        return Err(ApplyError::Cancelled);
    }
    let grown = grown_trees(&mut supports, &on, (&bvh, &table));
    let report = Report::Supported {
        placed: contacts.len(),
        trees: grown.len(),
    };
    document.set_supports(id, groups, (supports.points().to_vec(), grown));
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report,
    })
}

/// The contacts to add, plate mm: those the preset finds unless `only_at`, then the surface
/// point nearest each point asked for.
fn contacts_asked(
    on: &PlacedModel<'_>,
    supports: &ModelSupports,
    (asked, profile): (&SupportsAsked<'_>, &SupportProfile),
    (id, cancel): (ObjectId, Cancel<'_>),
) -> Result<Vec<Vec3>, ApplyError> {
    let mut contacts = Vec::new();
    if !asked.only_at {
        let seeds = supports.contacts(on.state.transform);
        contacts = found(on, &seeds, profile, (id, cancel))?;
    }
    for at in asked.at_mm {
        let closest = on.placed.1.closest(*at).ok_or(ApplyError::NoSurface(id))?;
        contacts.push(closest.point);
    }
    Ok(contacts)
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
}

/// Where the model needs holding up by `profile`, plate mm, with `seeds` standing already.
fn found(
    on: &PlacedModel<'_>,
    seeds: &[Vec3],
    profile: &SupportProfile,
    (id, cancel): (ObjectId, Cancel<'_>),
) -> Result<Vec<Vec3>, ApplyError> {
    let failed = |reason: String| ApplyError::Failed { object: id, reason };
    let (placed, bvh) = on.placed;
    let transform = on.state.transform;
    let settings = SliceSettings::default();
    let stack = PlaneSliceEngine
        .slice(placed, &settings)
        .map_err(|error| failed(error.to_string()))?;
    let source = &on.meshes.source;
    let shown = Shown::new(source, &on.state.shown_painted, transform);
    let asked = NeedsAsked {
        stack: &stack,
        surface: placed,
        bvh,
        layer_height_mm: settings.layer_height,
        plan: &plan_of(profile, None),
        lows: &low_points(on.meshes.printed(), transform),
        painted: Blocked::new(source, &on.state.supports.blocked, transform),
        shown: shown.as_ref(),
        seeds,
        side: on.wall.clone(),
    };
    let needs = needs_on(asked, seeds, &mut |_| !cancel.asked())
        .map_err(|error| failed(error.to_string()))?;
    if cancel.asked() {
        return Err(ApplyError::Cancelled);
    }
    Ok(needs.into_iter().map(|need| need.contact).collect())
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

#[cfg(test)]
mod tests;
