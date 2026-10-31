use core_raster::RasterSettings;
use core_slicer::LayerPlan;
use core_thumbnail::Thumbnail;
use printer_profiles::{LimitBreach, MaterialProfile, PrinterProfile};

use crate::timestamp::format_utc;
use crate::{ExposurePlan, exposure_for_mm};

/// Everything a sliced-file writer needs besides the layers themselves.
///
/// The masks are not held here: a full stack does not fit in memory, so the writer takes
/// them one at a time. See `docs/decisions/0012-streaming-sliced-file-writer.md`.
#[derive(Debug, Clone)]
pub struct PrintJob {
    pub printer: PrinterProfile,
    pub material: MaterialProfile,
    /// Panel the masks were rasterised for. The header repeats it, so the two must agree.
    pub raster: RasterSettings,
    /// Where every layer starts and stops, which is how many the writer will be handed
    /// and how high the plate stands for each. The header records both before the first
    /// layer arrives.
    pub plan: LayerPlan,
    /// Resin the part consumes, cubic millimetres.
    pub volume_mm3: f32,
    /// Exposure bands over the resin's own; empty means the resin's throughout.
    pub exposure: ExposurePlan,
    /// Picture of the plate the format's preview records are cut from, or `None` to
    /// leave them the blank image of the right size that the header still points at.
    pub thumbnail: Option<Thumbnail>,
    /// When the file was made, seconds since the Unix epoch. The caller reads the clock,
    /// because a browser has none this crate can reach.
    pub created_unix_s: u64,
}

/// Every height a file states, rounded to the micron the machines step in; see
/// `docs/decisions/0133-a-written-height-is-a-whole-micron.md`.
fn micron(mm: f32) -> f32 {
    (mm * 1000.0).round() / 1000.0
}

impl PrintJob {
    /// Layers the writer will be handed.
    pub fn layer_count(&self) -> u32 {
        self.plan.layer_count() as u32
    }

    /// Total print height in millimetres, to the micron.
    pub fn height_mm(&self) -> f32 {
        micron(
            self.plan
                .top_of(self.plan.layer_count().saturating_sub(1))
                .unwrap_or(0.0),
        )
    }

    /// Height of the plate when layer `index` is exposed, to the micron: the top of that
    /// layer.
    pub fn layer_z_mm(&self, index: u32) -> f32 {
        micron(self.plan.top_of(index as usize).unwrap_or(0.0))
    }

    /// Thickness of layer `index`, millimetres, to the micron.
    pub fn layer_height_mm(&self, index: u32) -> f32 {
        micron(
            self.plan
                .thickness_of(index as usize)
                .unwrap_or(self.material.layer_height_mm),
        )
    }

    /// The thickness a file header states for the whole stack: the thickest layer in the
    /// plan, which on a uniform stack is every layer.
    pub fn nominal_height_mm(&self) -> f32 {
        let nominal = self.plan.nominal_thickness();
        if nominal > 0.0 {
            micron(nominal)
        } else {
            micron(self.material.layer_height_mm)
        }
    }

    /// Whether every layer is the same thickness, so the header alone describes the stack.
    pub fn is_uniform(&self) -> bool {
        self.plan.is_uniform()
    }

    /// Exposure of layer `index`, seconds: the resin's, unless a band of height covers it.
    ///
    /// The bottom block and its transition keep the resin's ramp whatever the bands say;
    /// see `docs/decisions/0090-exposure-is-banded-by-height.md`.
    pub fn exposure_of_layer_s(&self, index: u32) -> f32 {
        let material = &self.material;
        let base = material.exposure_of_layer_s(index);
        // The bottom block is exposed for adhesion to the plate rather than for curing
        // through, so a thin bottom layer keeps the resin's own long exposure.
        if index < material.bottom_layers + u32::from(material.transition_layers) {
            return base;
        }
        let banded = self
            .exposure
            .exposure_at_s(self.layer_z_mm(index))
            .unwrap_or(base);
        self.for_thickness(banded, self.layer_height_mm(index))
    }

    /// The normal exposure a file header states: the resin's, at the thickness the header
    /// also states. Every layer of a uniform stack takes exactly this.
    pub fn header_exposure_s(&self) -> f32 {
        self.for_thickness(self.material.exposure_s, self.nominal_height_mm())
    }

