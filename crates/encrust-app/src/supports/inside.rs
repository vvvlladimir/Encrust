use super::{ISLANDS_GROUP, NECKS_GROUP, Side, SupportGroup, SupportTool, frozen_inside};
use crate::job::Placement;
use crate::scene::{ObjectId, Scene};
use core_engine::neck_profile;
use core_geometry::Vec3;
use core_supports::SupportTree;
use printer_profiles::SupportProfile;

impl SupportTool {
    /// Stands a support at each of `found`, a model and the plate point of its underside an
    /// island hangs from, in the islands' group, and answers how many; a ring or a crescent
    /// is left to the hand.
    pub fn support_islands(&mut self, scene: &mut Scene, found: &[(ObjectId, Vec3)]) -> usize {
        self.stand_islands(scene, found, |_| None)
    }

    /// The same, each support to the profile `profile_of` names for its model: what a
    /// recipe's run builds islands to, not the panel's.
    pub fn support_islands_to(
        &mut self,
        scene: &mut Scene,
        found: &[(ObjectId, Vec3)],
        profile_of: impl Fn(ObjectId) -> SupportProfile,
    ) -> usize {
        self.stand_islands(scene, found, |id| Some(profile_of(id)))
    }

    /// Stands a support at each of `found`, under a weak neck, in the necks' group: each to
    /// `profile_of`'s for its model, or the panel's, made thick enough to carry the pull.
    pub fn support_necks(
        &mut self,
        scene: &mut Scene,
        found: &[(ObjectId, Vec3)],
        profile_of: impl Fn(ObjectId) -> Option<SupportProfile>,
    ) -> usize {
        for (id, contact) in found {
            let base = profile_of(*id).unwrap_or_else(|| self.profile.clone());
            let group = self.answer_group(NECKS_GROUP, Some(&neck_profile(&base)));
            if let Some(object) = scene.get_mut(*id) {
                let transform = object.transform;
                object.supports.add(*contact, transform, group);
            }
        }
        found.len()
    }

    fn stand_islands(
        &mut self,
        scene: &mut Scene,
        found: &[(ObjectId, Vec3)],
        profile_of: impl Fn(ObjectId) -> Option<SupportProfile>,
    ) -> usize {
        for (id, contact) in found {
            let group = self.answer_group(ISLANDS_GROUP, profile_of(*id).as_ref());
            if let Some(object) = scene.get_mut(*id) {
                let transform = object.transform;
                object.supports.add(*contact, transform, group);
            }
        }
        found.len()
    }

    /// What a run found, each placement keeping only the contacts on the side of the wall
    /// the run was started for. A contact is inside when the surface nearest it bounds the
    /// cavity (ADR 0332).
    pub(super) fn on_its_side(
        mut placements: Vec<Placement>,
        scene: &Scene,
        side: Side,
    ) -> Vec<Placement> {
        let inside = side == Side::Inside;
        for placement in &mut placements {
            let Some(object) = scene.get(placement.id) else {
                continue;
            };
            let into_model = placement.transform.to_matrix().inverse();
            placement.needs.retain(|need| {
                object.holds_inside(into_model.transform_point3(need.contact)) == inside
            });
        }
        placements
    }

    /// How many supports stand inside the cavities of the models aimed at, frozen ones
    /// counted by their tips.
    pub fn inside_count(scene: &Scene) -> usize {
        scene
            .targets()
            .filter(|object| object.supported_inside())
            .map(|object| {
                let points = object.supports.points().iter();
                let frozen = object.supports.frozen().iter();
                points
                    .filter(|point| object.holds_inside(point.contact))
                    .count()
                    + frozen
                        .filter(|tree| frozen_inside(object, tree))
                        .map(SupportTree::tip_count)
                        .sum::<usize>()
            })
            .sum()
    }

    /// Takes every support standing inside a cavity off the models aimed at, the ones a
    /// hand has frozen included. Answers how many went.
    pub fn clear_inside(scene: &mut Scene) -> usize {
        let aimed: Vec<ObjectId> = scene.targets().map(|object| object.id).collect();
        Self::clear_inside_of(scene, &aimed)
    }

    /// The same, off the models `ids` names.
    pub(super) fn clear_inside_of(scene: &mut Scene, ids: &[ObjectId]) -> usize {
        let mut gone = 0;
        for id in ids {
            let Some(object) = scene.get_mut(*id) else {
                continue;
            };
            if !object.supported_inside() {
                continue;
            }
            let points: Vec<bool> = object
                .supports
                .points()
                .iter()
                .map(|point| object.holds_inside(point.contact))
                .collect();
            let frozen: Vec<bool> = object
                .supports
                .frozen()
                .iter()
                .map(|tree| frozen_inside(object, tree))
                .collect();
            gone += object.supports.retain_points(|index| !points[index]);
            gone += object.supports.retain_frozen(|index| !frozen[index]);
        }
        gone
    }

    /// The group named `name` for what Check answers: the one of `profile` when one is asked,
    /// else the panel's numbers in hand the first time.
    pub(super) fn answer_group(&mut self, name: &str, profile: Option<&SupportProfile>) -> u16 {
        let held = |group: &SupportGroup| {
            group.name == name && profile.is_none_or(|asked| group.profile == *asked)
        };
        if let Some(group) = self.groups.iter().position(held) {
            return group as u16;
        }
        let profile = match profile {
            Some(asked) => asked.clone(),
            None => {
                self.groups[self.active as usize].profile = self.profile.clone();
                self.profile.clone()
            }
        };
        self.groups.push(SupportGroup {
            name: name.to_owned(),
            profile,
            hidden: false,
        });
        (self.groups.len() - 1) as u16
    }
}
