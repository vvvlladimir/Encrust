//! The program the board runs: a settings header in comments, then one block a layer that
//! shows an image, exposes it and peels. Every line is in `docs/formats/cws.md`.

use std::fmt::Write as _;

use core_format::PrintJob;
use core_raster::Shading;

/// The keyword a layer's image is shown by, and the one that blanks the panel again.
const SLICE: &str = ";<Slice>";
const BLANK: &str = ";<Slice> Blank";

/// The keyword a wait is stated by, in milliseconds.
const DELAY: &str = ";<Delay>";

/// The whole program but the per-layer blocks: the settings header and the opening moves.
pub(crate) fn preamble(job: &PrintJob) -> String {
    let mut text = header(job);
    text.push_str(
        "\nG28 ;Auto Home\n\
         G21 ;Set units to be mm\n\
         G91 ;Relative Positioning\n\
         M17 ;Enable motors\n",
    );
    text.push_str(BLANK);
    text.push_str("\nM106 S0\n\n");
    text
}

/// The settings header, `;(Key = value)` a line, in the order a vendor file carries them.
///
/// The firmware ignores all of it and a slicer reads it back: the keys are spelt as the
/// container spells them, spaces and all.
fn header(job: &PrintJob) -> String {
    let mut lines = vec![";(****Build and Slicing Parameters****)".to_owned()];
    lines.extend(slicing_settings(job));
    lines.push(";(****Machine Configuration ******)".to_owned());
    lines.extend(machine_settings(job));
    lines.push(";(****Slicer Configuration ******)".to_owned());
    lines.extend(slicer_settings(job));
    lines.join("\n") + "\n"
}

/// One setting as the header states it.
fn setting(key: &str, value: impl AsRef<str>) -> String {
    format!(";({key} = {})", value.as_ref())
}

/// What the stack is and how each layer of it is printed.
fn slicing_settings(job: &PrintJob) -> Vec<String> {
    let (material, raster) = (&job.material, &job.raster);
    let (anti_alias, levels) = match raster.shading {
        Shading::Coverage => ("True", 8),
        Shading::Binary => ("False", 1),
    };
    vec![
        setting("Pix per mm X", format!("{:.3}", 1.0 / raster.pitch.x)),
        setting("Pix per mm Y", format!("{:.3}", 1.0 / raster.pitch.y)),
        setting("X Resolution", raster.width_px.to_string()),
        setting("Y Resolution", raster.height_px.to_string()),
        setting("Layer Thickness", format!("{:.3}", job.nominal_height_mm())),
        setting("Layer Time", milliseconds(job.header_exposure_s())),
        setting("Render Outlines", "False"),
        setting("Outline Width Inset", "0"),
        setting("Outline Width Outset", "0"),
        setting(
            "Bottom Layers Time",
            milliseconds(material.bottom_exposure_s),
        ),
        setting(
            "Number of Bottom Layers",
            material.bottom_layers.to_string(),
        ),
        setting("Blanking Layer Time", milliseconds(material.light_off_s())),
        setting("Build Direction", "Bottom_Up"),
        setting("Lift Distance", format!("{:.3}", material.lift_distance_mm)),
        setting("Slide/Tilt Value", "0"),
        setting("Use Mainlift GCode Tab", "False"),
        setting("Settle Time", milliseconds(material.light_off_s())),
        setting("Anti Aliasing", anti_alias),
        setting("Anti Aliasing Value", format!("{levels:.3}")),
        setting(
            "Z Lift Feed Rate",
            format!("{:.3}", material.lift_speed_mm_min),
        ),
        setting(
            "Z Bottom Lift Feed Rate",
            format!("{:.3}", material.bottom_lift_speed_mm_min),
        ),
        setting(
            "Z Lift Retract Rate",
            format!("{:.3}", material.retract_speed_mm_min),
        ),
        setting("Flip X", bool_text(job.printer.mirror_x)),
        setting("Flip Y", bool_text(job.printer.mirror_y)),
        setting("Number of Slices", job.layer_count().to_string()),
    ]
}

