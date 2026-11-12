use super::{SHAPES, Side, Sizing, SupportGroup, SupportTool, frozen_inside};
use crate::i18n::tr;
use crate::job::{Placement, SupportJob, SupportOutcome, SupportRequest, SupportTask, tasks_of};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;
use anyhow::{Result, bail};
use core_geometry::{Region, Scalar};
use core_supports::Need;
use printer_profiles::{AUTO_SIZES, AutoProfile, Catalogue, ProfileError, SupportProfile};

/// The word a recipe and `--supports` name the catalogue's automatic profile by.
const AUTO: &str = "auto";

/// What Prepare builds supports to: an automatic profile's sizing, or one profile alone.
#[derive(Debug, Clone)]
pub enum BuiltTo {
    Sized(Box<Sizing>),
    Alone(Box<SupportProfile>),
}

impl BuiltTo {
    /// The profile `id` names in `catalogue`: [`AUTO`], an automatic one, or a support one.
    pub fn of(id: &str, catalogue: &Catalogue) -> Result<Self> {
        if id == AUTO {
            return Ok(Self::Sized(Box::new(catalogue_auto(catalogue)?)));
        }
        if let Some(entry) = catalogue.autos().find(|entry| entry.id == id) {
            let shapes = shapes_of(&entry.profile, catalogue)?;
            return Ok(Self::Sized(Box::new((entry.profile.clone(), shapes))));
        }
        match catalogue.support(id) {
            Ok(entry) => Ok(Self::Alone(Box::new(entry.profile.clone()))),
            Err(_) => bail!(tr!("supports-no-profile", id = id)),
        }
    }

    /// The profile islands stand to when a run builds to this: the lightest shape of a
    /// sizing, or the profile alone.
    pub fn islands_profile(&self) -> SupportProfile {
        match self {
            Self::Sized(sizing) => sizing.0.placing_with(&sizing.1[0]),
            Self::Alone(profile) => (**profile).clone(),
        }
    }
}

/// The catalogue's automatic profile, the window's Auto and `--supports auto`, and its shapes.
pub(super) fn catalogue_auto(
    catalogue: &Catalogue,
) -> Result<(AutoProfile, [SupportProfile; SHAPES])> {
    let Some(entry) = catalogue.autos().next() else {
        bail!(tr!("supports-no-automatic"));
    };
    let auto = entry.profile.clone();
    let shapes = shapes_of(&auto, catalogue)?;
    Ok((auto, shapes))
}

/// The support profiles `auto` names, lightest first and the steadying one last, as
/// `catalogue` has them.
pub(super) fn shapes_of(
    auto: &AutoProfile,
    catalogue: &Catalogue,
) -> Result<[SupportProfile; SHAPES]> {
    match auto.shapes_in(catalogue) {
        Err(ProfileError::Unknown { id, .. }) => bail!(tr!(
            "supports-auto-no-shape",
            name = auto.name.clone(),
            id = id
        )),
        shapes => Ok(shapes?),
    }
}

impl SupportTool {
    /// The automatic profile chosen and the shapes it sizes supports into, or `None` while
    /// supports are built to the group in hand.
    pub(super) fn sized(
        &self,
        catalogue: &Catalogue,
    ) -> Result<Option<(AutoProfile, [SupportProfile; SHAPES])>> {
        self.auto
            .as_ref()
            .map(|auto| Ok((auto.clone(), shapes_of(auto, catalogue)?)))
            .transpose()
    }

    /// Starts placement over everything visible, by the group in hand or the automatic profile;
    /// for the inside, only cavities, and what stood in them goes when it lands (ADR 0332).
    /// The faces painted to be seen are held sparser while `clean` (ADR 0427). What stands in
    /// the groups it grows is grown again, not added to: it goes when the run lands.
    pub fn start(
        &mut self,
        scene: &mut Scene,
        layer_height_mm: Scalar,
        catalogue: &Catalogue,
        (side, clean): (Side, bool),
    ) -> Result<()> {
        self.sizing = self.sized(catalogue)?;
        let mut tasks = match &self.sizing {
            Some((auto, shapes)) => tasks_of(scene, |_| auto.placing_with(&shapes[0])),
            None => tasks_of(scene, |object| self.profile_for(object).clone()),
        };
        // A run inside a cavity takes away whatever stood there already when it lands.
        self.regrowing = match side {
            Side::Outside => self.grown_groups(),
            Side::Inside => Vec::new(),
        };
        for task in &mut tasks {
            if !clean {
                task.shown = Region::default();
            }
            if let Some(object) = scene.get(task.id) {
                task.seeds = object
                    .supports
                    .contacts_outside(object.transform, &self.regrowing);
            }
        }
        let job = spawn(scene, tasks, (layer_height_mm, side), self.sizing.as_ref())?;
        self.job = Some(job);
        self.side = side;
        Ok(())
    }

