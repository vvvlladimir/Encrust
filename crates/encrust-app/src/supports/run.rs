use super::{ISLANDS_GROUP, SHAPES, Side, Sizing, SupportGroup, SupportTool};
use crate::i18n::tr;
use crate::job::{Placement, SupportJob, SupportOutcome, SupportRequest, SupportTask, tasks_of};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;
use anyhow::{Result, bail};
use core_geometry::Scalar;
use core_supports::Need;
use printer_profiles::{AutoProfile, Catalogue, ProfileError, SupportProfile};

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

    /// The profile's name, as the Select card says what a model stands on.
    pub fn name(&self) -> &str {
        match self {
            Self::Sized(sizing) => &sizing.0.name,
            Self::Alone(profile) => &profile.name,
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
    pub fn start(
        &mut self,
        scene: &mut Scene,
        layer_height_mm: Scalar,
        catalogue: &Catalogue,
        side: Side,
    ) -> Result<()> {
        self.sizing = self.sized(catalogue)?;
        let tasks = match &self.sizing {
            Some((auto, shapes)) => tasks_of(scene, |_| auto.placing_with(&shapes[0])),
            None => tasks_of(scene, |object| self.profile_for(object).clone()),
        };
        let job = spawn(scene, tasks, (layer_height_mm, side), self.sizing.as_ref())?;
        self.job = Some(job);
        self.side = side;
        Ok(())
    }

    /// Placement on `ids` alone, built to `built` whatever the panel holds, handed back
    /// rather than kept: what Prepare stands a model on.
    pub fn start_built_on(
        scene: &Scene,
        ids: &[ObjectId],
        (layer_height_mm, built): (Scalar, &BuiltTo),
        side: Side,
    ) -> Result<SupportJob> {
        let (profile, sizing) = match built {
            BuiltTo::Sized(sizing) => (sizing.0.placing_with(&sizing.1[0]), Some(&**sizing)),
            BuiltTo::Alone(profile) => ((**profile).clone(), None),
        };
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
        *status = match outcome {
            SupportOutcome::Placed(placements) => {
                let sizing = self.sizing.take();
                self.land_placed(scene, placements, self.side, sizing)
            }
            SupportOutcome::Cancelled => Status::Info(tr!("supports-cancelled")),
            SupportOutcome::Failed(message) => Status::Error(message),
        };
        false
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
            // A frozen tree taken away renumbers the ones after it.
            self.picked.clear();
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

    /// The group each of `shapes` that is wanted is placed into: the one already built to
    /// a profile of its name, else one nothing stands in, retuned to it, else a new one.
    /// A shape nothing is sized into is `None`, and is given no group.
    pub(super) fn groups_for(
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
    pub(super) fn group_to_build(
        &mut self,
        shape: SupportProfile,
        taken: &[u16],
        scene: &mut Scene,
    ) -> u16 {
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
        sizing: sizing.map(|(auto, _)| auto.clone()),
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
