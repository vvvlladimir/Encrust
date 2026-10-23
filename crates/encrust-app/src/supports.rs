use anyhow::{Context, Result, bail};
use core_engine::project::{AutoSizing, FillProfiles, own_table};
use core_geometry::{Adjacency, Ray, Scalar, Vec2, Vec3, raycast_placed};
use core_supports::{Part, Placed, ProjectSettings, Projected, SupportTree, project};
use printer_profiles::{AUTO_SIZES, AutoProfile, Catalogue, Raft, SupportProfile};
use serde::{Deserialize, Serialize};

/// How many shapes an automatic run builds to: its sizes, and the one that steadies a part.
const SHAPES: usize = AUTO_SIZES + 1;

/// How near a support has to stand to where the last one was placed to start a line from
/// it, millimetres: both are the same point put through the same matrix.
const LINE_START_MM: Scalar = 1e-3;

use crate::i18n::tr;
use crate::job::{Placement, SupportJob, SupportOutcome, SupportRequest, tasks_of};
use crate::scene::{ObjectId, OwnSettings, Scene, SceneObject};
use crate::status::Status;

/// What the one group a plate starts with is called until a profile is chosen for it.
const FIRST_GROUP: &str = "Default";

/// The group the Check tool stands island supports in, apart from the ones built by hand.
pub const ISLANDS_GROUP: &str = "Islands";

/// Whether a tree a hand has frozen stands inside the model's cavity: every tip of it does.
fn frozen_inside(object: &SceneObject, tree: &SupportTree) -> bool {
    let mut contacts = tree.contacts().peekable();
    contacts.peek().is_some() && contacts.all(|contact| object.holds_inside(contact))
}

/// Which side of a hollow model's wall an automatic run holds up: everything outside it,
/// or only its cavity; see `docs/decisions/0334`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Outside,
    Inside,
}

/// What a click on the model does while the Paint tool is in hand: stand one support,
/// paint the patch a fill covers, paint the patch supports keep out of, or take either
/// paint off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placing {
    #[default]
    Point,
    Paint,
    Block,
    Erase,
}

impl Placing {
    pub fn label(self) -> String {
        match self {
            Self::Point => tr!("supports-placing-place"),
            Self::Paint => tr!("supports-placing-fill"),
            Self::Block => tr!("supports-placing-block"),
            Self::Erase => tr!("supports-placing-erase"),
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
    /// The saved profiles a fill builds its rim and its inside to, or the group in hand.
    pub fill_profiles: FillProfiles,
    /// Which side of a hollow model's wall the run going holds up.
    side: Side,
    /// How far apart a line of supports drawn with shift stands, millimetres.
    pub inline_spacing_mm: Scalar,
    /// The support Place last put down, in its model's own space: where a line drawn with
    /// shift starts from. See `docs/decisions/0332`.
    pub last_placed: Option<(ObjectId, Vec3)>,
    /// The brush has hold of the pointer: the press that started this drag landed on a
    /// model, so the stroke paints instead of turning the camera.
    pub stroke: bool,
    /// Where the brush was last put down, in panel points, so that a fast drag is painted
    /// as the line it drew rather than as the frames it was sampled on.
    pub last_daub: Option<Vec2>,
    pub job: Option<SupportJob>,
    /// The automatic profile the panel's list has chosen in place of a support profile,
    /// or `None` while a run places to the group in hand. See `docs/decisions/0325`.
    pub auto: Option<AutoProfile>,
    /// The automatic profile set aside while supports are built to the group in hand, so
    /// that sizing turned back on finds its numbers as they were left.
    parked_auto: Option<AutoProfile>,
    /// What the run going sizes its supports by, and the shapes it sizes them into.
    sizing: Option<(AutoProfile, [SupportProfile; SHAPES])>,
    /// The raft every model stands on that has none of its own. A raft is one slab under
    /// a part, so it is the part's and the plate's, not any one group's; see ADR 0253.
    pub raft: Raft,
    /// A profile the panel has just saved, until the plate has taken it on.
    pub saved: Option<SupportProfile>,
    /// A profile the panel's list has just had chosen, until a group has taken it up.
    pub chosen: Option<SupportProfile>,
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
            fill_profiles: FillProfiles::default(),
            side: Side::Outside,
            inline_spacing_mm: core_supports::INLINE_SPACING_MM,
            last_placed: None,
            stroke: false,
            last_daub: None,
            job: None,
            auto: None,
            parked_auto: None,
            sizing: None,
            raft: SupportProfile::default().raft,
            saved: None,
            chosen: None,
        }
    }
}

