use super::{FIRST_GROUP, ISLANDS_GROUP, SupportGroup, SupportTool};
use crate::scene::{OwnSettings, Scene, SceneObject};
use anyhow::Result;
use core_engine::project::own_table;
use core_engine::{SupportPlan, SupportShapes};
use printer_profiles::{Catalogue, Raft, SupportProfile};

/// The raft the first picked model keeps of its own, which only a project can give it.
pub fn own_raft_shown(scene: &Scene) -> Option<Raft> {
    let first = scene.selection().first()?;
    scene.get(*first)?.own.raft
}

/// `table` as a model keeping `own` settings is built to it, on its own raft or
/// `plate_raft`: the rule a project is cut by, `core_engine::project::own_table`.
pub fn with_own(
    table: Vec<SupportProfile>,
    own: &OwnSettings,
    plate_raft: Raft,
) -> Vec<SupportProfile> {
    let profiles = own
        .supports
        .iter()
        .map(|(group, profile)| (*group, profile));
    own_table(table, profiles, Some(own.raft.unwrap_or(plate_raft)))
}

impl SupportTool {
    /// Every group's profile, with the active group's taken from the numbers being
    /// edited rather than from the stale copy in the table.
    pub fn table(&self) -> Vec<SupportProfile> {
        self.groups
            .iter()
            .enumerate()
            .map(|(group, entry)| {
                if group == self.active as usize {
                    self.profile.clone()
                } else {
                    entry.profile.clone()
                }
            })
            .collect()
    }

    /// Which groups are left out of the view, by group number.
    pub fn hidden(&self) -> Vec<bool> {
        self.groups.iter().map(|group| group.hidden).collect()
    }

    /// Every group's profile as `object` is built to: its own where it keeps one, the
    /// plate's where it does not. What every rebuild of a model's supports is given.
    pub fn table_for(&self, object: &SceneObject) -> Vec<SupportProfile> {
        with_own(self.table(), &object.own, self.raft)
    }

    /// Makes `raft` the plate's, or the picked models' own while the panel shows one. A
    /// model whose raft is the plate's again keeps none of its own; see ADR 0291.
    pub fn keep_raft(&mut self, raft: Raft, scene: &mut Scene) {
        if own_raft_shown(scene).is_none() {
            self.raft = raft;
            return;
        }
        for id in scene.selection().to_vec() {
            if let Some(object) = scene.get_mut(id)
                && object.own.raft.is_some()
            {
                object.own.raft = (raft != self.raft).then_some(raft);
            }
        }
    }

