use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};
use core_format::{OpenFile, SlicedFile};
use core_pipeline::open;

/// Opens a sliced file, prints what it says about itself, and decodes every layer.
///
/// Decoding the whole stack is the point: a header can be read from a file no machine would
/// print, and the layers are where a container is really tested.
pub fn read(path: &Path) -> Result<String> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut source = std::io::BufReader::new(file);
    let mut opened = open(path, &mut source)
        .with_context(|| format!("cannot read {} as a sliced file", path.display()))?;

    let mut report = facts_of(opened.facts());
    let stack = decode_stack(&mut opened)?;
    report.push_str(&stack);
    Ok(report)
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
fn decode_stack(opened: &mut impl OpenFile) -> Result<String> {
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

    let mut out = String::new();
    let _ = writeln!(
        out,
        "  {:<14}{} layers decoded, every pixel of the panel covered",
        "stack",
        facts.layer_count()
    );
    let _ = writeln!(out, "  {:<14}{lit_total} over the stack", "lit pixels");
    if pitch > 0.0 {
        let _ = writeln!(out, "  {:<14}{cured_mm3:.1} mm^3", "masks cure");
    }
    Ok(out)
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
