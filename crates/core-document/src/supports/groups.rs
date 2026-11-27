//! The plate's support groups a run grows into, and the plan it sizes its supports by.

use core_edit::AUTO;
use core_engine::project::{AutoSizing, Group, ObjectState};
use core_engine::{SupportPlan, plan_of};
use core_geometry::Transform;
use core_supports::{ModelSupports, Need};
use printer_profiles::{Catalogue, ProfileError, Raft, SupportProfile};

use super::SupportsAsked;
use crate::error::ApplyError;

/// The profiles of `groups` as `state` is built to them: its own where it keeps any, on its raft.
pub(super) fn table_of(
    state: &ObjectState,
    groups: &[Group],
    raft: Option<Raft>,
) -> Vec<SupportProfile> {
    let profiles: Vec<SupportProfile> = groups.iter().map(|g| g.profile.clone()).collect();
    state.support_table(&profiles, raft)
}

/// Adds each of `needs` to `supports` in the group of its size, which `groups` gains where
/// the plan's shape has none yet; `group` takes a plan of one profile.
pub(super) fn stand_needs(
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
pub(super) fn plan_for(
    asked: &SupportsAsked<'_>,
    (sizing, groups): (Option<&AutoSizing>, &[Group]),
    profile: &SupportProfile,
) -> Result<SupportPlan, ApplyError> {
    let mut plan = match asked.profile {
        Some(AUTO) => sized_plan(Catalogue::bundled(), (sizing, groups), profile)?,
        _ => plan_of(profile, None),
    };
    plan.cavity = asked.cavity;
    plan.shown_at = asked.shown_at.to_vec();
    Ok(plan)
}

/// Supports sized by `sizing`'s profile where the plate keeps one, else by the first of
/// `catalogue`; a shape is the plate's group of its name where there is one.
pub(super) fn sized_plan(
    catalogue: Result<Catalogue, ProfileError>,
    (sizing, groups): (Option<&AutoSizing>, &[Group]),
    profile: &SupportProfile,
) -> Result<SupportPlan, ApplyError> {
    let catalogue = catalogue.map_err(|error| ApplyError::BadCatalogue(error.to_string()))?;
    let kept = sizing.map(|sizing| sizing.profile.clone());
    let auto = kept
        .or_else(|| catalogue.autos().next().map(|entry| entry.profile.clone()))
        .ok_or_else(|| ApplyError::NoProfile(AUTO.into()))?;
    let mut plan = SupportPlan::sized(auto, &catalogue).map_err(|error| match error {
        ProfileError::Unknown { id, .. } => ApplyError::NoProfile(id),
        other => ApplyError::BadCatalogue(other.to_string()),
    })?;
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
pub(super) fn group_by_size(
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

/// The plate's groups with the one `profile` names, and its number: the active group where
/// none is named, a shipped preset put in as a group where the plate has none by its name.
pub(super) fn group_for(
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
