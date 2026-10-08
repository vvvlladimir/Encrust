use std::fmt;
use std::path::PathBuf;

use core_analysis::{Measured, Risk, RiskKind, equivalent_disc_mm};
use core_pipeline::Written;
use core_raster::{RasterSettings, Shading};

/// What rasterising the layer stack produced, and what did not fit on the panel.
pub struct RasterReport {
    /// Where the layers went: a directory of PNGs, or one sliced file.
    pub destination: PathBuf,
    pub settings: RasterSettings,
    pub layers_written: usize,
    pub clipped_layers: usize,
    /// Furthest any layer reached past the edge of the panel, pixels.
    pub max_overflow_px: f32,
    /// What the written masks cure, which is what the resin volume is taken from.
    pub measured: Measured,
}

impl RasterReport {
    /// An empty report over a stack about to be written, folding its layers with `fold`.
    pub fn new(destination: PathBuf, settings: RasterSettings, fold: Measured) -> Self {
        Self {
            destination,
            settings,
            layers_written: 0,
            clipped_layers: 0,
            max_overflow_px: 0.0,
            measured: fold,
        }
    }

    /// The report for a sliced file the pipeline has already written.
    pub fn of_written(destination: PathBuf, settings: RasterSettings, written: Written) -> Self {
        Self {
            destination,
            settings,
            layers_written: written.layers,
            clipped_layers: written.clipped_layers,
            max_overflow_px: written.max_overflow_px,
            measured: written.measured,
        }
    }

    /// Counts one written layer and how far past the panel it reached. What the layer
    /// cures was folded in by `core_pipeline::fold_group`.
    pub fn count(&mut self, overflow_px: f32) {
        self.layers_written += 1;
        if overflow_px > 0.0 {
            self.clipped_layers += 1;
            self.max_overflow_px = self.max_overflow_px.max(overflow_px);
        }
    }

    /// True when every layer landed on the panel whole.
    pub fn is_clean(&self) -> bool {
        self.clipped_layers == 0
    }
}

impl fmt::Display for RasterReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let settings = &self.settings;
        writeln!(
            f,
            "  panel         {} x {} px at {:.4} x {:.4} mm",
            settings.width_px, settings.height_px, settings.pitch.x, settings.pitch.y
        )?;
        writeln!(f, "  shading       {}", describe_shading(settings))?;
        writeln!(
            f,
            "  masks         {} written to {}",
            self.layers_written,
            self.destination.display()
        )?;
        writeln!(f, "  cured volume  {:.3} mm^3", self.measured.volume_mm3())?;
        write_risks(f, &self.measured)?;

        if !self.is_clean() {
            writeln!(
                f,
                "  raster defect {} layers clipped, up to {:.1} px past the panel",
                self.clipped_layers, self.max_overflow_px
            )?;
        }
        Ok(())
    }
}

/// Risks listed before the rest are only counted: a stack with hundreds of islands needs
/// fixing, not reading.
const RISKS_SHOWN: usize = 10;

/// One risk as a line: the layer counted from one, as a printer's screen does.
fn describe_risk(risk: &Risk) -> String {
    let [x, y] = risk.at_mm;
    let what = match risk.kind {
        RiskKind::Island { area_mm2 } => format!("island of {area_mm2:.2} mm^2"),
        RiskKind::Lever {
            stress_mpa,
            lever_mm,
            neck_mm2,
        } => {
            format!("{stress_mpa:.0} MPa on a neck of {neck_mm2:.2} mm^2, {lever_mm:.1} mm off it")
        }
        RiskKind::Peel { force_n } => format!("peel of {force_n:.0} N on the film"),
    };
    format!("layer {}, {what} at {x:.1}, {y:.1} mm", risk.layer + 1)
}

/// The layers that pull hardest and grow most, and what is likely to fail, as both the
/// written report and `estimate` print them.
pub fn write_risks(f: &mut fmt::Formatter<'_>, measured: &Measured) -> fmt::Result {
    if let Some(pull) = measured.hardest_pull() {
        writeln!(
            f,
            "  hardest pull  layer {}, like a disc {:.1} mm across",
            pull.layer + 1,
            equivalent_disc_mm(pull.value)
        )?;
    }
    if let Some(growth) = measured.largest_growth() {
        writeln!(
            f,
            "  widest step   layer {}, {:.1} mm^2 more than the one under it",
            growth.layer + 1,
            growth.value
        )?;
    }
    let risks = measured.risks();
    for risk in risks.iter().take(RISKS_SHOWN) {
        writeln!(f, "  risk          {}", describe_risk(risk))?;
    }
    if risks.len() > RISKS_SHOWN {
        writeln!(f, "  risk          and {} more", risks.len() - RISKS_SHOWN)?;
    }
    if measured.removed_islands() > 0 {
        writeln!(
            f,
            "  islands       {} taken out of the file, over {} layers",
            measured.removed_islands(),
            measured.removed_layers()
        )?;
    }
    if let Some(worst) = measured.worst() {
        writeln!(f, "  likely fails  at layer {}", worst.layer + 1)?;
    }
    Ok(())
}

