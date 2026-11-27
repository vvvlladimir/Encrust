//! One project's models put on a plate of another, as the window stands a revision beside what
//! stands and the command line lays several projects on one plate (ADR 0462, 0459).

use std::collections::BTreeSet;

use printer_profiles::SupportProfile;

use super::{Group, ObjectState, OwnSupportState, Project, assign_ids};

/// Puts `from`'s plate, plugs and all, on `plate` of `into` after what stands there, an object
/// without its mesh left out; a taken id is renumbered, each group joins `into`'s of its name.
pub fn insert(into: &mut Project, from: Project, plate: u32) {
    let Project {
        manifest, models, ..
    } = from;
    let theirs = &manifest.supports.groups;
    let joined = join_groups(theirs, &mut into.manifest.supports.groups);
    let table: Vec<SupportProfile> = (into.manifest.supports.groups.iter())
        .map(|group| group.profile.clone())
        .collect();
    let raft = manifest
        .supports
        .raft
        .filter(|raft| into.manifest.supports.raft.as_ref() != Some(raft));
    let kept: Vec<usize> = (manifest.objects.iter().enumerate())
        .filter(|(index, object)| object.plate == manifest.active_plate && *index < models.len())
        .map(|(index, _)| index)
        .collect();
    let first = into.manifest.objects.len();
    let moved = |index: u32| {
        let at = kept.iter().position(|&kept| kept == index as usize)?;
        u32::try_from(first + at).ok()
    };
    for &index in &kept {
        let mut state = manifest.objects[index].clone();
        state.plate = plate;
        state.plug_of = state.plug_of.and_then(moved);
        regroup(&mut state, &joined, theirs, &table);
        state.own_raft = state.own_raft.or(raft);
        into.manifest.objects.push(state);
        into.models.push(models[index].clone());
    }
    assign_ids(&mut into.manifest.objects);
}

/// The group of `ours` each of `theirs` is built in: the one of its name, else a new one.
pub fn join_groups(theirs: &[Group], ours: &mut Vec<Group>) -> Vec<u16> {
    (theirs.iter())
        .map(|group| {
            let at = (ours.iter().position(|our| our.name == group.name)).unwrap_or_else(|| {
                ours.push(group.clone());
                ours.len() - 1
            });
            at as u16
        })
        .collect()
}

/// Renumbers `state`'s supports into the groups `joined` names, keeping as its own the profile
/// of each group of `theirs` it stands in that the same group of `table` builds otherwise.
pub fn regroup(
    state: &mut ObjectState,
    joined: &[u16],
    theirs: &[Group],
    table: &[SupportProfile],
) {
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
    for group in used {
        let (Some(their), ours) = (theirs.get(group as usize), to(group)) else {
            continue;
        };
        let kept = state.own_supports.iter().any(|own| own.group == ours);
        if !kept && table.get(ours as usize) != Some(&their.profile) {
            state.own_supports.push(OwnSupportState {
                group: ours,
                profile: their.profile.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ObjectId;
    use super::super::tests::project;
    use super::*;

    #[test]
    fn a_project_inserted_keeps_its_models_on_the_plate_named_with_fresh_ids_and_plugs() {
        let mut into = project(1);
        let mut from = project(2);
        from.manifest.objects[1].plug_of = Some(0);

        insert(&mut into, from, 0);

        let objects = &into.manifest.objects;
        assert_eq!(
            objects.len(),
            3,
            "the one standing and the two put beside it"
        );
        let ids: BTreeSet<ObjectId> = objects.iter().map(|object| object.id).collect();
        assert_eq!(ids.len(), 3, "no id taken twice: {ids:?}");
        assert_eq!(objects[2].plug_of, Some(1), "the plug follows its model");
        assert_eq!(into.models.len(), into.manifest.objects.len());
    }

    #[test]
    fn an_object_whose_model_is_missing_is_left_out() {
        let mut into = project(1);
        let mut from = project(3);
        from.models.truncate(2);

        insert(&mut into, from, 0);

        assert_eq!(into.models.len(), into.manifest.objects.len());
        assert_eq!(into.manifest.objects.len(), 3, "the third has no model");
    }

    #[test]
    fn a_group_of_a_name_present_is_joined_and_a_new_one_added() {
        let profile = SupportProfile::default();
        let named = |name: &str| Group {
            name: name.into(),
            profile: profile.clone(),
        };
        let mut ours = vec![named("medium")];
        let joined = join_groups(&[named("light"), named("medium")], &mut ours);
        assert_eq!(joined, [1, 0]);
        assert_eq!(ours.len(), 2);
    }
}
