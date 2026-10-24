use super::{ISLANDS_GROUP, SHAPES, Side, SupportGroup, SupportTool};
use crate::i18n::tr;
use crate::job::{Placement, SupportJob, SupportOutcome, SupportRequest, tasks_of};
use crate::scene::Scene;
use crate::status::Status;
use anyhow::{Context, Result, bail};
use core_geometry::Scalar;
use core_supports::Need;
use printer_profiles::{AutoProfile, Catalogue, SupportProfile};

/// The support profiles `auto` names, lightest first and the steadying one last, as
/// `catalogue` has them.
pub(super) fn shapes_of(
    auto: &AutoProfile,
    catalogue: &Catalogue,
) -> Result<[SupportProfile; SHAPES]> {
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
        self.sizing = self.sized(catalogue)?;
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
        *status = match outcome {
            SupportOutcome::Placed(placements) => {
                let placements = self.on_its_side(placements, scene);
                match self.sizing.take() {
                    Some((auto, shapes)) => self.place_sized(placements, &auto, shapes, scene),
                    None => {
                        let count = add_needs(scene, placements, |_| self.active);
                        Status::Info(tr!("supports-placed", count = count))
                    }
                }
            }
            SupportOutcome::Cancelled => Status::Info(tr!("supports-cancelled")),
            SupportOutcome::Failed(message) => Status::Error(message),
        };
        false
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
