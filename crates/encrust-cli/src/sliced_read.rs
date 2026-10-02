use std::fmt::{self, Write as _};
use std::path::Path;

use anyhow::{Context, Result};
use core_format::{OpenFile, SlicedFile};
use core_pipeline::open;
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
    stack: &'a Stack,
}

impl Info {
    /// The same facts as the text, as `info --json` prints them.
    pub fn document(&self) -> impl Serialize + '_ {
        let facts = &self.facts;
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
                height_mm: facts.height_mm(),
                exposure_s: facts.exposure_s,
                bottom_exposure_s: facts.bottom_exposure_s,
                bottom_layers: facts.bottom_layers,
                grey_steps: facts.grey_steps,
                print_time_s: facts.print_time_s,
                volume_mm3: facts.volume_mm3,
            },
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
        format!(
            "{} at {:.4} mm, {:.3} mm tall",
            facts.layer_count(),
            facts.layer_height_mm,
            facts.height_mm()
        ),
    );
    line(
        &mut out,
        "exposure",
        format!(
            "{:.2} s, {:.2} s for the first {}",
            facts.exposure_s, facts.bottom_exposure_s, facts.bottom_layers
        ),
    );
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