impl SupportTool {
    /// Why the Generate button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<String> {
        if scene.target_count() == 0 {
            return Some(tr!("supports-nothing-visible"));
        }
        None
    }

    /// Where a line of supports drawn on object `id` starts, in plate coordinates: the last
    /// support placed, if it was placed on that model and still stands there. One undone,
    /// taken away or left on a scene since replaced is no start.
    pub fn line_start(&self, scene: &Scene, id: ObjectId) -> Option<Vec3> {
        let (on, contact) = self.last_placed?;
        let object = scene.get(id).filter(|_| on == id)?;
        let supports = &object.supports;
        let mut standing = supports
            .points()
            .iter()
            .map(|point| point.contact)
            .chain(supports.frozen().iter().flat_map(SupportTree::contacts));
        standing
            .any(|at| at.distance_squared(contact) < LINE_START_MM * LINE_START_MM)
            .then(|| object.transform.to_matrix().transform_point3(contact))
    }

    /// Sizes supports by what they hold, or builds them to the group in hand. Turned on,
    /// it takes up the automatic profile last set aside, else the first `catalogue` ships.
    pub fn set_automatic(&mut self, on: bool, catalogue: &Catalogue) {
        if on == self.auto.is_some() {
            return;
        }
        if on {
            self.auto = self
                .parked_auto
                .take()
                .or_else(|| catalogue.autos().next().map(|entry| entry.profile.clone()));
        } else {
            self.parked_auto = self.auto.take();
        }
    }

    /// The automatic profile in hand or set aside, as a project and the undo stack keep it.
    pub fn sizing(&self) -> Option<AutoSizing> {
        let (on, profile) = match (&self.auto, &self.parked_auto) {
            (Some(profile), _) => (true, profile),
            (None, Some(profile)) => (false, profile),
            (None, None) => return None,
        };
        Some(AutoSizing {
            on,
            profile: profile.clone(),
        })
    }

    /// Takes up automatic sizing as `sizing` left it, or none chosen at all.
    pub fn set_sizing(&mut self, sizing: Option<AutoSizing>) {
        (self.auto, self.parked_auto) = match sizing {
            Some(AutoSizing { on: true, profile }) => (Some(profile), None),
            Some(AutoSizing { on: false, profile }) => (None, Some(profile)),
            None => (None, None),
        };
    }

    /// Starts an automatic placement run over everything visible on the plate: by the
    /// group in hand, or by the automatic profile chosen, whose shapes are read from
    /// `catalogue`.
    ///
    /// The run cuts each model at the height it will be printed at, so the layers it
    /// looks at are the layers the printer will expose.
    ///
    /// Run for the inside, it holds up only the cavities of hollow models, taking what
    /// stood in them away first.
    pub fn start(
        &mut self,
        scene: &mut Scene,
        layer_height_mm: Scalar,
        catalogue: &Catalogue,
        side: Side,
    ) -> Result<()> {
        self.sizing = self
            .auto
            .as_ref()
            .map(|auto| Ok::<_, anyhow::Error>((auto.clone(), shapes_of(auto, catalogue)?)))
            .transpose()?;
        if side == Side::Inside {
            Self::clear_inside(scene);
            // A frozen tree taken away renumbers the ones after it.
            self.picked.clear();
        }
        let mut tasks = match &self.sizing {
            Some((auto, shapes)) => tasks_of(scene, |_| auto.placing_with(&shapes[0])),
            None => tasks_of(scene, |object| self.profile_for(object).clone()),
        };
        for task in &mut tasks {
            let object = scene.get(task.id);
            task.side = object.and_then(|object| object.wall_side(side == Side::Inside));
        }
        if side == Side::Inside {
            tasks.retain(|task| task.side.is_some());
        }
        if tasks.is_empty() {
            bail!(tr!("supports-nothing-visible-error"));
        }
        self.side = side;

        self.job = Some(SupportJob::spawn(SupportRequest {
            tasks,
            layer_height_mm,
            sizing: self.sizing.as_ref().map(|(auto, _)| auto.clone()),
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
        let outcome = match outcome {
            SupportOutcome::Placed(placements) => {
                SupportOutcome::Placed(self.on_its_side(placements, scene))
            }
            other => other,
        };
        let added = outcome.count();
        match outcome {
            SupportOutcome::Placed(placements) => match self.sizing.take() {
                Some((auto, shapes)) => {
                    let mut sized = [0usize; SHAPES];
                    for need in placements.iter().flat_map(|placement| &placement.needs) {
                        sized[need.size_in(&auto)] += 1;
                    }
                    let mut shapes = shapes.map(Some);
                    for (shape, count) in shapes.iter_mut().zip(sized) {
                        if count == 0 {
                            *shape = None;
                        }
                    }
                    let groups = self.groups_for(shapes, scene);
                    for placement in placements {
                        let Some(object) = scene.get_mut(placement.id) else {
                            continue;
                        };
                        for need in placement.needs {
                            let group = groups[need.size_in(&auto)];
                            object
                                .supports
                                .add(need.contact, placement.transform, group);
                        }
                    }
                    *status = Status::Info(tr!(
                        "supports-placed-sized",
                        count = added,
                        light = sized[0],
                        medium = sized[1],
                        heavy = sized[2],
                        steady = sized[3]
                    ));
                }
                None => {
                    let group = self.active;
                    for placement in placements {
                        let Some(object) = scene.get_mut(placement.id) else {
                            continue;
                        };
                        for need in placement.needs {
                            object
                                .supports
                                .add(need.contact, placement.transform, group);
                        }
                    }
                    *status = Status::Info(tr!("supports-placed", count = added));
                }
            },
            SupportOutcome::Cancelled => *status = Status::Info(tr!("supports-cancelled")),
            SupportOutcome::Failed(message) => *status = Status::Error(message),
        }
        false
    }

    /// The group each of `shapes` that is wanted is placed into: the one already built to
    /// a profile of its name, else one nothing stands in, retuned to it, else a new one.
    /// A shape nothing is sized into is `None`, and is given no group.
    fn groups_for(
        &mut self,
        shapes: [Option<SupportProfile>; SHAPES],
        scene: &mut Scene,
    ) -> [u16; SHAPES] {
        let table = self.table();
        let named = shapes.each_ref().map(|shape| {
            let shape = shape.as_ref()?;
            let group = table.iter().position(|kind| kind.name == shape.name)?;
            Some(group as u16)
        });
        let mut taken: Vec<u16> = named.iter().flatten().copied().collect();
        let mut groups = [0u16; SHAPES];
        for ((group, shape), named) in groups.iter_mut().zip(shapes).zip(named) {
            *group = match (shape, named) {
                (_, Some(group)) => group,
                (Some(shape), None) => self.group_to_build(shape, &taken, scene),
                (None, None) => continue,
            };
            taken.push(*group);
        }
        groups
    }

    /// A group for `shape` none of `taken` is: one nothing stands in, retuned to it, else
    /// a new one.
    fn group_to_build(&mut self, shape: SupportProfile, taken: &[u16], scene: &mut Scene) -> u16 {
        let empty = (0..self.groups.len() as u16).find(|group| {
            !taken.contains(group)
                && self.groups[*group as usize].name != ISLANDS_GROUP
                && self.group_holds(*group, scene) == 0
        });
        if let Some(group) = empty {
            self.retune_group(group, shape, scene);
            return group;
        }
        self.groups.push(SupportGroup {
            name: shape.name.clone(),
            profile: shape,
            hidden: false,
        });
        (self.groups.len() - 1) as u16
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
        with_own(self.table(), &object.own, self.raft)
    }

    /// The raft the panel shows: the first picked model's own where it keeps one, the
    /// plate's otherwise.
    pub fn raft_shown(&self, scene: &Scene) -> Raft {
        own_raft_shown(scene).unwrap_or(self.raft)
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

    /// The active group's profile as `object` is built to, with the raft it stands on, its
    /// own or the plate's: what orienting it weighs its supports by.
    pub fn built_for(&self, object: &SceneObject) -> SupportProfile {
        let mut profile = self.profile_for(object).clone();
        profile.raft = object.own.raft.unwrap_or(self.raft);
        profile
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
            name: tr!("supports-group-name", index = self.groups.len() + 1),
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

    /// What a run found, each placement keeping only the contacts on the side of the wall
    /// the run was started for. A contact is inside when the surface nearest it bounds the
    /// cavity (ADR 0334).
    fn on_its_side(&self, mut placements: Vec<Placement>, scene: &Scene) -> Vec<Placement> {
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
    /// put down. The rim and the inside each go into the group of the profile chosen for
    /// them, made for it when there is none, or into the group in hand (ADR 0333).
    ///
    /// The face adjacency the rim is walked over is built here and thrown away: a fill is
    /// one press of a button, and keeping it beside every model would cost the memory of
    /// a third mesh for something a plate full of models asks for once.
    pub fn fill(&mut self, scene: &mut Scene, catalogue: &Catalogue) -> usize {
        let filled: Vec<(ObjectId, Projected)> = scene
            .targets()
            .filter(|object| !object.supports.painted().is_empty())
            .map(|object| {
                let keep_out = object.supports.keep_out(&object.mesh, object.transform);
                // On the model the patch is painted on: a fill reads its faces by number.
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
            true => self.fill_group(
                self.fill_profiles.inside.clone(),
                &[rim_group],
                catalogue,
                scene,
            ),
            false => self.active,
        };

        let mut added = 0;
        for (id, projected) in filled {
            let Some(object) = scene.get_mut(id) else {
                continue;
            };
            let transform = object.transform;
            let parts = [(projected.rim, rim_group), (projected.inside, inside_group)];
            for (contacts, group) in parts {
                for contact in contacts {
                    object.supports.add(contact, transform, group);
                    added += 1;
                }
            }
        }
        added
    }

    /// The group one part of a fill goes into: the group in hand, or the one built to the
    /// saved profile `id`, else one made for it that is none of `taken`.
    fn fill_group(
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

/// The support profiles `auto` names, lightest first and the steadying one last, as
/// `catalogue` has them.
fn shapes_of(auto: &AutoProfile, catalogue: &Catalogue) -> Result<[SupportProfile; SHAPES]> {
    let [light, medium, heavy] = auto.shapes.each_ref();
    let ids = [light, medium, heavy, &auto.steady_shape];
    let [light, medium, heavy, steady] = ids.map(|id| {
        let entry = catalogue.support(id).with_context(|| {
            tr!(
                "supports-auto-no-shape",
                name = auto.name.clone(),
                id = id.clone()
            )
        })?;
        Ok::<_, anyhow::Error>(entry.profile.clone())
    });
    Ok([light?, medium?, heavy?, steady?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Bvh, Mesh, Transform};
    use core_supports::SupportTree;
    use core_supports::{Need, Reason};
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

    /// Runs placement with the shipped automatic profile under `scene` to its end.
    fn grown_by_auto(scene: &mut Scene) -> (SupportTool, Status) {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let mut tool = SupportTool {
            auto: catalogue.autos().next().map(|entry| entry.profile.clone()),
            ..SupportTool::default()
        };
        tool.start(scene, 0.05, &catalogue, Side::Outside)
            .expect("a model to place under");
        let mut status = Status::default();
        while tool.poll(scene, &mut status) {
            std::thread::yield_now();
        }
        (tool, status)
    }

    #[test]
    fn a_small_island_is_held_by_the_lightest_shape_in_the_empty_group() {
        let mut scene = one_cube();
        scene.objects_mut()[0].transform.translation.z = 10.0;
        let (tool, status) = grown_by_auto(&mut scene);

        assert!(
            matches!(status, Status::Info(_)),
            "the run ends placed: {status:?}"
        );
        assert_eq!(
            tool.groups.len(),
            1,
            "no group for a shape nothing is sized into"
        );
        assert_eq!(
            tool.groups[0].profile.name, "Auto Light",
            "the empty first group is retuned to the shape a 1 mm² island takes"
        );
        let object = &scene.objects()[0];
        assert!(object.supports.point_count() > 0);
        assert_eq!(
            object.supports.group_count(0),
            object.supports.point_count()
        );
    }

    #[test]
    fn sizing_turned_off_and_on_again_keeps_the_numbers_it_was_left_with() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let mut tool = SupportTool::default();
        tool.set_automatic(true, &catalogue);
        let auto = tool
            .auto
            .as_mut()
            .expect("the shipped automatic profile is taken up");
        auto.density = 2.0;
        tool.set_automatic(false, &catalogue);
        assert!(tool.auto.is_none());
        tool.set_automatic(true, &catalogue);
        assert_eq!(tool.auto.map(|auto| auto.density), Some(2.0));
    }

    #[test]
    fn a_missing_shape_stops_the_run_before_it_starts() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let mut auto = catalogue
            .autos()
            .next()
            .map(|entry| entry.profile.clone())
            .expect("an automatic profile ships");
        auto.shapes[2] = "no-such-shape".to_owned();
        let mut tool = SupportTool {
            auto: Some(auto),
            ..SupportTool::default()
        };
        assert!(
            tool.start(&mut one_cube(), 0.05, &catalogue, Side::Outside)
                .is_err()
        );
        assert!(tool.job.is_none());
    }

    #[test]
    fn a_model_is_oriented_by_the_overhang_of_its_own_profile() {
        let mut scene = one_cube();
        let tool = SupportTool::default();
        let mut own = tool.profile.clone();
        own.max_overhang_deg += 10.0;
        let wanted = own.max_overhang_deg;
        scene.objects_mut()[0].own.supports.insert(tool.active, own);

        let tasks =
            crate::job::orient_tasks(&scene, None, |object| tool.profile_for(object).clone());
        assert!(
            (tasks[0].support.max_overhang_deg - wanted).abs() < f32::EPSILON,
            "not the angle of the plate's profile"
        );
    }

    #[test]
    fn a_model_is_oriented_on_the_raft_it_stands_on() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        tool.raft.enabled = true;

        let tasks = crate::job::orient_tasks(&scene, None, |object| tool.built_for(object));
        assert!(tasks[0].support.raft.enabled, "the plate's raft is weighed");

        let own = Raft {
            enabled: false,
            ..tool.raft
        };
        scene.objects_mut()[0].own.raft = Some(own);
        let tasks = crate::job::orient_tasks(&scene, None, |object| tool.built_for(object));
        assert!(!tasks[0].support.raft.enabled, "its own raft wins");
    }

    #[test]
    fn another_profile_starts_a_kind_of_its_own_and_leaves_what_stands_alone() {
        let mut scene = one_supported_cube();
        let mut tool = SupportTool::default();
        let first = tool.profile.clone();
        let other = SupportProfile::shipped("heavy").expect("the heavy preset ships");
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
        let other = SupportProfile::shipped("heavy").expect("the heavy preset ships");

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
        tool.chosen = Some(SupportProfile::shipped("heavy").expect("the heavy preset ships"));
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

    /// A raft unlike the default one: switched the other way and a millimetre thicker.
    fn another_raft(tool: &SupportTool) -> Raft {
        Raft {
            enabled: !tool.raft.enabled,
            thickness_mm: tool.raft.thickness_mm + 1.0,
            ..tool.raft
        }
    }

    #[test]
    fn a_raft_edited_with_a_model_picked_is_the_plates() {
        let mut scene = one_cube();
        scene.select(Some(scene.objects()[0].id));
        let mut tool = SupportTool::default();
        let edited = another_raft(&tool);

        tool.keep_raft(edited, &mut scene);

        assert_eq!(tool.raft, edited, "the plate's, so the next run keeps it");
        assert_eq!(scene.objects()[0].own.raft, None, "and not the model's own");
    }

    #[test]
    fn a_models_own_raft_is_edited_in_place_and_dropped_at_the_plates() {
        let mut scene = one_cube();
        let first = scene.objects()[0].id;
        scene.select(Some(first));
        let mut tool = SupportTool::default();
        let (plate, own) = (tool.raft, another_raft(&tool));
        scene.get_mut(first).expect("there").own.raft = Some(own);
        let thicker = Raft {
            thickness_mm: own.thickness_mm + 1.0,
            ..own
        };

        tool.keep_raft(thicker, &mut scene);
        assert_eq!(scene.get(first).expect("there").own.raft, Some(thicker));
        assert_eq!(tool.raft, plate, "a project's own raft leaves the plate's");

        tool.keep_raft(plate, &mut scene);
        assert_eq!(
            scene.get(first).expect("there").own.raft,
            None,
            "a raft that is the plate's again is no longer the model's own"
        );
    }

    #[test]
    fn a_raft_edited_with_nothing_picked_is_the_plates() {
        let mut scene = one_cube();
        scene.select(None);
        let mut tool = SupportTool::default();
        let edited = another_raft(&tool);

        tool.keep_raft(edited, &mut scene);

        assert_eq!(tool.raft, edited);
        assert_eq!(tool.table_for(&scene.objects()[0])[0].raft, edited);
    }

    #[test]
    fn another_profile_leaves_the_raft_as_it_was() {
        let mut scene = one_supported_cube();
        let mut tool = SupportTool::default();
        let raft = another_raft(&tool);
        tool.raft = raft;
        assert_ne!(
            SupportProfile::shipped("heavy")
                .expect("the heavy preset ships")
                .raft,
            raft
        );

        tool.chosen = Some(SupportProfile::shipped("heavy").expect("the heavy preset ships"));
        tool.take_up_chosen(&mut scene);
        tool.retune_group(
            0,
            SupportProfile::shipped("light").expect("the light preset ships"),
            &mut scene,
        );

        assert_eq!(
            tool.table_for(&scene.objects()[0])[0].raft,
            raft,
            "the raft is the plate's, whatever the groups are built to"
        );
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

        let mut tool = SupportTool {
            fill: ProjectSettings {
                infill_spacing_mm: Some(0.3),
                border_spacing_mm: None,
                infill_offset_mm: 0.0,
                ..ProjectSettings::default()
            },
            ..SupportTool::default()
        };
        let added = tool.fill(&mut scene, &Catalogue::default());

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
    fn a_fill_builds_its_rim_and_its_inside_to_the_profiles_chosen_for_them() {
        let mut scene = one_cube();
        let underside = face_looking(&scene.objects()[0].mesh, -1.0);
        paint_surface(&mut scene, underside, false);
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let manual: Vec<&printer_profiles::Entry<SupportProfile>> = catalogue
            .supports()
            .filter(|entry| !catalogue.is_auto_shape(&entry.id))
            .collect();
        let [rim, inside, ..] = manual.as_slice() else {
            panic!("the shipped catalogue has at least two profiles to build by hand");
        };

        let mut tool = SupportTool {
            fill: ProjectSettings {
                infill_spacing_mm: Some(0.3),
                border_spacing_mm: Some(0.3),
                border_offset_mm: 0.0,
                infill_offset_mm: 0.0,
                ..ProjectSettings::default()
            },
            fill_profiles: FillProfiles {
                rim: Some(rim.id.clone()),
                inside: Some(inside.id.clone()),
            },
            ..SupportTool::default()
        };
        tool.fill(&mut scene, &catalogue);

        let group_of = |name: &str| {
            tool.groups
                .iter()
                .position(|group| group.profile.name == name)
                .unwrap_or_else(|| panic!("no group was made for {name}")) as u16
        };
        let (rim_group, inside_group) =
            (group_of(&rim.profile.name), group_of(&inside.profile.name));
        assert_ne!(rim_group, tool.active, "the group in hand was not retuned");
        assert_ne!(
            inside_group, tool.active,
            "the group in hand was not retuned"
        );
        let points = scene.objects()[0].supports.points();
        assert!(
            points.iter().any(|point| point.group == rim_group),
            "nothing in the rim's group"
        );
        assert!(
            points.iter().any(|point| point.group == inside_group),
            "nothing in the inside's group"
        );
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

        let mut tool = SupportTool::default();
        assert_eq!(
            tool.fill(&mut scene, &Catalogue::default()),
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
            .add(Vec3::new(0.5, 0.5, 8.0), Transform::default(), 0);

        let rebuild = |scene: &mut Scene| {
            scene.objects_mut()[0].supports.refresh(
                &mesh,
                &bvh,
                Transform::default(),
                std::slice::from_ref(
                    &SupportProfile::shipped("medium").expect("the medium preset ships"),
                ),
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
            chosen: Some(SupportProfile::shipped("heavy").expect("the heavy preset ships")),
            ..SupportTool::default()
        };
        tool.take_up_chosen(&mut scene);
        scene.objects_mut()[0]
            .supports
            .add(Vec3::new(0.2, 0.2, 10.0), transform, 1);

        let gone = tool.remove_group(0, &mut scene);

        assert_eq!(gone, 1, "the first group's one support went with it");
        assert_eq!(tool.groups.len(), 1);
        assert_eq!(
            tool.groups[0].name,
            SupportProfile::shipped("heavy")
                .expect("the heavy preset ships")
                .name
        );
        assert_eq!(
            tool.profile,
            SupportProfile::shipped("heavy").expect("the heavy preset ships")
        );
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

        tool.retune_group(
            0,
            SupportProfile::shipped("heavy").expect("the heavy preset ships"),
            &mut scene,
        );

        assert_eq!(
            tool.table(),
            vec![SupportProfile::shipped("heavy").expect("the heavy preset ships")]
        );
        assert_eq!(
            tool.groups[0].name,
            SupportProfile::shipped("heavy")
                .expect("the heavy preset ships")
                .name
        );
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
    fn a_raft_stands_a_supported_model_up_by_its_thickness_and_back_down_without_it() {
        let mut scene = one_cube();
        let mut tool = SupportTool::default();
        scene.objects_mut()[0].lift_to(tool.profile.z_lift_mm);
        let bottom = |scene: &Scene| {
            scene.objects()[0]
                .world_bounds()
                .expect("a cube has bounds")
                .mins
                .z
        };
        let lifted = bottom(&scene);

        tool.raft.enabled = true;
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
            (bottom(&scene) - lifted - tool.raft.thickness_mm).abs() < 1e-5,
            "the slab must not be printed into the part, and asking twice lifts once"
        );
        let over_raft = scene.objects()[0].lift().expect("a cube has bounds");
        assert!(
            (over_raft - tool.profile.z_lift_mm).abs() < 1e-5,
            "the lift is measured over the raft, got {over_raft}"
        );

        tool.raft.enabled = false;
        tool.settle_on_raft(&mut scene);
        assert!((bottom(&scene) - lifted).abs() < 1e-5, "and back down");
    }

    #[test]
    fn a_rafted_model_put_back_on_the_plate_is_not_sunk_into_it_when_its_supports_go() {
        let mut scene = one_supported_cube();
        let mut tool = SupportTool::default();
        tool.raft.enabled = true;
        tool.settle_on_raft(&mut scene);

        scene.objects_mut()[0].stand_on_plate();
        tool.settle_on_raft(&mut scene);
        scene.objects_mut()[0].supports.clear();
        tool.settle_on_raft(&mut scene);

        let bottom = scene.objects()[0]
            .world_bounds()
            .expect("a cube has bounds")
            .mins
            .z;
        assert!(
            bottom > -1e-5,
            "a model stood on the plate owes the raft nothing, got its bottom at {bottom} mm"
        );
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
    /// The cube 20 mm a side, 5 mm over the plate, hollowed to a cavity 12 mm a side in its
    /// middle: the shell is the cube with that box appended wound inward.
    fn hollow_cube() -> Scene {
        let mut scene = one_cube();
        let object = &mut scene.objects_mut()[0];
        object.transform = Transform {
            translation: Vec3::new(0.0, 0.0, 5.0),
            scale: Vec3::splat(20.0),
            ..Transform::default()
        };
        let mut shell = cube();
        let inner = cube();
        let offset = shell.vertices.len() as u32;
        shell
            .vertices
            .extend(inner.vertices.iter().map(|at| *at * 0.6 + Vec3::splat(0.2)));
        shell.faces.extend(
            inner
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
        );
        let settings = object
            .hollow
            .asking(&core_volume::HollowSettings::for_wall(4.0));
        object.hollow.take(core_volume::Shell {
            mesh: Arc::new(shell),
            cavity: 12..24,
            cavity_mm3: 1728.0,
            voxel_mm: 0.1,
            coarsened: false,
            shape: object.transform.unturned(),
            settings,
        });
        assert!(
            object.keep_support_surface(),
            "the shell begins with the cube"
        );
        scene
    }

    /// A run's placement under the hollow cube: one contact under the cavity's ceiling,
    /// 21 mm up, and one under the cube's own underside at 5 mm.
    fn inside_and_out(scene: &Scene) -> Vec<Placement> {
        let object = &scene.objects()[0];
        let need = |z: Scalar| Need {
            contact: Vec3::new(10.0, 10.0, z),
            reason: Reason::Steady,
        };
        vec![Placement {
            id: object.id,
            transform: object.transform,
            needs: vec![need(20.95), need(4.95)],
        }]
    }

    fn needs_at(placements: &[Placement]) -> Vec<Scalar> {
        placements[0]
            .needs
            .iter()
            .map(|need| need.contact.z)
            .collect()
    }

    #[test]
    fn a_run_keeps_only_the_contacts_on_its_side_of_the_wall() {
        let scene = hollow_cube();
        let tool = SupportTool {
            side: Side::Inside,
            ..SupportTool::default()
        };
        let inside = tool.on_its_side(inside_and_out(&scene), &scene);
        assert_eq!(
            needs_at(&inside),
            [20.95],
            "only the cavity's ceiling is held from inside"
        );

        let tool = SupportTool::default();
        let outside = tool.on_its_side(inside_and_out(&scene), &scene);
        assert_eq!(
            needs_at(&outside),
            [4.95],
            "only the underside is held from outside"
        );
    }

    /// A box `min`..`max` from the unit cube, wound inward when `inward`.
    fn box_of(min: Vec3, max: Vec3, inward: bool) -> Mesh {
        let unit = cube();
        let vertices = unit.vertices.iter().map(|at| min + *at * (max - min));
        let faces = unit
            .faces
            .iter()
            .map(|&[a, b, c]| if inward { [a, c, b] } else { [a, b, c] });
        Mesh::new(vertices.collect(), faces.collect())
    }

    fn joined(meshes: &[Mesh]) -> Mesh {
        let mut all = Mesh::new(Vec::new(), Vec::new());
        for mesh in meshes {
            let offset = all.vertices.len() as u32;
            all.vertices.extend(&mesh.vertices);
            all.faces
                .extend(mesh.faces.iter().map(|face| face.map(|at| at + offset)));
        }
        all
    }

    /// A 4 mm slab tilted 50° about Y and lifted 25 mm, and, when `hollow`, a cavity in it
    /// 0.6 mm in from every face: its ceiling runs along the slab's leaning underside.
    fn tilted_slab(hollow: bool) -> Scene {
        let solid = box_of(Vec3::ZERO, Vec3::new(20.0, 10.0, 4.0), false);
        let mut scene = one_cube();
        let object = &mut scene.objects_mut()[0];
        object.mesh = Arc::new(solid.clone());
        object.bvh = Arc::new(core_geometry::Bvh::build(&solid));
        object.transform.rotation = core_geometry::Quat::from_rotation_y(50f32.to_radians());
        object.transform.translation.z = 25.0;
        if hollow {
            let cavity = box_of(Vec3::splat(0.6), Vec3::new(19.4, 9.4, 3.4), true);
            let shell = joined(&[solid.clone(), cavity]);
            let settings = object
                .hollow
                .asking(&core_volume::HollowSettings::for_wall(1.0));
            object.hollow.take(core_volume::Shell {
                cavity: solid.faces.len()..shell.faces.len(),
                mesh: Arc::new(shell),
                cavity_mm3: 18.8 * 8.8 * 2.8,
                voxel_mm: 0.1,
                coarsened: false,
                shape: object.transform.unturned(),
                settings,
            });
            assert!(object.keep_support_surface());
        }
        scene
    }

    /// Runs `sides` one after the other over `scene`.
    fn grown(scene: &mut Scene, sides: &[Side]) {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let mut tool = SupportTool::default();
        for side in sides {
            tool.start(scene, 0.05, &catalogue, *side)
                .expect("a model to hold up");
            let mut status = Status::default();
            while tool.poll(scene, &mut status) {
                std::thread::yield_now();
            }
        }
    }

    /// The contacts `sides` stand under the slab, in the model's own space.
    fn under_the_slab(mut scene: Scene, sides: &[Side]) -> Vec<[u32; 3]> {
        grown(&mut scene, sides);
        let mut contacts: Vec<[u32; 3]> = scene.objects()[0]
            .supports
            .points()
            .iter()
            .filter(|point| point.contact.z < 1.0)
            .map(|point| point.contact.to_array().map(f32::to_bits))
            .collect();
        contacts.sort_unstable();
        contacts
    }

    #[test]
    fn a_cavity_through_a_thin_wall_holds_none_of_the_outside_up() {
        let solid = under_the_slab(tilted_slab(false), &[Side::Outside]);
        assert!(!solid.is_empty(), "the slab's underside needs holding up");
        assert_eq!(
            under_the_slab(tilted_slab(true), &[Side::Outside]),
            solid,
            "the cavity's ceiling 0.6 mm through the wall holds none of the underside up"
        );
        assert_eq!(
            under_the_slab(tilted_slab(true), &[Side::Inside, Side::Outside]),
            solid,
            "nor do the supports standing under it already"
        );
    }

    #[test]
    fn a_support_under_a_cavitys_ceiling_lands_on_its_floor() {
        let mut scene = hollow_cube();
        let object = &mut scene.objects_mut()[0];
        let transform = object.transform;
        object
            .supports
            .add(Vec3::new(10.0, 10.0, 20.95), transform, 0);

        object.refresh_supports(&SupportTool::default().table());
        assert_eq!(
            object.supports.standing(),
            1,
            "the cavity's floor is 12 mm under its ceiling, room enough to stand"
        );
    }

    #[test]
    fn holding_the_cavity_up_again_takes_only_what_stood_inside_it() {
        let mut scene = hollow_cube();
        let object = &mut scene.objects_mut()[0];
        let transform = object.transform;
        object
            .supports
            .add(Vec3::new(10.0, 10.0, 20.95), transform, 0);
        object
            .supports
            .add(Vec3::new(10.0, 10.0, 5.0), transform, 0);

        assert_eq!(SupportTool::clear_inside(&mut scene), 1);
        let left = scene.objects()[0].supports.points();
        assert_eq!(left.len(), 1);
        assert!(
            left[0].contact.z.abs() < 1e-4,
            "the underside's support stays"
        );
    }

    #[test]
    fn a_solid_model_has_nothing_inside() {
        let scene = one_cube();
        let tool = SupportTool::default();
        let object = &scene.objects()[0];
        let placements = vec![Placement {
            id: object.id,
            transform: object.transform,
            needs: vec![Need {
                contact: Vec3::new(0.5, 0.5, 0.5),
                reason: Reason::Steady,
            }],
        }];
        assert!(!object.supported_inside());
        let outside = tool.on_its_side(placements, &scene);
        assert_eq!(
            outside[0].needs.len(),
            1,
            "every contact of a solid model is outside"
        );
    }

    #[test]
    fn holding_the_cavity_up_stands_supports_under_its_ceiling_and_nowhere_else() {
        let mut scene = hollow_cube();
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let mut tool = SupportTool::default();
        tool.start(&mut scene, 0.05, &catalogue, Side::Inside)
            .expect("a hollow model to hold up");
        let mut status = Status::default();
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }

        let object = &scene.objects()[0];
        let points = object.supports.points();
        assert!(
            !points.is_empty(),
            "the cavity's 12 mm ceiling needs holding up: {status:?}"
        );
        for point in points {
            // The cavity spans 0.2..0.8 of the unit cube the model is built from.
            assert!(
                (point.contact.z - 0.8).abs() < 0.01,
                "{} is not under the cavity's ceiling",
                point.contact
            );
        }
    }

    #[test]
    fn a_drilled_hole_is_held_up_inside_like_any_underside() {
        use crate::scene::{ImportSummary, Imported};
        use core_geometry::diagnose;

        let mesh = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(20.0));
        let mut scene = Scene::default();
        let transform = Transform::from_translation(Vec3::new(0.0, 0.0, 5.0));
        let id = scene.insert(Imported::new(
            "block".to_owned(),
            Arc::new(mesh.clone()),
            transform,
            ImportSummary::made(diagnose(&mesh)),
        ));
        let object = scene.get_mut(id).expect("the block is on the plate");
        // A hole 10 mm across bored 10 mm in from the side at x = 20: a millimetre under
        // its crown it is 6 mm wide, 3 mm past the reach of a 45 degree face either side.
        let size = core_volume::HoleSize::round(10.0, 10.0, 1.0);
        let bvh = Bvh::build(&mesh);
        object.hollow.add_drain(
            &mesh,
            &bvh,
            Vec3::new(20.0, 10.0, 10.0),
            Vec3::X,
            size,
            transform,
        );
        let source = Arc::clone(object.uncut());
        let cuts = Arc::clone(object.hollow.cut_bodies().expect("the hole is cut"));
        let drilled = core_volume::Drilled::of(&source, None, &cuts).expect("a block drills");
        object.hollow.keep_drilled(&source, &cuts, Some(drilled));
        assert!(
            object.keep_support_surface(),
            "supports stand against the block drilled"
        );

        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
        let mut tool = SupportTool::default();
        tool.start(&mut scene, 0.05, &catalogue, Side::Outside)
            .expect("a model to place under");
        let mut status = Status::default();
        while tool.poll(&mut scene, &mut status) {
            std::thread::yield_now();
        }

        let in_the_hole = scene.objects()[0]
            .supports
            .points()
            .iter()
            .filter(|point| {
                let at = point.contact;
                // Over the block's underside, in from the side the hole was bored through.
                at.x > 10.0 && (at.y - 10.0).abs() < 5.0 && at.z > 1.0
            })
            .count();
        assert!(
            in_the_hole > 0,
            "the ceiling of the bore hangs over nothing: {status:?}"
        );
    }

    #[test]
    fn a_support_edited_inside_the_cavity_is_counted_and_deleted_with_it() {
        let mut scene = hollow_cube();
        let object = &mut scene.objects_mut()[0];
        let transform = object.transform;
        object
            .supports
            .add(Vec3::new(10.0, 10.0, 20.95), transform, 0);
        object
            .supports
            .add(Vec3::new(10.0, 10.0, 5.0), transform, 0);
        object.refresh_supports(&SupportTool::default().table());
        let inside_tree = (0..object.supports.trees().len())
            .find(|tree| object.supports.trees()[*tree].root().position.z > 9.0)
            .expect("the cavity's support stands on the cavity's floor");
        object.supports.freeze(inside_tree, transform);

        assert_eq!(
            SupportTool::inside_count(&scene),
            1,
            "the frozen tree is the cavity's"
        );
        assert_eq!(SupportTool::clear_inside(&mut scene), 1);
        let object = &scene.objects()[0];
        assert!(
            object.supports.frozen().is_empty(),
            "the edited support went with the cavity"
        );
        assert_eq!(
            object.supports.points().len(),
            1,
            "the underside's support stays"
        );
        assert_eq!(SupportTool::inside_count(&scene), 0);
    }

    #[test]
    fn a_line_starts_only_from_a_support_that_still_stands() {
        let mut scene = one_cube();
        let object = &mut scene.objects_mut()[0];
        object.transform.translation.z = 10.0;
        let (id, transform) = (object.id, object.transform);
        let at = Vec3::new(0.5, 0.5, 10.0);
        object.supports.add(at, transform, 0);
        let tool = SupportTool {
            last_placed: Some((id, transform.to_matrix().inverse().transform_point3(at))),
            ..SupportTool::default()
        };
        let start = tool
            .line_start(&scene, id)
            .expect("the last support stands");
        assert!(start.distance(at) < 1e-4, "the line starts at {start}");

        // As an undo, or a new scene whose first model reuses the id, leaves it.
        scene.objects_mut()[0].supports.clear();
        assert_eq!(tool.line_start(&scene, id), None);
    }
}
