use std::fmt::{self, Write as _};
use std::path::Path;

use anyhow::{Context, Result};
use core_format::{OpenFile, SlicedFile, encode_grey};
use core_pipeline::open;
use core_raster::LayerRuns;
use serde::Serialize;

/// What a sliced file states about itself, and what decoding its every layer found.
pub struct Info {
    facts: SlicedFile,
    stack: Stack,
}

/// What the decoded layers add up to, which the header can be checked against.
#[derive(Serialize)]
pub struct Stack {
    pub layers_decoded: u32,
    pub lit_px: u64,
    /// Absent when the container records no panel size to measure a pixel by.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cured_mm3: Option<f64>,
}

/// Opens a sliced file, reads what it says about itself, and decodes every layer.
///
/// Decoding the whole stack is the point: a header can be read from a file no machine would
/// print, and the layers are where a container is really tested.
pub fn read(path: &Path) -> Result<Info> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut source = std::io::BufReader::new(file);
    let mut opened = open(path, &mut source)
        .with_context(|| format!("cannot read {} as a sliced file", path.display()))?;

    let stack = decode_stack(&mut opened)?;
    Ok(Info {
        facts: opened.facts().clone(),
        stack,
    })
}

/// One layer of a sliced file: what its table row states and what its mask covers.
pub struct Layer {
    facts: LayerFacts,
    mask: LayerRuns,
}

/// What `info --layer N` states about one layer, counted from one as a screen counts.
#[derive(Serialize)]
struct LayerFacts {
    layer: u32,
    of: u32,
    z_mm: f32,
    thickness_mm: f32,
    exposure_s: f32,
    lit_px: u64,
    panel_px: u64,
}

/// Layer `number` of the file at `path`, counted from one as a printer's screen counts.
pub fn layer(path: &Path, number: u32) -> Result<Layer> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut opened = open(path, std::io::BufReader::new(file))
        .with_context(|| format!("cannot read {} as a sliced file", path.display()))?;
    let facts = opened.facts().clone();
    // Checked here, because a reader counts its layers from zero and the flag counts them
    // from one: its own number is the one to answer in.
    let layers = facts.layer_count();
    if number > layers || number == 0 {
        anyhow::bail!("layer {number} was asked for in a file of {layers} layers");
    }
    let index = number - 1;
    let runs = opened
        .layer(index)
        .with_context(|| format!("cannot decode layer {number}"))?;

    let mut mask = LayerRuns::builder(facts.width_px, facts.height_px);
    for run in &runs {
        mask.push(run.length, run.value);
    }
    let entry = facts.layers[index as usize];
    Ok(Layer {
        facts: LayerFacts {
            layer: number,
            of: layers,
            z_mm: entry.z_mm,
            thickness_mm: thickness_of(&facts, index),
            exposure_s: entry.exposure_s,
            lit_px: runs
                .iter()
                .filter(|run| run.value > 0)
                .map(|run| u64::from(run.length))
                .sum(),
            panel_px: u64::from(facts.width_px) * u64::from(facts.height_px),
        },
        mask: mask.finish(),
    })
}

impl Layer {
    /// The whole panel as an eight-bit greyscale PNG.
    pub fn png(&self) -> Result<Vec<u8>> {
        Ok(encode_grey(&self.mask)?)
    }

    /// The same facts as the text, as `info --layer N --json` prints them.
    pub fn document(&self) -> impl Serialize + '_ {
        &self.facts
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let facts = &self.facts;
        let line = |f: &mut fmt::Formatter<'_>, name: &str, value: String| {
            writeln!(f, "  {name:<14}{value}")
        };
        line(f, "layer", format!("{} of {}", facts.layer, facts.of))?;
        line(
            f,
            "top",
            format!(
                "{:.3} mm above the plate, {:.4} mm thick",
                facts.z_mm, facts.thickness_mm
            ),
        )?;
        line(f, "exposure", format!("{:.2} s", facts.exposure_s))?;
        let share = match facts.panel_px {
            0 => 0.0,
            panel => facts.lit_px as f64 / panel as f64 * 100.0,
        };
        line(
            f,
            "lit pixels",
            format!("{} of {} ({share:.2} %)", facts.lit_px, facts.panel_px),
        )
    }
}