/// The machine the stack was sliced for.
fn machine_settings(job: &PrintJob) -> Vec<String> {
    let printer = &job.printer;
    vec![
        setting(
            "Platform X Size",
            format!("{:.3}", printer.display.width_mm),
        ),
        setting(
            "Platform Y Size",
            format!("{:.3}", printer.display.height_mm),
        ),
        setting("Platform Z Size", format!("{:.3}", printer.build_volume.z)),
        setting("Max X Feedrate", "200"),
        setting("Max Y Feedrate", "200"),
        setting("Max Z Feedrate", "200"),
        setting("Machine Type", "UV_LCD"),
    ]
}

/// What wrote the file and what it was written for, which only a reader of ours looks at.
fn slicer_settings(job: &PrintJob) -> Vec<String> {
    let material = &job.material;
    vec![
        setting("Slicer", SLICER),
        setting("Resin", material.name.clone()),
        setting(
            "Bottom Layer Light PWM",
            material.bottom_light_pwm.to_string(),
        ),
        setting("Layer Light PWM", material.light_pwm.to_string()),
    ]
}

/// What names us in the header, which a reader shows and a machine ignores.
const SLICER: &str = concat!("Encrust-", env!("CARGO_PKG_VERSION"));

/// One layer's block: the image, its exposure, and the peel that follows it.
///
/// Every value here is this layer's own, so a banded exposure and a stack of mixed
/// thicknesses are written as they stand.
pub(crate) fn layer_block(job: &PrintJob, index: u32) -> String {
    let material = &job.material;
    let bottom = material.is_bottom_layer(index);
    let (lift_mm, lift_speed) = if bottom {
        (
            material.bottom_lift_distance_mm,
            material.bottom_lift_speed_mm_min,
        )
    } else {
        (material.lift_distance_mm, material.lift_speed_mm_min)
    };
    let retract_speed = if bottom {
        material.bottom_retract_speed_mm_min
    } else {
        material.retract_speed_mm_min
    };
    let pwm = if bottom {
        material.bottom_light_pwm
    } else {
        material.light_pwm
    };
    let exposure_s = job.exposure_of_layer_s(index);
    let height_mm = job.layer_height_mm(index);
    let settle_s = material.light_off_s() + material.waits.rests_s().iter().sum::<f32>();

    let mut block = format!("{SLICE} {index}\n");
    if exposure_s > 0.0 && pwm > 0 {
        let _ = writeln!(block, "M106 S{pwm} ;UV on");
        let _ = writeln!(block, "{DELAY} {}", milliseconds(exposure_s));
        block.push_str("M106 S0 ;UV off\n");
    }
    block.push_str(BLANK);
    block.push('\n');

    // The moves are relative, so a layer lifts clear and comes back a layer higher than it
    // started: the difference of the two is the only thing that moves the plate up.
    if lift_mm > 0.0 {
        let _ = writeln!(block, "G1 Z{lift_mm:.3} F{lift_speed:.0}");
        let _ = writeln!(
            block,
            "G1 Z-{:.3} F{retract_speed:.0}",
            (lift_mm - height_mm).max(0.0)
        );
    } else {
        let _ = writeln!(block, "G1 Z{height_mm:.3} F{lift_speed:.0}");
    }
    let _ = writeln!(block, "{DELAY} {}\n", milliseconds(settle_s));
    block
}

/// The closing moves: the light out, the plate clear of the vat, the motors off.
pub(crate) fn epilogue(job: &PrintJob) -> String {
    format!(
        "M106 S0 ;UV off\n\
         G1 Z{:.3} F{:.0}\n\
         M18 ;Disable Motors\n\
         ;<Completed>\n",
        job.printer.build_volume.z / 2.0,
        job.material.lift_speed_mm_min,
    )
}

/// What one layer's block says about its layer.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct Block {
    /// Top of the layer above the plate, millimetres, taken from the relative moves.
    pub z_mm: f32,
    pub exposure_s: f32,
}

