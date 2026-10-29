use std::fmt;
use std::path::PathBuf;

use core_analysis::{Measured, Risk, RiskKind, equivalent_disc_mm};
use core_pipeline::Written;
use core_raster::{GreyRange, RasterSettings, Shading};

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
        RiskKind::Floating { area_mm2 } => {
            format!("{area_mm2:.2} mm^2 left hanging over an island taken out")
        }
        RiskKind::Lever {
            stress_mpa,
            lever_mm,
            neck_mm2,
        } => {
            format!("{stress_mpa:.0} MPa on a neck of {neck_mm2:.2} mm^2, {lever_mm:.1} mm off it")
        }
        RiskKind::Peel { force_n } => format!("peel of {force_n:.0} N on the film"),
        RiskKind::SuctionCup {
            area_mm2,
            volume_mm3,
        } => format!(
            "suction cup of {area_mm2:.1} mm^2 holding {:.2} ml",
            volume_mm3 / 1000.0
        ),
        RiskKind::Thin { width_mm } => format!("section {width_mm:.2} mm across"),
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
            equivalent_disc_mm(f64::from(pull.value))
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
    let range = match settings.grey.range {
        GreyRange::FULL => String::new(),
        GreyRange { min, max } => format!(" spread over {min}..{max}"),
    };
    let blur = match settings.blur_px {
        0 => String::new(),
        radius => format!(", blurred {radius} px"),
    };
    format!("coverage, exact pixel area, {levels}{range}{blur}")
}

/// `--grey-range`, as `MIN..MAX`.
pub fn parse_grey_range(text: &str) -> Result<GreyRange, String> {
    let (min, max) = text
        .split_once("..")
        .ok_or_else(|| "expected MIN..MAX".to_owned())?;
    let grey = |value: &str| {
        value
            .trim()
            .parse::<u8>()
            .map_err(|_| format!("{value:?} is not a grey from 0 to 255"))
    };
    let (min, max) = (grey(min)?, grey(max)?);
    if min > max {
        return Err(format!("{min} is above {max}"));
    }
    Ok(GreyRange { min, max })
}

#[cfg(test)]
mod tests;