    /// The shapes Check builds the supports it stands to, lightest first: those of the automatic
    /// profile in hand or set aside, else the first `catalogue` ships, else the group in hand.
    pub fn check_shapes(&self, catalogue: &Catalogue) -> Vec<SupportProfile> {
        let auto = self.auto.as_ref().or(self.parked_auto.as_ref()).cloned();
        let auto = auto.or_else(|| catalogue.autos().next().map(|entry| entry.profile.clone()));
        let shapes = auto.and_then(|auto| shapes_of(&auto, catalogue).ok());
        match shapes {
            Some(shapes) => shapes[..AUTO_SIZES].to_vec(),
            None => vec![self.profile.clone()],
        }
    }

    /// The groups a run started now grows: every group built to a shape the automatic profile
    /// sizes into while it is on, else the group in hand; never one Check stood its answers in.
    fn grown_groups(&self) -> Vec<u16> {
        let Some((auto, shapes)) = &self.sizing else {
            let check = (self.groups.get(usize::from(self.active)))
                .is_none_or(|group| group.answers_check());
            return if check { Vec::new() } else { vec![self.active] };
        };
        let names: Vec<&str> = shapes
            .iter()
            .map(|shape| shape.name.as_str())
            .chain(std::iter::once(auto.name.as_str()))
            .collect();
        (0u16..)
            .zip(self.table())
            .zip(&self.groups)
            .filter(|((_, profile), group)| {
                names.contains(&profile.name.as_str()) && !group.answers_check()
            })
            .map(|((group, _), _)| group)
            .collect()
    }

    /// Placement on `ids` alone, built to `built` whatever the panel holds, handed back
    /// rather than kept: what Prepare stands a model on.
    pub fn start_built_on(
        scene: &Scene,
        ids: &[ObjectId],
        (layer_height_mm, built): (Scalar, &BuiltTo),
        side: Side,
    ) -> Result<SupportJob> {
        let sizing = match built {
            BuiltTo::Sized(sizing) => Some(&**sizing),
            BuiltTo::Alone(_) => None,
        };
        let profile = built.islands_profile();
        let tasks = scene
            .named(ids)
            .map(|object| SupportTask::of(object, profile.clone()))
            .collect();
        spawn(scene, tasks, (layer_height_mm, side), sizing)
    }

    /// Puts what a run started by [`Self::start_built_on`] placed into the scene.
    pub fn land_built(
        &mut self,
        scene: &mut Scene,
        placements: Vec<Placement>,
        side: Side,
        built: BuiltTo,
    ) -> Status {
        let profile = match built {
            BuiltTo::Sized(sizing) => {
                return self.land_placed(scene, placements, side, Some(*sizing));
            }
            BuiltTo::Alone(profile) => *profile,
        };
        let placements = self.cleared_for(scene, placements, side);
        let mut wanted: [Option<SupportProfile>; SHAPES] = Default::default();
        wanted[0] = Some(profile);
        let group = self.groups_for(wanted, scene)[0];
        let count = add_needs(scene, placements, |_| group);
        Status::Info(tr!("supports-placed", count = count))
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
        let regrown = std::mem::take(&mut self.regrowing);
        *status = match outcome {
            SupportOutcome::Placed(placements) => {
                self.clear_regrown(scene, &placements, &regrown);
                let sizing = self.sizing.take();
                self.land_placed(scene, placements, self.side, sizing)
            }
            SupportOutcome::Cancelled => Status::Info(tr!("supports-cancelled")),
            SupportOutcome::Failed(message) => Status::Error(message),
        };
        false
    }

    /// Takes what stood in `groups` outside its cavity off every model `placements` grew
    /// supports for again; what holds the cavity up is the inside run's.
    fn clear_regrown(&mut self, scene: &mut Scene, placements: &[Placement], groups: &[u16]) {
        if groups.is_empty() {
            return;
        }
        for placement in placements {
            let Some(object) = scene.get_mut(placement.id) else {
                continue;
            };
            let inside = object.supported_inside();
            let points: Vec<bool> = (object.supports.points().iter())
                .map(|point| {
                    groups.contains(&point.group) && !(inside && object.holds_inside(point.contact))
                })
                .collect();
            let frozen: Vec<bool> = (object.supports.frozen().iter())
                .map(|tree| {
                    groups.contains(&tree.group()) && !(inside && frozen_inside(object, tree))
                })
                .collect();
            object.supports.retain_points(|index| !points[index]);
            object.supports.retain_frozen(|index| !frozen[index]);
            self.thaw_unseated(object);
        }
        self.let_go(scene);
    }

    /// Lets go of every part picked and of the carry under way: a frozen tree taken away
    /// renumbers the ones after it, which they name.
    fn let_go(&mut self, scene: &mut Scene) {
        self.picked.clear();
        let carried = self.carry.take().map(|carry| carry.before);
        for (id, _) in carried.unwrap_or_default() {
            if let Some(object) = scene.get_mut(id) {
                object.supports.lift(Vec::new());
            }
        }
    }

