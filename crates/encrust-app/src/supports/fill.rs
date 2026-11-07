use super::SupportTool;
use crate::scene::{ObjectId, Scene, SceneObject};
use core_engine::Hanging;
use core_geometry::{Adjacency, Scalar, Vec3};
use core_supports::{Placed, Projected, project};
use printer_profiles::Catalogue;

/// For each of `floors`, the printable model of the plate in hand its island hangs from and
/// where a support there touches it, by `core_engine::island_holds`.
pub fn island_holds(
    scene: &Scene,
    floors: &[Vec3],
    layer_mm: Scalar,
) -> Vec<Option<(ObjectId, Vec3)>> {
    let objects: Vec<&SceneObject> = scene.printable(scene.active_plate()).collect();
    let models: Vec<Hanging> = objects
        .iter()
        .map(|object| (&*object.mesh, &*object.bvh, object.transform))
        .collect();
    core_engine::island_holds(floors, layer_mm, &models)
        .into_iter()
        .map(|held| held.map(|(at, contact)| (objects[at].id, contact)))
        .collect()
}

impl SupportTool {
    /// Fills every painted patch on the plate with supports, and returns how many were
    /// put down: the rim and the inside each in the group of its chosen profile (ADR 0331).
    pub fn fill(&mut self, scene: &mut Scene, catalogue: &Catalogue) -> usize {
        let filled: Vec<(ObjectId, Projected)> = scene
            .targets()
            .filter(|object| !object.supports.painted().is_empty())
            .map(|object| (object.id, self.projected(object)))
            .collect();
        let rim = filled
            .iter()
            .any(|(_, projected)| !projected.rim.is_empty());
        let inside = filled
            .iter()
            .any(|(_, projected)| !projected.inside.is_empty());
        let rim_group = match rim {
            true => self.fill_group(self.fill_profiles.rim.clone(), &[], catalogue, scene),
            false => self.active,
        };
        let inside_group = match inside {
            true => {
                let profile = self.fill_profiles.inside.clone();
                self.fill_group(profile, &[rim_group], catalogue, scene)
            }
            false => self.active,
        };
        let mut added = 0;
        for (id, projected) in filled {
            let Some(object) = scene.get_mut(id) else {
                continue;
            };
            let transform = object.transform;
            for (contacts, group) in [(projected.rim, rim_group), (projected.inside, inside_group)]
            {
                added += contacts.len();
                for contact in contacts {
                    object.supports.add(contact, transform, group);
                }
            }
        }
        added
    }

    /// Where the patch painted on `object` wants supports. The face adjacency its rim is
    /// walked over is built here and dropped: a fill is asked for once, not every frame.
    fn projected(&self, object: &SceneObject) -> Projected {
        let keep_out = object.supports.keep_out(&object.mesh, object.transform);
        // On the model the patch is painted on: a fill reads its faces by number.
        let placed =
            Placed::new(&object.mesh, &object.bvh, object.transform).blocking(keep_out.as_ref());
        let matrix = object.transform.to_matrix();
        let standing: Vec<Vec3> = object
            .supports
            .points()
            .iter()
            .map(|point| matrix.transform_point3(point.contact))
            .collect();
        project(
            &placed,
            &Adjacency::of(&object.mesh),
            object.supports.painted(),
            &self.fill,
            &standing,
            self.profile_for(object).max_overhang_deg,
        )
    }

    /// The group one part of a fill goes into: the group in hand, or the one built to the
    /// saved profile `id`, else one made for it that is none of `taken`.
    pub(super) fn fill_group(
        &mut self,
        id: Option<String>,
        taken: &[u16],
        catalogue: &Catalogue,
        scene: &mut Scene,
    ) -> u16 {
        let Some(id) = id else {
            return self.active;
        };
        // A profile deleted since it was chosen leaves the fill to the group in hand.
        let Ok(entry) = catalogue.support(&id) else {
            return self.active;
        };
        if let Some(group) = self
            .table()
            .iter()
            .position(|kind| kind.name == entry.profile.name)
        {
            return group as u16;
        }
        let mut taken = taken.to_vec();
        taken.push(self.active);
        self.group_to_build(entry.profile.clone(), &taken, scene)
    }
}
