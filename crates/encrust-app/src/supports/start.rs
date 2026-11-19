use anyhow::Result;
use core_geometry::Scalar;
use printer_profiles::Catalogue;

use super::run::spawn;
use super::{Side, SupportTool};
use crate::job::tasks_of;
use crate::scene::{ObjectId, Scene};

/// What a run is kept to: the models it works on, and whether the automatic profile sizes its
/// supports or the group in hand builds them. Such a run adds to what stands, regrowing nothing.
pub struct Only<'a> {
    pub ids: &'a [ObjectId],
    pub sized: bool,
}

impl SupportTool {
    /// Starts placement over everything visible, by the group in hand or the automatic profile;
    /// for the inside, only cavities, and what stood in them goes when it lands (ADR 0332).
    /// The faces painted to be seen are held sparser while `clean` (ADR 0425). What stands in
    /// the groups it grows is grown again, not added to: it goes when the run lands.
    pub fn start(
        &mut self,
        scene: &mut Scene,
        layer_height_mm: Scalar,
        catalogue: &Catalogue,
        (side, clean): (Side, bool),
    ) -> Result<()> {
        self.start_for(scene, layer_height_mm, catalogue, (side, clean), None)
    }

    /// [`Self::start`] on the models of `only` alone where it is given, which it adds supports
    /// to, the ones that stand holding up what is near them.
    pub fn start_for(
        &mut self,
        scene: &mut Scene,
        layer_height_mm: Scalar,
        catalogue: &Catalogue,
        (side, clean): (Side, bool),
        only: Option<Only<'_>>,
    ) -> Result<()> {
        let sized = only.as_ref().is_none_or(|only| only.sized);
        self.sizing = if sized { self.sized(catalogue)? } else { None };
        let mut tasks = match &self.sizing {
            Some((auto, shapes)) => tasks_of(scene, |_| auto.placing_with(&shapes[0])),
            None => tasks_of(scene, |object| self.profile_for(object).clone()),
        };
        if let Some(only) = &only {
            tasks.retain(|task| only.ids.contains(&task.id));
        }
        // A run inside a cavity takes away whatever stood there already when it lands.
        self.regrowing = match (side, &only) {
            (Side::Outside, None) => self.grown_groups(),
            _ => Vec::new(),
        };
        for task in &mut tasks {
            task.shown_only_if(clean);
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
}