/// The program's header settings and one record per layer block.
///
/// A block runs from the keyword that shows its image to the wait that follows its peel, so
/// the moves of the epilogue fall outside every layer.
pub(crate) fn parse(program: &str) -> (Vec<(String, String)>, Vec<Block>) {
    let mut settings = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let (mut lit, mut open, mut z_mm) = (false, false, 0.0f32);

    for line in program.lines() {
        let line = line.trim();
        if let Some(setting) = line
            .strip_prefix(";(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            if let Some((key, value)) = setting.split_once('=') {
                settings.push((key.trim().to_owned(), value.trim().to_owned()));
            }
        } else if line == BLANK {
            lit = false;
        } else if let Some(index) = line.strip_prefix(SLICE) {
            if index.trim().parse::<u32>().is_ok() {
                blocks.push(Block {
                    z_mm,
                    exposure_s: 0.0,
                });
                open = true;
            }
        } else if let Some(pwm) = line.strip_prefix("M106 S") {
            lit = number(pwm).is_some_and(|pwm| pwm > 0.0);
        } else if let Some(wait) = line.strip_prefix(DELAY) {
            // The light is lit by one `M106` and put out by the next, so the wait between
            // the two is the exposure and the wait after the peel ends the block.
            match (lit, number(wait), blocks.last_mut()) {
                (true, Some(ms), Some(block)) => block.exposure_s = ms / 1000.0,
                (false, _, _) => open = false,
                _ => {}
            }
        } else if let Some(mm) = line.strip_prefix("G1 Z").and_then(number) {
            z_mm += mm;
            if let (true, Some(block)) = (open, blocks.last_mut()) {
                block.z_mm = z_mm;
            }
        }
    }
    (settings, blocks)
}

/// The number a command's argument begins with, where it begins with one.
fn number(argument: &str) -> Option<f32> {
    argument
        .trim()
        .split([' ', ';'])
        .next()?
        .trim()
        .parse()
        .ok()
}

fn milliseconds(seconds: f32) -> String {
    format!("{:.0}", (seconds * 1000.0).max(0.0))
}

fn bool_text(value: bool) -> String {
    if value { "True" } else { "False" }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::sample_job;

    fn program(job: &PrintJob) -> String {
        let mut text = preamble(job);
        for index in 0..job.layer_count() {
            text.push_str(&layer_block(job, index));
        }
        text.push_str(&epilogue(job));
        text
    }

    #[test]
    fn every_layer_has_a_block_naming_its_own_image() {
        let text = program(&sample_job(3));
        for index in 0..3 {
            assert!(
                text.contains(&format!(";<Slice> {index}\n")),
                "layer {index}"
            );
        }
        assert!(text.ends_with(";<Completed>\n"));
    }

    #[test]
    fn a_blocks_exposure_is_the_wait_while_the_light_is_lit() {
        let mut job = sample_job(4);
        job.material.bottom_layers = 1;
        job.material.bottom_exposure_s = 30.0;
        job.material.exposure_s = 2.5;
        let (_, blocks) = parse(&program(&job));

        assert_eq!(blocks.len(), 4);
        assert!(
            (blocks[0].exposure_s - 30.0).abs() < 1e-3,
            "{:?}",
            blocks[0]
        );
        assert!((blocks[1].exposure_s - 2.5).abs() < 1e-3, "{:?}", blocks[1]);
    }

    #[test]
    fn the_relative_moves_add_up_to_the_height_of_the_stack() {
        let job = sample_job(4);
        let (_, blocks) = parse(&program(&job));
        let height = job.nominal_height_mm();

        for (index, block) in blocks.iter().enumerate() {
            let expected = height * (index + 1) as f32;
            assert!(
                (block.z_mm - expected).abs() < 1e-3,
                "layer {index} ends at {} and not {expected}",
                block.z_mm
            );
        }
    }

    #[test]
    fn the_header_states_the_panel_in_the_containers_own_spelling() {
        let (settings, _) = parse(&program(&sample_job(2)));
        let value = |key: &str| {
            settings
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(value("X Resolution"), Some("8"));
        assert_eq!(value("Number of Slices"), Some("2"));
        assert_eq!(value("Slicer"), Some(SLICER));
    }
}