    /// What `exposure_s` becomes on a layer of `thickness_mm`, against the height this
    /// job's resin was measured at.
    fn for_thickness(&self, exposure_s: f32, thickness_mm: f32) -> f32 {
        exposure_for_mm(
            exposure_s,
            self.material.layer_height_mm,
            thickness_mm,
            self.material.penetration_depth_mm,
        )
    }

    /// Whether the printer has to read the per-layer tables rather than the header alone.
    ///
    /// A machine told it does not read them is never asked to, whatever the stack does;
    /// see `docs/decisions/0141`.
    pub fn varies_by_layer(&self) -> bool {
        self.printer.firmware.per_layer_settings
            && (self.material.transition_layers > 0
                || !self.exposure.is_empty()
                || !self.is_uniform())
    }

    /// Each resin setting the job prints at past what its machine takes or its maker advises,
    /// the layer height the stack's own (ADR 0368).
    pub fn limit_breaches(&self) -> Vec<LimitBreach> {
        let printed = MaterialProfile {
            layer_height_mm: self.nominal_height_mm(),
            ..self.material.clone()
        };
        self.printer.limits.breaches(&printed)
    }

    /// When the file was made as `YYYY-MM-DD hh:mm:ss` in UTC.
    pub fn created_utc(&self) -> String {
        format_utc(self.created_unix_s)
    }

    /// When the file was made in minutes since the Unix epoch, the unit several containers
    /// stamp in.
    pub fn created_minutes(&self) -> u32 {
        (self.created_unix_s / 60) as u32
    }

    /// Resin consumed, grams.
    pub fn weight_g(&self) -> f32 {
        self.volume_mm3 / 1000.0 * self.material.density_g_cm3
    }

    /// What the resin this job consumes costs, or `None` for a resin with no price.
    pub fn cost(&self) -> Option<f32> {
        self.material.cost(self.weight_g(), self.volume_mm3)
    }

    /// Estimated print time in seconds: exposure, the waits around it and plate travel.
    ///
    /// Acceleration and the printer's own overheads are not modelled, so this runs a few
    /// per cent short of the machine's own estimate.
    pub fn print_time_s(&self) -> u32 {
        self.time_through_s(self.layer_count())
    }

    /// Estimated seconds until the first `layer_count` layers are printed, on the terms of
    /// [`Self::print_time_s`]; past the stack, the whole print.
    pub fn time_through_s(&self, layer_count: u32) -> u32 {
        let layer_count = layer_count.min(self.layer_count());
        let exposure: f32 = (0..layer_count)
            .map(|index| self.exposure_of_layer_s(index))
            .sum();
        let material = &self.material;

        let bottom = layer_count.min(material.bottom_layers);
        let normal = layer_count - bottom;
        // Under 1 mm/min is a placeholder for motion the machine runs itself, so it adds no
        // time here; the measured layer time carries it (ADR 0293, 0301).
        let travel = |distance_mm: f32, speed_mm_min: f32| {
            if speed_mm_min < 1.0 {
                0.0
            } else {
                distance_mm / speed_mm_min * 60.0
            }
        };

        let stage = &material.second_stage;
        let bottom_motion = travel(
            material.bottom_lift_distance_mm,
            material.bottom_lift_speed_mm_min,
        ) + travel(
            stage.bottom_lift_distance_mm,
            stage.bottom_lift_speed_mm_min,
        ) + travel(
            material.bottom_retract_distance_mm(),
            material.bottom_retract_speed_mm_min,
        ) + travel(
            stage.bottom_retract_distance_mm,
            stage.bottom_retract_speed_mm_min,
        );
        let normal_motion = travel(material.lift_distance_mm, material.lift_speed_mm_min)
            + travel(stage.lift_distance_mm, stage.lift_speed_mm_min)
            + travel(material.retract_distance_mm, material.retract_speed_mm_min)
            + travel(stage.retract_distance_mm, stage.retract_speed_mm_min);

        // What the machine spends per layer beyond any of this, measured on a real print.
        let unaccounted = layer_count as f32 * material.compensation.layer_time_s;
        let delays = layer_count as f32 * material.wait_per_layer_s()
            + bottom as f32 * (material.bottom_light_off_s() - material.light_off_s());
        let total = exposure
            + delays
            + unaccounted
            + bottom as f32 * bottom_motion
            + normal as f32 * normal_motion;
        total.max(0.0) as u32
    }
}

#[cfg(test)]
mod tests;