/// Thickness of layer `index`, millimetres: the step up from the layer below, or the
/// header's own height where the container records no Z per layer.
fn thickness_of(facts: &SlicedFile, index: u32) -> f32 {
    let top = facts.layers[index as usize].z_mm;
    let below = match index {
        0 => 0.0,
        _ => facts.layers[index as usize - 1].z_mm,
    };
    match top > below {
        true => top - below,
        false => facts.layer_height_mm,
    }
}

/// The thinnest and thickest layer of the stack, millimetres, or `None` for a stack of one
/// thickness — which is what the header already states.
///
/// A tenth of a micron apart is the same thickness: a container states Z per layer as an
/// `f32`, and adding those up leaves dust well below what any machine steps.
fn varying_thickness(facts: &SlicedFile) -> Option<(f32, f32)> {
    let (thin, thick) = (0..facts.layer_count())
        .map(|index| thickness_of(facts, index))
        .fold((f32::MAX, 0.0f32), |(thin, thick), mm| {
            (thin.min(mm), thick.max(mm))
        });
    (thick - thin > 1e-4).then_some((thin, thick))
}

/// One run of layers above the bottom block whose own exposure is not the header's.
///
/// A resin's transition ramp is one such run, and so is every `--exposure-at` band: what
/// the container records is an exposure per layer, and a run of them is what a reader can
/// say about it.
#[derive(Serialize)]
pub struct Band {
    from_layer: u32,
    to_layer: u32,
    from_mm: f32,
    to_mm: f32,
    from_exposure_s: f32,
    to_exposure_s: f32,
}

fn bands_of(facts: &SlicedFile) -> Vec<Band> {
    let mut bands: Vec<Band> = Vec::new();
    let differs = |exposure_s: f32| (exposure_s - facts.exposure_s).abs() > 1e-3;
    // The way the last layer's exposure moved, so a ramp down and a band of its own above
    // it stay two runs rather than reading as one long slide.
    let mut falling: Option<bool> = None;
    for (index, entry) in facts
        .layers
        .iter()
        .enumerate()
        .skip(facts.bottom_layers as usize)
    {
        if entry.exposure_s <= 0.0 || !differs(entry.exposure_s) {
            continue;
        }
        let number = index as u32 + 1;
        let carries_on = |band: &Band| {
            if band.to_layer + 1 != number {
                return false;
            }
            let step = entry.exposure_s - band.to_exposure_s;
            match (falling, step.abs() > 1e-3) {
                (_, false) => falling.is_none(),
                (None, true) => true,
                (Some(down), true) => down == (step < 0.0),
            }
        };
        match bands.last_mut() {
            Some(band) if carries_on(band) => {
                if (entry.exposure_s - band.to_exposure_s).abs() > 1e-3 {
                    falling = Some(entry.exposure_s < band.to_exposure_s);
                }
                band.to_layer = number;
                band.to_mm = entry.z_mm;
                band.to_exposure_s = entry.exposure_s;
            }
            _ => {
                falling = None;
                bands.push(Band {
                    from_layer: number,
                    to_layer: number,
                    from_mm: entry.z_mm,
                    to_mm: entry.z_mm,
                    from_exposure_s: entry.exposure_s,
                    to_exposure_s: entry.exposure_s,
                });
            }
        }
    }
    bands
}

/// How many runs of exposure the text prints before it stops naming them one by one.
const BANDS_SHOWN: usize = 6;

impl Band {
    /// The run as one line under the exposure the header states.
    fn line(&self) -> String {
        let exposure = if self.to_exposure_s < self.from_exposure_s - 1e-3 {
            format!(
                "from {:.2} s down to {:.2} s",
                self.from_exposure_s, self.to_exposure_s
            )
        } else if self.to_exposure_s > self.from_exposure_s + 1e-3 {
            format!(
                "from {:.2} s up to {:.2} s",
                self.from_exposure_s, self.to_exposure_s
            )
        } else {
            format!("at {:.2} s", self.from_exposure_s)
        };
        let layers = match self.from_layer == self.to_layer {
            true => format!("layer {}", self.from_layer),
            false => format!("layers {}-{}", self.from_layer, self.to_layer),
        };
        format!(
            "{layers} {exposure} ({:.3} to {:.3} mm)",
            self.from_mm, self.to_mm
        )
    }
}