fn describe_shading(settings: &RasterSettings) -> String {
    if settings.shading == Shading::Binary {
        return "binary, no intermediate grey".to_owned();
    }
    let levels = match settings.grey.levels {
        Some(levels) => format!("{levels} greys"),
        None => "all 255 greys".to_owned(),
    };
    let floor = match settings.grey.floor {
        0 => String::new(),
        floor => format!(" down to {floor}"),
    };
    let blur = match settings.blur_px {
        0 => String::new(),
        radius => format!(", blurred {radius} px"),
    };
    format!("coverage, exact pixel area, {levels}{floor}{blur}")
}

#[cfg(test)]
mod tests {
    use core_analysis::{Cured, Stretch, cure};
    use core_raster::{Grey, LayerRuns, PixelPitch};

    use super::*;

    /// One layer into the report, the way `stream_into` puts it there: folded for what it
    /// cures, then counted for what it cost the panel. The stack is 0.05 mm a layer.
    fn record(report: &mut RasterReport, overflow_px: f32, layer: Cured) -> Vec<Stretch> {
        let taken = report.measured.push(layer, 0.05);
        report.count(overflow_px);
        taken
    }

    fn report() -> RasterReport {
        RasterReport::new(
            PathBuf::from("out"),
            RasterSettings {
                width_px: 8520,
                height_px: 4320,
                pitch: PixelPitch { x: 0.018, y: 0.018 },
                mirror_x: true,
                mirror_y: false,
                shading: Shading::Coverage,
                grey: Grey::default(),
                blur_px: 0,
            },
            Measured::new(0),
        )
    }

    /// A square `side` pixels across in the corner of a panel of 1 mm pixels.
    fn square(side: u32) -> Cured {
        let mut builder = LayerRuns::builder(100, 100);
        for row in 0..side {
            builder.pad_to(row * 100);
            builder.push(side, 255);
        }
        cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 })
    }

    #[test]
    fn a_stack_that_fits_reports_no_defect() {
        let mut report = report();
        let _ = record(&mut report, 0.0, square(0));
        let _ = record(&mut report, 0.0, square(0));

        let text = report.to_string();
        assert!(report.is_clean());
        assert!(text.contains("masks         2 written to out"), "{text}");
        assert!(text.contains("coverage, exact pixel area"));
        assert!(!text.contains("raster defect"));
    }

    #[test]
    fn clipping_is_counted_and_the_worst_overflow_kept() {
        let mut report = report();
        let _ = record(&mut report, 0.0, square(0));
        let _ = record(&mut report, 4.5, square(0));
        let _ = record(&mut report, 1.5, square(0));

        assert!(!report.is_clean());
        assert_eq!(report.clipped_layers, 2);
        assert!(
            report
                .to_string()
                .contains("raster defect 2 layers clipped, up to 4.5 px past the panel")
        );
    }

    #[test]
    fn the_volume_and_the_hardest_layer_come_from_the_masks() {
        // Three layers of 0.05 mm over 9 + 25 + 16 mm^2.
        let mut report = report();
        for side in [3, 5, 4] {
            let _ = record(&mut report, 0.0, square(side));
        }
        assert!((report.measured.volume_mm3() - 2.5).abs() < 1e-4);
        let text = report.to_string();
        assert!(text.contains("hardest pull  layer 2"), "{text}");
        assert!(text.contains("widest step   layer 2, 16.0 mm^2"), "{text}");
        assert!(
            !text.contains("risk"),
            "a pyramid standing on itself: {text}"
        );
    }

    #[test]
    fn an_island_is_named_with_its_layer_and_the_layer_it_fails_on() {
        let mut report = report();
        let _ = record(&mut report, 0.0, square(3));
        let mut builder = LayerRuns::builder(100, 100);
        builder.pad_to(50 * 100 + 50);
        builder.push(4, 255);
        let _ = record(
            &mut report,
            0.0,
            cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 }),
        );

        let text = report.to_string();
        assert!(
            text.contains("risk          layer 2, island of 4.00 mm^2"),
            "{text}"
        );
        assert!(text.contains("likely fails  at layer 2"), "{text}");
    }

    #[test]
    fn an_island_taken_out_is_counted_rather_than_named() {
        let mut report = RasterReport {
            measured: Measured::new(0).removing_islands(),
            ..report()
        };
        let _ = record(&mut report, 0.0, square(3));
        let mut builder = LayerRuns::builder(100, 100);
        builder.pad_to(50 * 100 + 50);
        builder.push(4, 255);
        let taken = record(
            &mut report,
            0.0,
            cure(&builder.finish(), PixelPitch { x: 1.0, y: 1.0 }),
        );

        assert_eq!(taken.len(), 1, "the island's one row");
        let text = report.to_string();
        assert!(
            text.contains("islands       1 taken out of the file, over 1 layers"),
            "{text}"
        );
        assert!(!text.contains("risk"), "{text}");
    }

    #[test]
    fn binary_shading_says_so() {
        let mut report = RasterReport::new(
            PathBuf::from("out"),
            RasterSettings {
                shading: Shading::Binary,
                ..report().settings
            },
            Measured::new(0),
        );
        let _ = record(&mut report, 0.0, square(0));
        assert!(report.to_string().contains("binary, no intermediate grey"));
    }

    #[test]
    fn a_blur_is_named_with_its_radius() {
        let settings = RasterSettings {
            blur_px: 2,
            ..report().settings
        };
        assert_eq!(
            describe_shading(&settings),
            "coverage, exact pixel area, all 255 greys, blurred 2 px"
        );
    }
}
