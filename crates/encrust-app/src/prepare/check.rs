//! The check step: the stack measured by the preview's own job, as the Check tool reads it,
//! `islands`, `peel_n` and `support_ml` noted on each model, and its islands supported again
//! while the step's `retry` holds.

use anyhow::Result;
use core_analysis::Risk;
use core_engine::island_floors;
use core_raster::RasterSettings;
use core_recipe::{Metric, retry};
use core_slicer::LayerPlan;
use printer_profiles::{Catalogue, SupportProfile};

use super::Run;
use super::step::Ended;
use super::work::{Batch, Work};
use crate::check::{ForRun, support_islands};
use crate::i18n::tr;
use crate::preview::{as_seen, stack_fingerprint};
use crate::scene::{ObjectId, Scene};
use crate::state::{Doc, Machine, Tools};
use crate::status::Status;
use crate::supports::{BuiltTo, island_holds};

/// The stack a check waits on: the plate it asked to be cut and measured as, so that a cut
/// or a measurement that ended with nothing is a failure, not a reason to ask again.
#[derive(Debug, Default)]
pub(super) struct Checking {
    built: Option<u64>,
    measuring: Option<u64>,
}

impl Checking {
    /// Cuts the plate and measures it where the preview has not, and lands once the
    /// measurement of the plate as it stands is in.
    pub(super) fn poll(&mut self, doc: &Doc, machine: &mut Machine) -> Option<Ended> {
        let (slicing, preview) = (&machine.slicing, &mut machine.preview);
        let Some(settings) = slicing.raster_settings() else {
            return Some(Ended::Stopped(Status::Error(tr!("slicing-no-printer"))));
        };
        let scene = &doc.scene;
        if scene.supports_pending(scene.active_plate()) || preview.is_building() {
            return None;
        }
        let (cutting, fold) = (slicing.cutting(), slicing.fold());
        let fingerprint = stack_fingerprint(scene, cutting);
        if preview.layer_count() == 0 || preview.is_stale(fingerprint) {
            if self.built == Some(fingerprint) {
                return Some(Ended::Stopped(failed(&machine.status)));
            }
            self.built = Some(fingerprint);
            return preview
                .build(scene, cutting)
                .err()
                .map(|error| Ended::Stopped(Status::failed(&error)));
        }
        if preview.measured(&settings, fold).is_some() {
            return Some(Ended::Landed(machine.status.clone()));
        }
        if preview.is_measuring() {
            return None;
        }
        if self.measuring == Some(fingerprint) {
            return Some(Ended::Stopped(failed(&machine.status)));
        }
        self.measuring = Some(fingerprint);
        preview.measure(&settings, fold);
        None
    }
}

/// What the preview said when its cut or measurement failed, or that it did.
fn failed(status: &Status) -> Status {
    match status {
        Status::Error(_) => status.clone(),
        _ => Status::Error(tr!("prepare-check-failed")),
    }
}

impl Run {
    /// Notes what a landed check measured on its models, and where its retry holds for
    /// any of them, supports their islands and checks again.
    pub(super) fn checked(
        &mut self,
        doc: &mut Doc,
        tools: &mut Tools,
        machine: &mut Machine,
        batch: &Batch,
    ) -> Result<()> {
        let Work::Check { attempt } = batch.work else {
            return Ok(());
        };
        let slicing = &machine.slicing;
        let settings = slicing.raster_settings();
        let plan = machine.preview.plan();
        let measured =
            settings.and_then(|settings| machine.preview.measured(&settings, slicing.fold()));
        let (Some(settings), Some(plan), Some(measured)) = (settings, plan, measured) else {
            return Ok(());
        };
        let risks = measured.risks();
        let peel_n = measured.hardest_pull().map_or(0.0, |peak| peak.value);
        let stack = (plan, &settings);
        for (id, islands) in islands_of(&doc.scene, &batch.ids, &risks, stack) {
            let support_mm3 = doc
                .scene
                .get(id)
                .map_or(0.0, |object| object.supports_mm3());
            self.note(id, Metric::Islands, islands as f32);
            self.note(id, Metric::PeelN, peel_n);
            self.note(id, Metric::SupportMl, support_mm3 / 1000.0);
        }
        if !self.retries(&doc.scene, &doc.plate, &batch.ids, attempt)? {
            return Ok(());
        }
        let profiles = self.island_profiles(&batch.ids, &machine.slicing.catalogue)?;
        let run = ForRun {
            ids: &batch.ids,
            profiles: &profiles,
        };
        let plate = (&mut doc.scene, &mut tools.supports);
        machine.status = support_islands(plate, stack, &risks, Some(run));
        let again = Work::Check {
            attempt: attempt + 1,
        };
        self.queue.push_front(Batch::of(again, batch.ids.clone()));
        Ok(())
    }

    /// The profile each of `ids` stands islands on: its latest supports step's, else `auto`.
    fn island_profiles(
        &self,
        ids: &[ObjectId],
        catalogue: &Catalogue,
    ) -> Result<Vec<(ObjectId, SupportProfile)>> {
        let automatic = BuiltTo::of("auto", catalogue)?;
        Ok(ids
            .iter()
            .map(|id| {
                let built = self.built.iter().rev().find(|(owner, _)| owner == id);
                let built = built.map_or(&automatic, |(_, built)| built);
                (*id, built.islands_profile())
            })
            .collect())
    }

    /// Whether the check in hand supports islands again for any of `ids` after `attempt`
    /// tries; the recipe's `max` bounds it.
    fn retries(
        &self,
        scene: &Scene,
        plate: &crate::plate::BuildPlate,
        ids: &[ObjectId],
        attempt: u8,
    ) -> Result<bool> {
        for object in scene.named(ids) {
            let metrics = self.metrics_of(object, plate);
            if retry(&self.plan, self.index, &metrics, attempt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// How many islands of `risks` each of `ids` holds: those it hangs from, and those that hang
/// from no model, which count against every one.
fn islands_of(
    scene: &Scene,
    ids: &[ObjectId],
    risks: &[Risk],
    (plan, settings): (&LayerPlan, &RasterSettings),
) -> Vec<(ObjectId, usize)> {
    let layer_mm = plan.nominal_thickness();
    let floors = island_floors(risks, plan, &as_seen(settings));
    let owners: Vec<Option<ObjectId>> = island_holds(scene, &floors, layer_mm)
        .into_iter()
        .map(|held| held.map(|(id, _)| id))
        .collect();
    ids.iter()
        .map(|id| {
            let held = owners
                .iter()
                .filter(|owner| owner.is_none_or(|owner| owner == *id));
            (*id, held.count())
        })
        .collect()
}