#[derive(Serialize)]
struct Header<'a> {
    format: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<u32>,
    machine: Option<&'a str>,
    slicer: Option<&'a str>,
    resin: Option<&'a str>,
    width_px: u32,
    height_px: u32,
    display_mm: Option<[f32; 2]>,
    layers: u32,
    layer_height_mm: f32,
    /// Thinnest and thickest layer of the stack, millimetres, where they differ from the
    /// height the header states for all of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    thinnest_layer_mm: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thickest_layer_mm: Option<f32>,
    height_mm: f32,
    exposure_s: f32,
    bottom_exposure_s: f32,
    bottom_layers: u32,
    grey_steps: u16,
    print_time_s: Option<u32>,
    volume_mm3: Option<f32>,
}

#[derive(Serialize)]
struct Document<'a> {
    file: Header<'a>,
    /// Every run of layers exposed differently from the header, in print order.
    exposure_bands: Vec<Band>,
    stack: &'a Stack,
}

impl Info {
    /// The same facts as the text, as `info --json` prints them.
    pub fn document(&self) -> impl Serialize + '_ {
        let facts = &self.facts;
        let varying = varying_thickness(facts);
        Document {
            file: Header {
                format: facts.format,
                version: facts.version,
                machine: facts.machine.as_deref(),
                slicer: facts.slicer.as_deref(),
                resin: facts.resin.as_deref(),
                width_px: facts.width_px,
                height_px: facts.height_px,
                display_mm: facts.display_mm.map(|(width, height)| [width, height]),
                layers: facts.layer_count(),
                layer_height_mm: facts.layer_height_mm,
                thinnest_layer_mm: varying.map(|(thin, _)| thin),
                thickest_layer_mm: varying.map(|(_, thick)| thick),
                height_mm: facts.height_mm(),
                exposure_s: facts.exposure_s,
                bottom_exposure_s: facts.bottom_exposure_s,
                bottom_layers: facts.bottom_layers,
                grey_steps: facts.grey_steps,
                print_time_s: facts.print_time_s,
                volume_mm3: facts.volume_mm3,
            },
            exposure_bands: bands_of(facts),
            stack: &self.stack,
        }
    }
}

impl fmt::Display for Info {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&facts_of(&self.facts))?;
        let stack = &self.stack;
        writeln!(
            f,
            "  {:<14}{} layers decoded, every pixel of the panel covered",
            "stack", stack.layers_decoded
        )?;
        writeln!(f, "  {:<14}{} over the stack", "lit pixels", stack.lit_px)?;
        if let Some(cured_mm3) = stack.cured_mm3 {
            writeln!(f, "  {:<14}{cured_mm3:.1} mm^3", "masks cure")?;
        }
        Ok(())
    }
}

fn facts_of(facts: &SlicedFile) -> String {
    let mut out = String::new();
    let line = |out: &mut String, name: &str, value: String| {
        let _ = writeln!(out, "  {name:<14}{value}");
    };

    line(
        &mut out,
        "format",
        match facts.version {
            Some(version) => format!(".{} v{version}", facts.format),
            None => format!(".{}", facts.format),
        },
    );
    for (name, value) in [
        ("machine", &facts.machine),
        ("slicer", &facts.slicer),
        ("resin", &facts.resin),
    ] {
        if let Some(value) = value {
            line(&mut out, name, value.clone());
        }
    }

    let panel = match facts.display_mm {
        Some((width, height)) => format!(
            "{} x {} px over {width:.2} x {height:.2} mm",
            facts.width_px, facts.height_px
        ),
        None => format!("{} x {} px", facts.width_px, facts.height_px),
    };
    line(&mut out, "panel", panel);
    line(
        &mut out,
        "layers",
        match varying_thickness(facts) {
            Some((thin, thick)) => format!(
                "{}, {thin:.4} to {thick:.4} mm, {:.3} mm tall",
                facts.layer_count(),
                facts.height_mm()
            ),
            None => format!(
                "{} at {:.4} mm, {:.3} mm tall",
                facts.layer_count(),
                facts.layer_height_mm,
                facts.height_mm()
            ),
        },
    );
    line(
        &mut out,
        "exposure",
        format!(
            "{:.2} s, {:.2} s for the first {}",
            facts.exposure_s, facts.bottom_exposure_s, facts.bottom_layers
        ),
    );
    let bands = bands_of(facts);
    for band in bands.iter().take(BANDS_SHOWN) {
        line(&mut out, "", band.line());
    }
    if let Some(rest) = bands
        .len()
        .checked_sub(BANDS_SHOWN)
        .filter(|rest| *rest > 0)
    {
        line(&mut out, "", format!("and {rest} more runs of their own"));
    }
    line(&mut out, "greys", format!("{}", facts.grey_steps));
    if let Some(seconds) = facts.print_time_s {
        line(&mut out, "print time", format!("{seconds} s"));
    }
    if let Some(volume) = facts.volume_mm3 {
        line(&mut out, "states", format!("{volume:.1} mm^3 of resin"));
    }
    out
}

