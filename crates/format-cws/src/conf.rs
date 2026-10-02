//! `slice.conf`: the settings entry the firmware reads the stack's shape from. Every key
//! is in `docs/formats/cws.md`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use core_format::PrintJob;

/// Name of the entry, which is what tells this archive from the other two we read.
pub(crate) const CONF: &str = "slice.conf";

/// What names us in the entry's first line, which a reader shows and a machine ignores.
const SLICER: &str = concat!("Encrust-", env!("CARGO_PKG_VERSION"));

/// Width the keys are padded to, as in a vendor file.
const KEY_WIDTH: usize = 24;

/// The whole entry: two comment lines and then one `key = value` a line.
pub(crate) fn conf(job: &PrintJob) -> String {
    let material = &job.material;
    let mut out = format!("# {SLICER}\n# conf version 1.0\n\n");
    let mut put = |key: &str, value: String| {
        let _ = writeln!(out, "{key:<KEY_WIDTH$}= {value}");
    };

    // Pixels a millimetre, which is what the firmware scales the images by.
    put("xppm", format!("{:.3}", 1.0 / job.raster.pitch.x));
    put("yppm", format!("{:.3}", 1.0 / job.raster.pitch.y));
    put("xres", job.raster.width_px.to_string());
    put("yres", job.raster.height_px.to_string());
    put("thickness", format!("{:.3}", job.nominal_height_mm()));
    put("layers_num", job.layer_count().to_string());
    put("head_layers_num", material.bottom_layers.to_string());
    put("layers_expo_ms", milliseconds(job.header_exposure_s()));
    put(
        "head_layers_expo_ms",
        milliseconds(material.bottom_exposure_s),
    );
    put("wait_before_expo_ms", milliseconds(material.light_off_s()));
    put("lift_distance", format!("{:.3}", material.lift_distance_mm));
    put("lift_up_speed", speed(material.lift_speed_mm_min));
    put("lift_down_speed", speed(material.retract_speed_mm_min));

    // How far the plate rises when the print is done, which is the whole travel it has.
    put(
        "lift_when_finished",
        format!("{:.0}", job.printer.build_volume.z / 2.0),
    );
    out
}

/// The settings of an entry, by key. A line that is not a setting is skipped rather than
/// refused: the first two are comments and a vendor file has been seen with more.
pub(crate) fn parse(text: &str) -> BTreeMap<String, String> {
    let mut settings = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            settings.insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    settings
}

/// What wrote the file, which the first comment line carries.
pub(crate) fn slicer(text: &str) -> Option<String> {
    let first = text.lines().next()?.trim_start_matches(['#', ' ']);
    Some(first.to_owned()).filter(|name| !name.is_empty() && !name.starts_with("conf version"))
}

fn milliseconds(seconds: f32) -> String {
    format!("{:.0}", (seconds * 1000.0).max(0.0))
}

/// A speed in millimetres a minute, as the entry states it.
fn speed(mm_min: f32) -> String {
    format!("{mm_min:.0}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::sample_job;

    #[test]
    fn the_entry_states_the_panel_and_the_two_exposures() {
        let mut job = sample_job(10);
        job.material.exposure_s = 2.5;
        job.material.bottom_exposure_s = 32.0;
        job.material.bottom_layers = 4;
        let settings = parse(&conf(&job));

        assert_eq!(settings.get("xres").map(String::as_str), Some("8"));
        assert_eq!(settings.get("yres").map(String::as_str), Some("4"));
        assert_eq!(settings.get("layers_num").map(String::as_str), Some("10"));
        assert_eq!(
            settings.get("layers_expo_ms").map(String::as_str),
            Some("2500"),
            "the firmware counts an exposure in milliseconds"
        );
        assert_eq!(
            settings.get("head_layers_expo_ms").map(String::as_str),
            Some("32000")
        );
        assert_eq!(
            settings.get("head_layers_num").map(String::as_str),
            Some("4")
        );
        assert_eq!(settings.get("xppm").map(String::as_str), Some("10.000"));
    }

    #[test]
    fn the_first_comment_line_names_what_wrote_the_file() {
        let text = conf(&sample_job(1));
        assert_eq!(slicer(&text).as_deref(), Some(SLICER));
        assert_eq!(slicer("# conf version 1.0\nxres = 8"), None);
    }
}
