//! A project's objects put on the plate beside what stands, as the window shows a revision of
//! a chat client's job by the plate its check wrote (ADR 0473).

use core_engine::project::{Group, ObjectState, Project, regroup};

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
            regroup(&mut state, &joined, &plate.groups, &supports.table());
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

#[cfg(test)]
mod tests;