    /// What orienting weighs each model's supports by, as a run would build them: the
    /// automatic profile's sizes where one is chosen, else the active group's profile, on
    /// the model's own raft or the plate's.
    pub fn orient_plans<'a>(
        &'a self,
        catalogue: &Catalogue,
    ) -> Result<impl Fn(&SceneObject) -> SupportPlan + 'a> {
        let sized = self.sized(catalogue)?;
        Ok(move |object: &SceneObject| {
            let mut plan = match &sized {
                Some((auto, shapes)) => SupportPlan {
                    shapes: SupportShapes::Sized {
                        auto: Box::new(auto.clone()),
                        shapes: Box::new(shapes.clone()),
                    },
                    cavity: false,
                },
                None => SupportPlan::of(self.profile_for(object).clone()),
            };
            let raft = object.own.raft.unwrap_or(self.raft);
            for profile in plan.profiles_mut() {
                profile.raft = raft;
            }
            plan
        })
    }

    /// The active group's profile as `object` is built to.
    pub fn profile_for<'a>(&'a self, object: &'a SceneObject) -> &'a SupportProfile {
        object
            .own
            .supports
            .get(&self.active)
            .unwrap_or(&self.profile)
    }

    /// The numbers the panel shows and the viewport marks overhangs by: the first picked
    /// model's own where it keeps them, the plate's otherwise.
    pub fn shown<'a>(&'a self, scene: &'a Scene) -> &'a SupportProfile {
        scene
            .selection()
            .first()
            .and_then(|id| scene.get(*id))
            .map_or(&self.profile, |object| self.profile_for(object))
    }

    /// Puts `edited` on every picked model as its own profile of the active group, or
    /// takes its own away where `edited` is what the plate already has.
    pub fn keep_on_picked(&self, edited: &SupportProfile, scene: &mut Scene) {
        for id in scene.selection().to_vec() {
            let Some(object) = scene.get_mut(id) else {
                continue;
            };
            if *edited == self.profile {
                object.own.supports.remove(&self.active);
            } else {
                object.own.supports.insert(self.active, edited.clone());
            }
        }
    }

    /// Takes on a profile that has just been saved: the plate's numbers of that name
    /// become the saved ones, so every model built to it follows, and a model whose own
    /// are now no different lets go of them. A model keeping other edits keeps them.
    pub fn adopt_saved(&mut self, scene: &mut Scene) {
        let Some(saved) = self.saved.take() else {
            return;
        };
        if self.profile.name == saved.name {
            self.profile = saved.clone();
        }
        for group in &mut self.groups {
            if group.profile.name == saved.name {
                group.profile = saved.clone();
            }
        }
        let table = self.table();
        for object in scene.objects_mut() {
            object
                .own
                .supports
                .retain(|group, own| table.get(*group as usize).is_some_and(|plate| plate != own));
        }
    }

    /// Takes up the profile the panel's list had chosen: the group built to it becomes
    /// the one in hand, and one is made for it when there is none. What already stands
    /// stays in the group it stands in, built as it was. See `docs/decisions/0247`.
    ///
    /// Chosen again, the profile in hand goes back to what is saved, on the picked models
    /// too. A group nothing stands in is retuned instead of left behind empty.
    pub fn take_up_chosen(&mut self, scene: &mut Scene) {
        let Some(profile) = self.chosen.take() else {
            return;
        };
        let table = self.table();
        if self.profile.name == profile.name {
            self.profile = profile;
            for id in scene.selection().to_vec() {
                if let Some(object) = scene.get_mut(id) {
                    object.own.supports.remove(&self.active);
                }
            }
            return;
        }
        if let Some(group) = table.iter().position(|kind| kind.name == profile.name) {
            self.choose_group(group as u16);
            return;
        }

        let active = self.active as usize;
        if self.group_holds(self.active, scene) == 0 && self.groups[active].name != ISLANDS_GROUP {
            // Named for its profile, or still the name it was born with: follow the new one.
            let named_for_it = [self.profile.name.as_str(), FIRST_GROUP]
                .contains(&self.groups[active].name.as_str());
            if named_for_it {
                self.groups[active].name.clone_from(&profile.name);
            }
            for object in scene.objects_mut() {
                object.own.supports.remove(&self.active);
            }
            self.groups[active].profile = profile.clone();
            self.profile = profile;
            return;
        }
        self.groups[active].profile = self.profile.clone();
        self.groups.push(SupportGroup {
            name: profile.name.clone(),
            profile: profile.clone(),
            hidden: false,
        });
        self.active = (self.groups.len() - 1) as u16;
        self.profile = profile;
    }

    /// Builds `group` to `profile` instead of the one it had: every support in it is
    /// rebuilt, on every model, and a model's own numbers for it are let go. The group
    /// takes the profile's name unless the user had given it one.
    pub fn retune_group(&mut self, group: u16, profile: SupportProfile, scene: &mut Scene) {
        let Some(entry) = self.groups.get_mut(group as usize) else {
            return;
        };
        let was = if group == self.active {
            self.profile.name.clone()
        } else {
            entry.profile.name.clone()
        };
        if entry.name == was || entry.name == FIRST_GROUP {
            entry.name.clone_from(&profile.name);
        }
        entry.profile = profile.clone();
        if group == self.active {
            self.profile = profile;
        }
        for object in scene.objects_mut() {
            object.own.supports.remove(&group);
        }
    }

    /// Makes `group` the one new supports join and the one the panel's fields edit,
    /// keeping what was edited of the group being left.
    pub fn choose_group(&mut self, group: u16) {
        if group as usize >= self.groups.len() || group == self.active {
            return;
        }
        self.groups[self.active as usize].profile = self.profile.clone();
        self.active = group;
        self.profile = self.groups[group as usize].profile.clone();
    }

    /// Adds a group starting from the numbers in hand and makes it the active one. The
    /// window makes no group by hand any more; a test builds a plate of several with it.
    #[cfg(test)]
    pub fn add_group(&mut self) {
        self.groups[self.active as usize].profile = self.profile.clone();
        self.groups.push(SupportGroup {
            name: crate::i18n::tr!("supports-group-name", index = self.groups.len() + 1),
            profile: self.profile.clone(),
            hidden: false,
        });
        self.active = (self.groups.len() - 1) as u16;
    }

    /// How many supports stand in `group`, which is what dropping it would take away.
    pub fn group_holds(&self, group: u16, scene: &Scene) -> usize {
        scene
            .objects()
            .iter()
            .map(|object| object.supports.group_count(group))
            .sum()
    }

    /// Takes a group away, and with it every support built to it. The last one cannot
    /// go: something has to hold the raft's numbers, and when the first goes the one
    /// after it takes its place.
    ///
    /// Returns how many supports went. A group is a shape, so its supports leave with it
    /// rather than being rebuilt to another group's numbers; `Cmd+Z` puts them back.
    pub fn remove_group(&mut self, group: u16, scene: &mut Scene) -> usize {
        if self.groups.len() <= 1 || group as usize >= self.groups.len() {
            return 0;
        }
        if self.active != group {
            self.groups[self.active as usize].profile = self.profile.clone();
        }
        self.groups.remove(group as usize);
        let gone = scene
            .objects_mut()
            .iter_mut()
            .map(|object| {
                let own = std::mem::take(&mut object.own.supports);
                object.own.supports = own
                    .into_iter()
                    .filter(|(kept, _)| *kept != group)
                    .map(|(kept, profile)| (kept - u16::from(kept > group), profile))
                    .collect();
                object.supports.drop_group(group)
            })
            .sum();
        self.active = 0;
        self.profile = self.groups[0].profile.clone();
        gone
    }

    /// Stands every supported model up by the thickness of the raft under it, and back
    /// down when the raft or its supports go, so the slab is never printed into the part.
    ///
    /// The raft is the model's own, or the plate's (ADR 0253). What each model has
    /// been stood up by is kept beside its supports, which is what makes this safe to ask
    /// every frame. See `docs/decisions/0238`.
    pub fn settle_on_raft(&self, scene: &mut Scene) {
        for object in scene.objects_mut() {
            let raft = object.own.raft.unwrap_or(self.raft);
            let raft_mm = if raft.enabled { raft.thickness_mm } else { 0.0 };
            let wanted_mm = if object.supports.is_empty() {
                0.0
            } else {
                raft_mm
            };
            let step_mm = wanted_mm - object.supports.raft_lift_mm();
            if step_mm.abs() > f32::EPSILON {
                object.transform.translation.z += step_mm;
                object.supports.set_raft_lift_mm(wanted_mm);
            }
        }
    }
}
