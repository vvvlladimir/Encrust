use super::fill::underside;
use super::{ISLANDS_GROUP, Side, SupportGroup, SupportTool, frozen_inside};
use crate::job::Placement;
use crate::scene::{ObjectId, Scene};
use core_geometry::{Scalar, Vec3};
use core_supports::SupportTree;

impl SupportTool {
    /// Stands one support under each island floor of `floors` (plate, `layer_mm` deep), in the
    /// islands' group, and answers how many; a ring or a crescent is left to the hand.
    pub fn support_islands(
        &mut self,
        scene: &mut Scene,
        floors: &[Vec3],
        layer_mm: Scalar,
    ) -> usize {
        let found: Vec<(ObjectId, Vec3)> = floors
            .iter()
            .filter_map(|floor| underside(scene, *floor, layer_mm))
            .collect();
        if found.is_empty() {
            return 0;
        }
        let group = self.islands_group();
        for (id, contact) in &found {
            if let Some(object) = scene.get_mut(*id) {
                let transform = object.transform;
                object.supports.add(*contact, transform, group);
            }
        }
        found.len()
    }

    /// What a run found, each placement keeping only the contacts on the side of the wall
    /// the run was started for. A contact is inside when the surface nearest it bounds the
    /// cavity (ADR 0334).
    pub(super) fn on_its_side(
        &self,
        mut placements: Vec<Placement>,
        scene: &Scene,
    ) -> Vec<Placement> {
        let inside = self.side == Side::Inside;
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
        let mut gone = 0;
        for object in scene.targets_mut() {
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

    /// The group named for islands, made from the numbers in hand the first time.
    pub(super) fn islands_group(&mut self) -> u16 {
        if let Some(group) = self
            .groups
            .iter()
            .position(|group| group.name == ISLANDS_GROUP)
        {
            return group as u16;
        }
        self.groups[self.active as usize].profile = self.profile.clone();
        self.groups.push(SupportGroup {
            name: ISLANDS_GROUP.to_owned(),
            profile: self.profile.clone(),
            hidden: false,
        });
        (self.groups.len() - 1) as u16
    }
}
