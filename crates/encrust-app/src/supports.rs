use anyhow::{Result, bail};
use core_geometry::{Adjacency, Ray, Scalar, Vec2, Vec3, lift_over_plate, raycast_placed};
use core_supports::{Part, Placed, ProjectSettings, project};
use printer_profiles::SupportProfile;

use crate::job::{SupportJob, SupportOutcome, SupportRequest, tasks_of};
use std::collections::BTreeMap;

use crate::scene::{ObjectId, Scene, SceneObject};
use crate::status::Status;

/// What the one group a plate starts with is called until a profile is chosen for it.
const FIRST_GROUP: &str = "Default";

/// The group the Check tool stands island supports in, apart from the ones built by hand.
pub const ISLANDS_GROUP: &str = "Islands";

/// What a click on the model does while the Paint tool is in hand: stand one support,
/// paint the patch a fill covers, paint the patch supports keep out of, or take either
/// paint off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placing {
    #[default]
    Point,
    Paint,
    Block,
    Erase,
}

impl Placing {
    pub const ALL: [Self; 4] = [Self::Point, Self::Paint, Self::Block, Self::Erase];

    pub fn label(self) -> &'static str {
        match self {
            Self::Point => "Place",
            Self::Paint => "Fill",
            Self::Block => "Block",
            Self::Erase => "Erase",
        }
    }

    /// Whether this works the brush over a patch rather than on a single support.
    pub fn paints(self) -> bool {
        matches!(self, Self::Paint | Self::Block | Self::Erase)
    }

    /// Whether what it paints is the patch supports keep out of.
    pub fn blocks(self) -> bool {
        self == Self::Block
    }
}

/// A named set of supports built to one profile. A support carries the number of its
/// group, so retuning a group retunes every support in it; see `docs/decisions/0094`.
#[derive(Debug, Clone)]
pub struct SupportGroup {
    pub name: String,
    pub profile: SupportProfile,
    /// Left out of the view, though not out of the print: a way to see past one kind of
    /// support to another.
    pub hidden: bool,
}

/// One piece of one frozen support: what the Edit mode selects, shows picked and carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picked {
    pub id: crate::scene::ObjectId,
    /// Which of the object's frozen trees, which is stable until it is taken away.
    pub frozen: usize,
    pub part: Part,
}

/// What the Supports tool places with, and the automatic run it has going.
#[derive(Debug)]
pub struct SupportTool {
    /// The active group's numbers, which the panel's fields edit in place. They are
    /// written back into `groups` whenever another group is chosen.
    pub profile: SupportProfile,
    /// Every group on the plate, the first of which everything falls back to.
    pub groups: Vec<SupportGroup>,
    /// Which group new supports join, and whose numbers `profile` is holding.
    pub active: u16,
    /// The parts the Edit mode is working on. Several at once: a drag carries all of
    /// them by the same step.
    pub picked: Vec<Picked>,
    /// Where the part under the cursor stood when the drag began, in plate coordinates,
    /// so that every step is measured from the same place.
    pub anchor: Option<Vec3>,
    pub placing: Placing,
    /// Radius of the brush, millimetres of plate.
    pub brush_radius_mm: Scalar,
    /// How far a surface may turn from the face that was clicked and still be painted
    /// with it, degrees.
    pub flood_angle_deg: Scalar,
    pub fill: ProjectSettings,
    /// The brush has hold of the pointer: the press that started this drag landed on a
    /// model, so the stroke paints instead of turning the camera.
    pub stroke: bool,
    /// Where the brush was last put down, in panel points, so that a fast drag is painted
    /// as the line it drew rather than as the frames it was sampled on.
    pub last_daub: Option<Vec2>,
    pub job: Option<SupportJob>,
    /// A profile the panel has just saved, until the plate has taken it on.
    pub saved: Option<SupportProfile>,
    /// A profile the panel's list has just had chosen, until a group has taken it up.
    pub chosen: Option<SupportProfile>,
    /// The group the panel is asking about before it takes it away, and the supports it
    /// holds, or `None` when it is asking nothing.
    pub dropping: Option<(u16, usize)>,
}

