//! A project's objects put on the plate beside what stands, as the window shows a revision of
//! a chat client's job by the plate its check wrote (ADR 0473).

use std::collections::BTreeSet;

use core_engine::project::{Group, ObjectState, OwnSupportState, Project};

use super::state::restore_objects;
use crate::scene::{ObjectId, Scene};
use crate::supports::{SupportGroup, SupportTool};

/// Puts every object of `project` on the scene's active plate beside what stands, under fresh
/// ids where theirs are taken, its support groups joined to the window's by name. Answers the ids.
pub fn insert(project: Project, scene: &mut Scene, supports: &mut SupportTool) -> Vec<ObjectId> {
    let Project {
        manifest, models, ..
    } = project;
    let plate = &manifest.supports;
    let joined = join_groups(&plate.groups, supports);
    let raft = plate.raft.filter(|raft| *raft != supports.raft);
    let active = scene.active_plate();
    let objects: Vec<ObjectState> = (manifest.objects.into_iter())
        .map(|mut state| {
            state.plate = active;
            regroup(&mut state, &joined, &plate.groups, supports);
            state.own_raft = state.own_raft.or(raft);
            state
        })
        .collect();
    let table = supports.table();
    restore_objects(scene, objects, models, &table, supports.raft)
}

/// The window's group each of `groups` is built in: the one of its name, or a new one.
fn join_groups(groups: &[Group], supports: &mut SupportTool) -> Vec<u16> {
    (groups.iter())
        .map(|group| {
            let named = (supports.groups.iter()).position(|ours| ours.name == group.name);
            let at = named.unwrap_or_else(|| {
                supports.groups.push(SupportGroup {
                    name: group.name.clone(),
                    profile: group.profile.clone(),
                    hidden: false,
                });
                supports.groups.len() - 1
            });
            at as u16
        })
        .collect()
}

/// Renumbers `state`'s supports into the window's groups `joined`, keeping as its own the
/// profile of each group it stands in that the window's group of that name builds otherwise.
fn regroup(state: &mut ObjectState, joined: &[u16], groups: &[Group], supports: &SupportTool) {
    let to = |group: u16| joined.get(group as usize).copied().unwrap_or(group);
    let held = &mut state.supports;
    let mut used: BTreeSet<u16> = held.points.iter().map(|point| point.group).collect();
    for tree in held.frozen.iter().chain(&held.grown) {
        used.insert(tree.group());
        used.extend(tree.tip_groups());
    }
    for point in &mut held.points {
        point.group = to(point.group);
    }
    for tree in held.frozen.iter_mut().chain(&mut held.grown) {
        tree.renumber_groups(to);
    }
    for own in &mut state.own_supports {
        own.group = to(own.group);
    }
    let table = supports.table();
    for group in used {
        let (Some(theirs), ours) = (groups.get(group as usize), to(group)) else {
            continue;
        };
        let kept = state.own_supports.iter().any(|own| own.group == ours);
        if !kept && table.get(ours as usize) != Some(&theirs.profile) {
            state.own_supports.push(OwnSupportState {
                group: ours,
                profile: theirs.profile.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests;