/// Decodes every layer and adds up what the masks actually cure, which is the number the
/// header can be checked against.
fn decode_stack(opened: &mut impl OpenFile) -> Result<Stack> {
    let facts = opened.facts().clone();
    let pitch = pitch_mm2(&facts);
    let mut cured_mm3 = 0.0f64;
    let mut lit_total = 0u64;
    let mut previous_z = 0.0;

    for index in 0..facts.layer_count() {
        let runs = opened
            .layer(index)
            .with_context(|| format!("cannot decode layer {index}"))?;

        let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
        let expected = u64::from(facts.width_px) * u64::from(facts.height_px);
        if covered != expected {
            anyhow::bail!("layer {index} covers {covered} pixels, not the panel's {expected}");
        }

        let lit: u64 = runs
            .iter()
            .filter(|run| run.value > 0)
            .map(|run| u64::from(run.length))
            .sum();
        lit_total += lit;

        let z_mm = facts.layers[index as usize].z_mm;
        let thickness = if z_mm > previous_z {
            z_mm - previous_z
        } else {
            facts.layer_height_mm
        };
        previous_z = z_mm;
        cured_mm3 += lit as f64 * f64::from(pitch) * f64::from(thickness);
    }

    Ok(Stack {
        layers_decoded: facts.layer_count(),
        lit_px: lit_total,
        cured_mm3: (pitch > 0.0).then_some(cured_mm3),
    })
}

/// Area of one pixel, square millimetres, or zero when the container records no panel size.
fn pitch_mm2(facts: &SlicedFile) -> f32 {
    match facts.display_mm {
        Some((width_mm, height_mm)) if facts.width_px > 0 && facts.height_px > 0 => {
            (width_mm / facts.width_px as f32) * (height_mm / facts.height_px as f32)
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_format::LayerEntry;

    fn facts() -> SlicedFile {
        SlicedFile {
            format: "goo",
            version: Some(30),
            machine: Some("Mars 4 Ultra".to_owned()),
            slicer: None,
            resin: None,
            width_px: 100,
            height_px: 50,
            display_mm: Some((10.0, 5.0)),
            layer_height_mm: 0.05,
            exposure_s: 2.5,
            bottom_exposure_s: 30.0,
            bottom_layers: 6,
            print_time_s: Some(120),
            volume_mm3: Some(1000.0),
            grey_steps: 256,
            layers: vec![LayerEntry {
                z_mm: 0.05,
                exposure_s: 2.5,
                offset: 0,
                size: 0,
            }],
        }
    }

    #[test]
    fn a_field_the_container_has_no_place_for_is_left_out() {
        let report = facts_of(&facts());
        assert!(report.contains("Mars 4 Ultra"));
        assert!(
            !report.contains("slicer"),
            "a .goo names its slicer, but this file does not, so the line is not printed"
        );
    }

    #[test]
    fn the_panel_is_reported_in_pixels_and_millimetres_where_both_are_known() {
        assert!(facts_of(&facts()).contains("100 x 50 px over 10.00 x 5.00 mm"));

        let no_panel = SlicedFile {
            display_mm: None,
            ..facts()
        };
        assert!(facts_of(&no_panel).contains("100 x 50 px\n"));
    }

    #[test]
    fn a_pixel_area_needs_both_a_panel_and_a_resolution() {
        assert!((pitch_mm2(&facts()) - 0.01).abs() < 1e-6);
        assert_eq!(
            pitch_mm2(&SlicedFile {
                display_mm: None,
                ..facts()
            }),
            0.0
        );
    }
}