    /// Puts what a run placed on `side` of the wall into the scene, sized by `sizing` or in
    /// the group in hand, and says how many.
    pub fn land_placed(
        &mut self,
        scene: &mut Scene,
        placements: Vec<Placement>,
        side: Side,
        sizing: Option<Sizing>,
    ) -> Status {
        let placements = self.cleared_for(scene, placements, side);
        match sizing {
            Some((auto, shapes)) => self.place_sized(placements, &auto, shapes, scene),
            None => {
                let count = add_needs(scene, placements, |_| self.active);
                Status::Info(tr!("supports-placed", count = count))
            }
        }
    }

    /// The needs of `placements` on `side` of the wall, what stood inside taken away first
    /// where that side is the cavity.
    fn cleared_for(
        &mut self,
        scene: &mut Scene,
        placements: Vec<Placement>,
        side: Side,
    ) -> Vec<Placement> {
        if side == Side::Inside {
            let placed: Vec<ObjectId> = placements.iter().map(|placed| placed.id).collect();
            Self::clear_inside_of(scene, &placed);
            self.let_go(scene);
        }
        Self::on_its_side(placements, scene, side)
    }

    /// Puts each need of `placements` in the group of the shape `auto` sizes it to, making
    /// groups only for the shapes that hold something.
    fn place_sized(
        &mut self,
        placements: Vec<Placement>,
        auto: &AutoProfile,
        shapes: [SupportProfile; SHAPES],
        scene: &mut Scene,
    ) -> Status {
        let mut sized = [0usize; SHAPES];
        for need in placements.iter().flat_map(|placement| &placement.needs) {
            sized[need.size_in(auto)] += 1;
        }
        let mut wanted = shapes.map(Some);
        for (shape, count) in wanted.iter_mut().zip(sized) {
            if count == 0 {
                *shape = None;
            }
        }
        let groups = self.groups_for(wanted, scene);
        let count = add_needs(scene, placements, |need| groups[need.size_in(auto)]);
        Status::Info(tr!(
            "supports-placed-sized",
            count = count,
            light = sized[0],
            medium = sized[1],
            heavy = sized[2],
            steady = sized[3]
        ))
    }

    /// The group each of `shapes` that is wanted is placed into: the one not Check's already
    /// built to a profile of its name, else one nothing stands in, retuned to it, else a new
    /// one. A shape nothing is sized into is `None`, and is given no group.
    pub(super) fn groups_for(
        &mut self,
        shapes: [Option<SupportProfile>; SHAPES],
        scene: &mut Scene,
    ) -> [u16; SHAPES] {
        let table = self.table();
        let named = shapes.each_ref().map(|shape| {
            let shape = shape.as_ref()?;
            let group = (table.iter().zip(&self.groups))
                .position(|(kind, group)| kind.name == shape.name && !group.answers_check())?;
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
    pub(super) fn group_to_build(
        &mut self,
        shape: SupportProfile,
        taken: &[u16],
        scene: &mut Scene,
    ) -> u16 {
        let empty = (0..self.groups.len() as u16).find(|group| {
            !taken.contains(group)
                && !self.groups[*group as usize].answers_check()
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
}

/// Spawns the run over `tasks`, each held up on `side` of its wall.
fn spawn(
    scene: &Scene,
    mut tasks: Vec<SupportTask>,
    (layer_height_mm, side): (Scalar, Side),
    sizing: Option<&Sizing>,
) -> Result<SupportJob> {
    for task in &mut tasks {
        let Some(object) = scene.get(task.id) else {
            continue;
        };
        task.side = object.wall_side(side == Side::Inside);
        if side == Side::Inside {
            let into_model = object.transform.to_matrix().inverse();
            task.seeds
                .retain(|seed| !object.holds_inside(into_model.transform_point3(*seed)));
        }
    }
    if side == Side::Inside {
        tasks.retain(|task| task.side.is_some());
    }
    if tasks.is_empty() {
        bail!(tr!("supports-nothing-visible-error"));
    }
    Ok(SupportJob::spawn(SupportRequest {
        tasks,
        layer_height_mm,
        sizing: sizing.cloned(),
    }))
}

/// Adds every need of `placements` to its model in the group `group_of` answers, and
/// answers how many there were.
fn add_needs(
    scene: &mut Scene,
    placements: Vec<Placement>,
    group_of: impl Fn(&Need) -> u16,
) -> usize {
    let mut count = 0;
    for placement in placements {
        count += placement.needs.len();
        let Some(object) = scene.get_mut(placement.id) else {
            continue;
        };
        for need in placement.needs {
            object
                .supports
                .add(need.contact, placement.transform, group_of(&need));
        }
    }
    count
}