impl Default for SupportTool {
    fn default() -> Self {
        Self {
            profile: SupportProfile::default(),
            groups: vec![SupportGroup {
                name: FIRST_GROUP.to_owned(),
                profile: SupportProfile::default(),
                hidden: false,
            }],
            active: 0,
            picked: Vec::new(),
            anchor: None,
            placing: Placing::default(),
            brush_radius_mm: 2.0,
            flood_angle_deg: 30.0,
            fill: ProjectSettings::default(),
            stroke: false,
            last_daub: None,
            job: None,
            saved: None,
            chosen: None,
            dropping: None,
        }
    }
}

impl SupportTool {
    /// Why the Generate button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<&'static str> {
        if scene.target_count() == 0 {
            return Some("Nothing visible on the plate to hold up.");
        }
        None
    }

    /// Starts an automatic placement run over everything visible on the plate.
    ///
    /// The run cuts each model at the height it will be printed at, so the layers it
    /// looks at are the layers the printer will expose.
    pub fn start(&mut self, scene: &Scene, layer_height_mm: Scalar) -> Result<()> {
        let tasks = tasks_of(scene, |object| self.profile_for(object).clone());
        if tasks.is_empty() {
            bail!("nothing visible on the plate to hold up");
        }

        self.job = Some(SupportJob::spawn(SupportRequest {
            tasks,
            layer_height_mm,
        }));
        Ok(())
    }

    /// Drains a running placement run into the scene and the stage notice. Returns whether
    /// one is still going, which is what tells the window to keep repainting.
    pub fn poll(&mut self, scene: &mut Scene, status: &mut Status) -> bool {
        let Some(job) = self.job.as_mut() else {
            return false;
        };
        let Some(outcome) = job.poll() else {
            return true;
        };

        self.job = None;
        let group = self.active;
        let added = outcome.count();
        match outcome {
            SupportOutcome::Placed(placements) => {
                for placement in placements {
                    let Some(object) = scene.get_mut(placement.id) else {
                        continue;
                    };
                    for contact in placement.contacts {
                        object.supports.add(contact, placement.transform, group);
                    }
                }
                *status = Status::Info(match added {
                    0 => "Nothing on the plate needs holding up".to_owned(),
                    1 => "Placed 1 support".to_owned(),
                    added => format!("Placed {added} supports"),
                });
            }
            SupportOutcome::Cancelled => *status = Status::Info("Placement cancelled".to_owned()),
            SupportOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

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
        with_own(self.table(), &object.own.supports)
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

    /// Adds a group starting from the numbers in hand and makes it the active one.
    pub fn add_group(&mut self) {
        self.groups[self.active as usize].profile = self.profile.clone();
        self.groups.push(SupportGroup {
            name: format!("Group {}", self.groups.len() + 1),
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

    /// Stands one support under each island whose floor is in `floors`, plate points at
    /// the bottom of the layer it starts on, in the islands' group. Answers how many stood:
    /// an island whose centre misses it, a ring or a crescent, is left to the hand.
    ///
    /// `layer_mm` is how far under a floor the underside may lie and still be the
    /// island's: a layer is cut somewhere inside its own thickness.
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

    /// The group named for islands, made from the numbers in hand the first time.
    fn islands_group(&mut self) -> u16 {
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

    /// Fills every painted patch on the plate with supports, and returns how many were
    /// put down.
    ///
    /// The face adjacency the rim is walked over is built here and thrown away: a fill is
    /// one press of a button, and keeping it beside every model would cost the memory of
    /// a third mesh for something a plate full of models asks for once.
    pub fn fill(&self, scene: &mut Scene) -> usize {
        let filled: Vec<(crate::scene::ObjectId, Vec<Vec3>)> = scene
            .targets()
            .filter(|object| !object.supports.painted().is_empty())
            .map(|object| {
                let keep_out = object.supports.keep_out(&object.mesh, object.transform);
                let placed = Placed::new(&object.mesh, &object.bvh, object.transform)
                    .blocking(keep_out.as_ref());
                let adjacency = Adjacency::of(&object.mesh);
                let matrix = object.transform.to_matrix();
                let standing: Vec<Vec3> = object
                    .supports
                    .points()
                    .iter()
                    .map(|point| matrix.transform_point3(point.contact))
                    .collect();
                (
                    object.id,
                    project(
                        &placed,
                        &adjacency,
                        object.supports.painted(),
                        &self.fill,
                        &standing,
                        self.profile_for(object).max_overhang_deg,
                    ),
                )
            })
            .collect();

        let mut added = 0;
        for (id, contacts) in filled {
            let Some(object) = scene.get_mut(id) else {
                continue;
            };
            let transform = object.transform;
            for contact in contacts {
                object.supports.add(contact, transform, self.active);
                added += 1;
            }
        }
        added
    }

    /// Stands everything visible `z_lift_mm` clear of the plate, over whatever its raft
    /// has already stood it up by.
    ///
    /// Supports need room under a part, and the layers printed straight onto the plate
    /// are the ones that stick to it hardest. Returns how many objects moved.
    pub fn lift(&self, scene: &mut Scene) -> usize {
        let mut moved = 0;
        for object in scene.targets_mut() {
            let lift_mm = self.profile_for(object).z_lift_mm;
            let Some(bounds) = object.world_bounds() else {
                continue;
            };
            let offset = lift_over_plate(&bounds, lift_mm + object.supports.raft_lift_mm());
            if offset.z.abs() < f32::EPSILON {
                continue;
            }
            object.transform.translation += offset;
            moved += 1;
        }
        moved
    }

    /// Stands every supported model up by the thickness of the raft under it, and back
    /// down when the raft or its supports go, so the slab is never printed into the part.
    ///
    /// The raft is the plate's, built to the first group's profile. What each model has
    /// been stood up by is kept beside its supports, which is what makes this safe to ask
    /// every frame. See `docs/decisions/0238`.
    pub fn settle_on_raft(&self, scene: &mut Scene) {
        let table = self.table();
        for object in scene.objects_mut() {
            let raft_mm = object
                .own
                .supports
                .get(&0)
                .or(table.first())
                .filter(|ground| ground.raft.enabled)
                .map_or(0.0, |ground| ground.raft.thickness_mm);
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

/// `table` with the profiles a model keeps of its own put in place of its groups'.
pub fn with_own(
    mut table: Vec<SupportProfile>,
    own: &BTreeMap<u16, SupportProfile>,
) -> Vec<SupportProfile> {
    for (group, profile) in own {
        if let Some(plate) = table.get_mut(*group as usize) {
            plate.clone_from(profile);
        }
    }
    table
}

/// The first surface straight over a point a layer under `floor`, if it is no more than
/// two layers up: the underside of the island standing there.
fn underside(scene: &Scene, floor: Vec3, layer_mm: Scalar) -> Option<(ObjectId, Vec3)> {
    let ray = Ray::new(floor - Vec3::Z * layer_mm, Vec3::Z);
    scene
        .printable(scene.active_plate())
        .filter_map(|object| {
            let hit = raycast_placed(&object.mesh, &object.bvh, object.transform, &ray)?;
            Some((object.id, hit))
        })
        .min_by(|(_, a), (_, b)| a.distance.total_cmp(&b.distance))
        .filter(|(_, hit)| hit.distance <= 2.0 * layer_mm)
        .map(|(id, hit)| (id, hit.point))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Bvh, Mesh, Transform};
    use core_supports::SupportTree;
    use std::sync::Arc;

    /// A scene holding one unit cube standing on the plate.
    fn one_cube() -> Scene {
        use crate::scene::{ImportSummary, Imported};
        use core_geometry::{Orientation, diagnose};

        let cube = crate::supports::tests::cube();
        let summary = ImportSummary {
            vertices_merged: 0,
            faces_removed: 0,
            orientation: Orientation {
                flipped_faces: 0,
                inverted_shells: 0,
                orientable: true,
            },
            diagnostics: diagnose(&cube),
        };

        let mut scene = Scene::default();
        scene.insert(Imported::new(
            "cube".to_owned(),
            Arc::new(cube),
            Transform::default(),
            summary,
        ));
        scene
    }

    /// The cube with one support under it in the first group.
    fn one_supported_cube() -> Scene {
        let mut scene = one_cube();
        let object = &mut scene.objects_mut()[0];
        object.transform.translation.z = 10.0;
        let transform = object.transform;
        object.supports.add(Vec3::new(0.5, 0.5, 10.0), transform, 0);
        scene
    }

    #[test]
    fn another_profile_starts_a_kind_of_its_own_and_leaves_what_stands_alone() {
        let mut scene = one_supported_cube();
        let mut tool = SupportTool::default();
        let first = tool.profile.clone();
        let other = SupportProfile::heavy();
        assert_ne!(first.name, other.name);

        tool.chosen = Some(other.clone());
        tool.take_up_chosen(&mut scene);

        assert_eq!(tool.groups.len(), 2, "what stands keeps its group");
        assert_eq!(tool.active, 1, "and the next support joins the new one");
        assert_eq!(tool.groups[1].name, other.name);
        assert_eq!(tool.table(), vec![first, other]);
        assert_eq!(scene.objects()[0].supports.group_count(0), 1);
    }

    #[test]
    fn a_group_nothing_stands_in_is_retuned_rather_than_left_behind() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        let other = SupportProfile::heavy();

        tool.chosen = Some(other.clone());
        tool.take_up_chosen(&mut scene);

        assert_eq!(tool.groups.len(), 1);
        assert_eq!(tool.groups[0].name, other.name);
        assert_eq!(tool.table(), vec![other]);
    }

    #[test]
    fn a_profile_a_group_is_built_to_is_that_group_again() {
        let mut scene = one_supported_cube();
        let mut tool = SupportTool::default();
        let first = tool.profile.clone();
        tool.chosen = Some(SupportProfile::heavy());
        tool.take_up_chosen(&mut scene);

        tool.chosen = Some(first);
        tool.take_up_chosen(&mut scene);

        assert_eq!(
            tool.groups.len(),
            2,
            "no third group for a profile already there"
        );
        assert_eq!(tool.active, 0);
    }

    #[test]
    fn the_profile_in_hand_chosen_again_goes_back_to_what_is_saved() {
        let mut scene = one_supported_cube();
        let id = scene.objects()[0].id;
        scene.select(Some(id));
        let mut tool = SupportTool::default();
        let saved = tool.profile.clone();
        tool.profile.middle.diameter_mm += 0.3;
        let mut own = saved.clone();
        own.raft.enabled = !own.raft.enabled;
        scene.objects_mut()[0].own.supports.insert(0, own);

        tool.chosen = Some(saved.clone());
        tool.take_up_chosen(&mut scene);

        assert_eq!(tool.profile, saved);
        assert!(
            scene.objects()[0].own.supports.is_empty(),
            "the picked model goes back with it"
        );
        assert_eq!(tool.groups.len(), 1);
    }

    /// An axis-aligned cube spanning 0..1, twelve triangles.
    fn cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    /// The index of the first face of `mesh` whose normal points the given way.
    fn face_looking(mesh: &Mesh, up: Scalar) -> usize {
        (0..mesh.faces.len())
            .find(|face| {
                mesh.triangle(*face).is_some_and(|triangle| {
                    (triangle.normal_unnormalized().normalize_or_zero().z - up).abs() < 0.1
                })
            })
            .expect("a cube has a face looking each way")
    }

    /// Paints the surface the face `seed` belongs to on the only object of `scene`.
    fn paint_surface(scene: &mut Scene, seed: usize, blocking: bool) {
        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let adjacency = Adjacency::of(&mesh);
        scene.objects_mut()[0]
            .supports
            .paint_face(&mesh, &adjacency, seed, 30.0, blocking, true);
    }

    #[test]
    fn filling_a_painted_patch_stands_supports_under_it() {
        let mut scene = one_cube();
        let underside = face_looking(&scene.objects()[0].mesh, -1.0);
        paint_surface(&mut scene, underside, false);

        let tool = SupportTool {
            fill: ProjectSettings {
                infill_spacing_mm: Some(0.3),
                border_spacing_mm: None,
            },
            ..SupportTool::default()
        };
        let added = tool.fill(&mut scene);

        assert!(
            added > 1,
            "a 1 mm square at 0.3 mm spacing takes several supports, got {added}"
        );
        for point in scene.objects()[0].supports.points() {
            assert!(
                point.contact.z.abs() < 1e-4,
                "{} is not on the underside that was painted",
                point.contact
            );
        }
    }

    #[test]
    fn a_painted_patch_is_drawable_before_the_columns_are_rebuilt() {
        let mut scene = one_cube();
        let underside = face_looking(&scene.objects()[0].mesh, -1.0);
        paint_surface(&mut scene, underside, false);

        let mesh = Arc::clone(&scene.objects()[0].mesh);
        scene.objects_mut()[0]
            .supports
            .refresh_patches(&mesh, Transform::default());

        let (painted, blocked) = scene.objects()[0].supports.patches();
        assert!(painted.is_some(), "the stroke has geometry to draw at once");
        assert!(blocked.is_none(), "nothing was painted out of bounds");
    }

    #[test]
    fn nothing_is_filled_where_the_patch_is_blocked() {
        let mut scene = one_cube();
        let underside = face_looking(&scene.objects()[0].mesh, -1.0);
        paint_surface(&mut scene, underside, false);
        paint_surface(&mut scene, underside, true);

        let tool = SupportTool::default();
        assert_eq!(
            tool.fill(&mut scene),
            0,
            "the painted patch is the blocked patch"
        );
    }

    #[test]
    fn a_support_over_a_blocked_face_steps_off_it() {
        let mut scene = one_cube();
        let lid = face_looking(&scene.objects()[0].mesh, 1.0);
        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let bvh = Bvh::build(&mesh);
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 0);

        let rebuild = |scene: &mut Scene| {
            scene.objects_mut()[0].supports.refresh(
                &mesh,
                &bvh,
                Transform::default(),
                std::slice::from_ref(&SupportProfile::medium()),
            );
            scene.objects()[0].supports.trees()[0].landing().on_model
        };

        assert!(
            rebuild(&mut scene),
            "it stands on the lid of the cube while nothing is blocked"
        );

        paint_surface(&mut scene, lid, true);
        assert!(
            !rebuild(&mut scene),
            "the lid is painted out of bounds, so the support has to reach past it"
        );
    }

    #[test]
    fn only_the_two_brushes_paint() {
        assert!(Placing::Paint.paints());
        assert!(Placing::Block.paints());
        assert!(!Placing::Point.paints());
        assert!(Placing::Erase.paints() && !Placing::Erase.blocks());
    }

    #[test]
    fn a_new_group_starts_from_the_numbers_in_hand_and_takes_the_pill() {
        let mut tool = SupportTool::default();
        tool.profile.middle.diameter_mm = 1.5;
        tool.add_group();

        assert_eq!(tool.active, 1);
        assert_eq!(tool.groups.len(), 2);
        assert!(
            (tool.table()[1].middle.diameter_mm - 1.5).abs() < 1e-6,
            "the new group was struck from what was being edited"
        );
    }

    #[test]
    fn choosing_another_group_keeps_what_was_edited_of_the_one_being_left() {
        let mut tool = SupportTool::default();
        tool.add_group();
        tool.profile.middle.diameter_mm = 2.5;

        tool.choose_group(0);
        assert!(
            (tool.profile.middle.diameter_mm - SupportProfile::default().middle.diameter_mm).abs()
                < 1e-6,
            "the default group's own numbers came back"
        );

        tool.choose_group(1);
        assert!(
            (tool.profile.middle.diameter_mm - 2.5).abs() < 1e-6,
            "the edit made to the second group was kept"
        );
    }

    #[test]
    fn dropping_a_group_takes_its_supports_away_with_it() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.add_group();
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 1);
        assert_eq!(
            tool.group_holds(1, &scene),
            1,
            "the group holds one support"
        );

        assert_eq!(tool.remove_group(1, &mut scene), 1);

        assert_eq!(tool.groups.len(), 1);
        assert_eq!(tool.active, 0);
        assert!(
            scene.objects()[0].supports.points().is_empty(),
            "a support of a group that is gone is gone with it, not handed to another \
             group to be rebuilt in its shape"
        );
    }

    #[test]
    fn a_support_of_another_group_survives_the_one_that_is_dropped() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.add_group();
        let object = &mut scene.objects_mut()[0];
        object
            .supports
            .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 0);
        object
            .supports
            .add(Vec3::new(0.6, 0.6, 5.0), Transform::default(), 1);

        assert_eq!(tool.remove_group(1, &mut scene), 1);
        assert_eq!(
            scene.objects()[0].supports.point_count(),
            1,
            "the default group's own support stayed"
        );
    }

    /// Stands one support under the cube of `scene` and builds what it grows into.
    fn one_support(scene: &mut Scene, tool: &SupportTool) {
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.5, 0.5, 5.0), Transform::default(), 0);
        rebuild(scene, tool);
    }

    fn rebuild(scene: &mut Scene, tool: &SupportTool) {
        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let bvh = Bvh::build(&mesh);
        scene.objects_mut()[0]
            .supports
            .refresh(&mesh, &bvh, Transform::default(), &tool.table());
    }

    #[test]
    fn freezing_a_support_takes_its_point_out_of_the_automatic_run() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        one_support(&mut scene, &tool);

        let frozen = scene.objects_mut()[0]
            .supports
            .freeze(0, Transform::default())
            .expect("the support was standing");
        rebuild(&mut scene, &tool);

        let supports = &scene.objects()[0].supports;
        assert_eq!(frozen, 0);
        assert!(supports.points().is_empty(), "the point went with the tree");
        assert_eq!(supports.frozen().len(), 1);
        assert_eq!(supports.trees().len(), 1, "it is still standing");
        assert_eq!(supports.point_count(), 1, "and still counted");
    }

    #[test]
    fn a_node_carried_aside_stays_there_through_a_rebuild() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        one_support(&mut scene, &tool);
        scene.objects_mut()[0]
            .supports
            .freeze(0, Transform::default())
            .expect("the support was standing");

        let moved = Vec3::new(2.0, 0.5, 5.0);
        scene.objects_mut()[0]
            .supports
            .frozen_mut(0)
            .expect("just frozen")
            .move_node(0, moved);
        rebuild(&mut scene, &tool);

        let standing = scene.objects()[0].supports.trees()[0].nodes()[0].position;
        assert!(
            standing.abs_diff_eq(moved, 1e-4),
            "the node stayed where it was carried, got {standing}"
        );
    }

    #[test]
    fn a_thawed_support_is_grown_again_from_its_tips() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        one_support(&mut scene, &tool);
        scene.objects_mut()[0]
            .supports
            .freeze(0, Transform::default())
            .expect("the support was standing");

        scene.objects_mut()[0]
            .supports
            .thaw(0, Transform::default());
        rebuild(&mut scene, &tool);

        let supports = &scene.objects()[0].supports;
        assert!(supports.frozen().is_empty());
        assert_eq!(supports.points().len(), 1, "its tip became a point again");
        assert_eq!(supports.trees().len(), 1);
    }

    #[test]
    fn the_last_group_cannot_be_dropped() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.remove_group(0, &mut scene);
        assert_eq!(tool.groups.len(), 1, "something has to hold the raft");
    }

    #[test]
    fn the_first_group_goes_and_the_next_takes_its_place() {
        let mut scene = one_supported_cube();
        let transform = scene.objects()[0].transform;
        let mut tool = SupportTool {
            chosen: Some(SupportProfile::heavy()),
            ..SupportTool::default()
        };
        tool.take_up_chosen(&mut scene);
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.2, 0.2, 10.0), transform, 1);

        let gone = tool.remove_group(0, &mut scene);

        assert_eq!(gone, 1, "the first group's one support went with it");
        assert_eq!(tool.groups.len(), 1);
        assert_eq!(tool.groups[0].name, SupportProfile::heavy().name);
        assert_eq!(tool.profile, SupportProfile::heavy());
        assert_eq!(
            scene.objects()[0].supports.group_count(0),
            1,
            "what stood in the second group now stands in the first"
        );
    }

    #[test]
    fn a_group_is_rebuilt_to_another_profile_and_named_for_it() {
        let mut scene = one_supported_cube();
        let mut tool = SupportTool::default();
        let mut own = tool.profile.clone();
        own.raft.enabled = !own.raft.enabled;
        scene.objects_mut()[0].own.supports.insert(0, own);

        tool.retune_group(0, SupportProfile::heavy(), &mut scene);

        assert_eq!(tool.table(), vec![SupportProfile::heavy()]);
        assert_eq!(tool.groups[0].name, SupportProfile::heavy().name);
        assert!(scene.objects()[0].own.supports.is_empty());
        assert_eq!(
            scene.objects()[0].supports.group_count(0),
            1,
            "the support stays, in the group it was in"
        );
    }

    #[test]
    fn two_groups_stand_two_thicknesses_on_one_model() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.add_group();
        tool.profile.middle.diameter_mm *= 2.0;
        tool.groups[1].profile = tool.profile.clone();

        let object = &mut scene.objects_mut()[0];
        object
            .supports
            .add(Vec3::new(0.2, 0.5, 5.0), Transform::default(), 0);
        object
            .supports
            .add(Vec3::new(0.8, 0.5, 5.0), Transform::default(), 1);

        let mesh = Arc::clone(&scene.objects()[0].mesh);
        let bvh = Bvh::build(&mesh);
        scene.objects_mut()[0]
            .supports
            .refresh(&mesh, &bvh, Transform::default(), &tool.table());

        let groups: Vec<u16> = scene.objects()[0]
            .supports
            .trees()
            .iter()
            .map(SupportTree::group)
            .collect();
        assert_eq!(
            groups,
            vec![0, 1],
            "one tree per group, each to its own shape"
        );
    }

    #[test]
    fn lifting_stands_a_model_its_lift_clear_of_the_plate() {
        let mut scene = one_cube();
        let tool = SupportTool::default();

        assert_eq!(tool.lift(&mut scene), 1);
        let bounds = scene.objects()[0]
            .world_bounds()
            .expect("the cube has bounds");
        assert!(
            (bounds.mins.z - tool.profile.z_lift_mm).abs() < 1e-4,
            "the lowest point ends up at the lift, got {}",
            bounds.mins.z
        );
    }

    #[test]
    fn a_raft_stands_a_supported_model_up_by_its_thickness_and_back_down_without_it() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.lift(&mut scene);
        let bottom = |scene: &Scene| {
            scene.objects()[0]
                .world_bounds()
                .expect("a cube has bounds")
                .mins
                .z
        };
        let lifted = bottom(&scene);

        tool.profile.raft.enabled = true;
        tool.settle_on_raft(&mut scene);
        assert!(
            (bottom(&scene) - lifted).abs() < 1e-5,
            "a model with no supports has no raft under it"
        );

        let transform = scene.objects()[0].transform;
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.5, 0.5, lifted), transform, 0);
        tool.settle_on_raft(&mut scene);
        tool.settle_on_raft(&mut scene);
        assert!(
            (bottom(&scene) - lifted - tool.profile.raft.thickness_mm).abs() < 1e-5,
            "the slab must not be printed into the part, and asking twice lifts once"
        );
        assert_eq!(
            tool.lift(&mut scene),
            0,
            "the lift is measured over the raft, so a model already there stays"
        );

        tool.profile.raft.enabled = false;
        tool.settle_on_raft(&mut scene);
        assert!((bottom(&scene) - lifted).abs() < 1e-5, "and back down");
    }

    #[test]
    fn lifting_a_model_that_is_already_there_moves_nothing() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        tool.lift(&mut scene);
        assert_eq!(
            tool.lift(&mut scene),
            0,
            "a second lift must not stack on the first"
        );
    }

    #[test]
    fn a_hidden_model_is_not_lifted() {
        let mut scene = one_cube();
        scene.objects_mut()[0].visible = false;
        assert_eq!(SupportTool::default().lift(&mut scene), 0);
    }

    /// A unit cube held 5 mm over the plate: its underside, the face at z = 5, is an
    /// island on the layer that starts there.
    fn floating_cube() -> Scene {
        let mut scene = one_cube();
        scene.objects_mut()[0].transform.translation.z = 5.0;
        scene
    }

    #[test]
    fn an_island_gets_a_support_on_its_underside_in_a_group_of_its_own() {
        let mut scene = floating_cube();
        let mut tool = SupportTool::default();
        let floor = Vec3::new(0.5, 0.5, 5.0);

        assert_eq!(tool.support_islands(&mut scene, &[floor], 0.05), 1);
        assert_eq!(tool.groups.len(), 2);
        assert_eq!(tool.groups[1].name, ISLANDS_GROUP);
        assert_eq!(tool.active, 0, "the group in hand stays in hand");
        let point = scene.objects()[0].supports.points()[0];
        assert_eq!(point.group, 1);
        let contact = scene.objects()[0]
            .transform
            .to_matrix()
            .transform_point3(point.contact);
        assert!(
            (contact - floor).length() < 1e-4,
            "the cube's bottom face is straight over the floor, got {contact}"
        );

        tool.support_islands(&mut scene, &[floor], 0.05);
        assert_eq!(tool.groups.len(), 2, "a second press joins the same group");
    }

    #[test]
    fn an_island_with_nothing_over_its_centre_stands_nothing_and_adds_no_group() {
        let mut scene = floating_cube();
        let mut tool = SupportTool::default();
        let beside = Vec3::new(3.0, 0.5, 5.0);
        let far_under = Vec3::new(0.5, 0.5, 2.0);

        assert_eq!(
            tool.support_islands(&mut scene, &[beside, far_under], 0.05),
            0
        );
        assert_eq!(tool.groups.len(), 1);
    }
}
