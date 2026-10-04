use core_raster::RasterSettings;
use core_slicer::LayerPlan;
use core_thumbnail::Thumbnail;
use printer_profiles::{MaterialProfile, PrinterProfile};

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
        let layer_count = self.layer_count();
        let exposure: f32 = (0..layer_count)
            .map(|index| self.exposure_of_layer_s(index))
            .sum();
        let material = &self.material;

        let bottom = layer_count.min(material.bottom_layers);
        let normal = layer_count - bottom;
        let travel = |distance_mm: f32, speed_mm_min: f32| distance_mm / speed_mm_min * 60.0;

        let bottom_motion = travel(
            material.bottom_lift_distance_mm,
            material.bottom_lift_speed_mm_min,
        ) + travel(
            material.bottom_lift_distance_mm,
            material.bottom_retract_speed_mm_min,
        );
        let normal_motion = travel(material.lift_distance_mm, material.lift_speed_mm_min)
            + travel(material.retract_distance_mm, material.retract_speed_mm_min);

        // What the machine spends per layer beyond any of this, measured on a real print.
        let unaccounted = layer_count as f32 * material.compensation.layer_time_s;
        let delays = layer_count as f32 * material.wait_per_layer_s();
        let total = exposure
            + delays
            + unaccounted
            + bottom as f32 * bottom_motion
            + normal as f32 * normal_motion;
        total.max(0.0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExposureRange;
    use crate::fixtures::sample_job;

    #[test]
    fn a_job_is_stamped_with_the_time_its_caller_gave() {
        // 2024-02-29T13:45:01Z, whatever the clock of the machine running the test says.
        let job = PrintJob {
            created_unix_s: 1_709_214_301,
            ..sample_job(1)
        };
        assert_eq!(job.created_utc(), "2024-02-29 13:45:01");
        assert_eq!(job.created_minutes(), 1_709_214_301 / 60);
    }

    #[test]
    fn height_is_layer_count_times_layer_height() {
        let job = PrintJob {
            material: MaterialProfile {
                layer_height_mm: 0.05,
                ..MaterialProfile::default()
            },
            ..sample_job(20)
        };
        assert!((job.height_mm() - 1.0).abs() < 1e-6);
        assert!((job.layer_z_mm(0) - 0.05).abs() < 1e-6);
        assert!((job.layer_z_mm(19) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_stack_lifted_off_the_plate_still_states_whole_microns() {
        // 0.05 mm bands starting 3.32 mm up: subtracting two f32 of that size leaves
        // 0.05000019, which a reader rejects as more than three decimals.
        let bounds: Vec<f32> = (0..=200).map(|layer| 3.32 + layer as f32 * 0.05).collect();
        let job = PrintJob {
            plan: LayerPlan::from_bounds(bounds, 13.32),
            ..sample_job(200)
        };

        // The three decimals the formats carry: a whole micron leaves nothing below them.
        for (stated, expected) in [
            (job.nominal_height_mm(), 0.05),
            (job.layer_height_mm(7), 0.05),
            (job.layer_z_mm(0), 3.37),
            (job.height_mm(), 13.32),
        ] {
            assert!(
                (stated * 1000.0).fract() == 0.0 && (stated - expected).abs() < 1e-6,
                "expected {expected} mm to the micron, got {stated}"
            );
        }
    }

    #[test]
    fn weight_is_the_volume_times_the_density() {
        let job = PrintJob {
            volume_mm3: 2000.0,
            material: MaterialProfile {
                density_g_cm3: 1.1,
                ..MaterialProfile::default()
            },
            ..sample_job(10)
        };
        // 2000 mm3 is 2 cm3, so 2.2 g at 1.1 g/cm3.
        assert!((job.weight_g() - 2.2).abs() < 1e-5);
    }

    #[test]
    fn a_band_of_height_replaces_the_resins_exposure() {
        let job = PrintJob {
            material: MaterialProfile {
                layer_height_mm: 1.0,
                bottom_layers: 2,
                exposure_s: 2.0,
                ..MaterialProfile::default()
            },
            // Layer tops run 1, 2, 3 ... mm, and the band covers 3 mm to 5 mm.
            plan: LayerPlan::of_count(1.0, 6),
            exposure: ExposurePlan::new(vec![ExposureRange::new(3.0, 5.0, 7.0)]),
            ..sample_job(6)
        };

        assert!((job.exposure_of_layer_s(2) - 7.0).abs() < 1e-6);
        assert!((job.exposure_of_layer_s(3) - 7.0).abs() < 1e-6);
        assert!(
            (job.exposure_of_layer_s(4) - 2.0).abs() < 1e-6,
            "the fifth layer tops out at 5 mm, which the band stops below"
        );
        assert!(job.varies_by_layer());
    }

    #[test]
    fn a_band_does_not_reach_into_the_bottom_block() {
        let job = PrintJob {
            material: MaterialProfile {
                layer_height_mm: 1.0,
                bottom_layers: 2,
                bottom_exposure_s: 30.0,
                transition_layers: 1,
                exposure_s: 2.0,
                ..MaterialProfile::default()
            },
            plan: LayerPlan::of_count(1.0, 5),
            exposure: ExposurePlan::new(vec![ExposureRange::new(0.0, 100.0, 7.0)]),
            ..sample_job(5)
        };

        assert!((job.exposure_of_layer_s(0) - 30.0).abs() < 1e-6);
        assert!(
            (job.exposure_of_layer_s(2) - 16.0).abs() < 1e-5,
            "the one transition layer sits half way between 30 s and 2 s"
        );
        assert!((job.exposure_of_layer_s(3) - 7.0).abs() < 1e-6);
    }

    #[test]
    fn print_time_counts_a_band_rather_than_the_resins_exposure() {
        let material = MaterialProfile {
            layer_height_mm: 1.0,
            bottom_layers: 0,
            exposure_s: 2.0,
            light_off_delay_s: 0.0,
            lift_distance_mm: 0.0,
            retract_distance_mm: 0.0,
            ..MaterialProfile::default()
        };
        let plain = PrintJob {
            material: material.clone(),
            plan: LayerPlan::of_count(1.0, 4),
            ..sample_job(4)
        };
        let banded = PrintJob {
            exposure: ExposurePlan::new(vec![ExposureRange::new(0.0, 100.0, 12.0)]),
            material,
            plan: LayerPlan::of_count(1.0, 4),
            ..sample_job(4)
        };

        assert_eq!(plain.print_time_s(), 8);
        assert_eq!(banded.print_time_s(), 48);
    }

    #[test]
    fn a_layer_is_exposed_against_the_height_the_resin_was_measured_at() {
        let job = PrintJob {
            material: MaterialProfile {
                // Measured at 0.05 mm, which is the only thickness 4 s is known right for.
                layer_height_mm: 0.05,
                bottom_layers: 0,
                exposure_s: 4.0,
                ..MaterialProfile::default()
            },
            plan: LayerPlan::from_bounds(vec![0.0, 0.1, 0.15, 0.175], 0.175),
            ..sample_job(3)
        };

        // Jacobs at the assumed 0.1 mm depth: 4 s times exp(±0.05/0.1) and exp(-0.025/0.1).
        assert!(
            (job.exposure_of_layer_s(0) - 6.5949).abs() < 1e-3,
            "0.10 mm is half a penetration depth thicker than 0.05"
        );
        assert!(
            (job.exposure_of_layer_s(1) - 4.0).abs() < 1e-6,
            "0.05 mm is what was measured"
        );
        assert!(
            (job.exposure_of_layer_s(2) - 3.1152).abs() < 1e-3,
            "0.025 mm takes far more than half of what 0.05 mm took"
        );
        assert!(
            (job.header_exposure_s() - 6.5949).abs() < 1e-3,
            "the header states the thickest layer, so it states that layer's exposure"
        );
        assert!(!job.is_uniform());
        assert!(job.varies_by_layer());
    }

    #[test]
    fn a_uniform_stack_cut_off_the_resins_height_is_compensated_throughout() {
        let job = PrintJob {
            material: MaterialProfile {
                layer_height_mm: 0.05,
                bottom_layers: 0,
                exposure_s: 4.0,
                ..MaterialProfile::default()
            },
            plan: LayerPlan::of_count(0.1, 4),
            ..sample_job(4)
        };

        for index in 0..4 {
            assert!((job.exposure_of_layer_s(index) - 6.5949).abs() < 1e-3);
        }
        assert!(
            (job.header_exposure_s() - 6.5949).abs() < 1e-3,
            "every layer agrees with the header, so nothing has to be read per layer"
        );
        assert!(!job.varies_by_layer());
    }

    #[test]
    fn a_machine_that_reads_the_header_alone_is_never_asked_for_per_layer_tables() {
        let mut job = sample_job(10);
        job.material.transition_layers = 4;
        assert!(job.varies_by_layer(), "a ramp needs the per-layer tables");

        job.printer.firmware.per_layer_settings = false;
        assert!(
            !job.varies_by_layer(),
            "the machine ramps the bottom block off the header instead"
        );
    }

    #[test]
    fn a_thin_bottom_layer_still_gets_the_exposure_that_sticks_it_to_the_plate() {
        let job = PrintJob {
            material: MaterialProfile {
                layer_height_mm: 0.05,
                bottom_layers: 2,
                bottom_exposure_s: 30.0,
                exposure_s: 4.0,
                ..MaterialProfile::default()
            },
            plan: LayerPlan::of_count(0.025, 4),
            ..sample_job(4)
        };

        assert!((job.exposure_of_layer_s(0) - 30.0).abs() < 1e-6);
        assert!((job.exposure_of_layer_s(2) - 3.1152).abs() < 1e-3);
    }

    #[test]
    fn a_resin_that_states_a_penetration_depth_is_compensated_by_its_working_curve() {
        let job = PrintJob {
            material: MaterialProfile {
                layer_height_mm: 0.05,
                bottom_layers: 0,
                exposure_s: 4.0,
                penetration_depth_mm: Some(0.05),
                ..MaterialProfile::default()
            },
            plan: LayerPlan::from_bounds(vec![0.0, 0.1, 0.15], 0.15),
            ..sample_job(2)
        };

        // Jacobs: a layer one Dp thicker than the measured one takes e times its exposure.
        assert!((job.exposure_of_layer_s(0) - 4.0 * 1.0f32.exp()).abs() < 1e-5);
        assert!((job.exposure_of_layer_s(1) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn print_time_counts_the_bottom_layers_at_their_own_exposure() {
        let material = MaterialProfile {
            bottom_layers: 2,
            bottom_exposure_s: 30.0,
            exposure_s: 2.0,
            light_off_delay_s: 0.0,
            lift_distance_mm: 6.0,
            lift_speed_mm_min: 60.0,
            retract_distance_mm: 6.0,
            retract_speed_mm_min: 60.0,
            bottom_lift_distance_mm: 6.0,
            bottom_lift_speed_mm_min: 60.0,
            bottom_retract_speed_mm_min: 60.0,
            transition_layers: 0,
            ..MaterialProfile::default()
        };
        let job = PrintJob {
            material,
            ..sample_job(4)
        };

        // 2 * 30 s + 2 * 2 s exposure, plus 6 mm up and down at 1 mm/s on all four layers.
        assert_eq!(job.print_time_s(), (60.0 + 4.0 + 4.0 * 12.0) as u32);
    }
}
