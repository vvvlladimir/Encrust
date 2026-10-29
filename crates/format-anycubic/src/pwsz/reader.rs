use core_format::{
    FormatError, LayerEntry, OpenFile, PrinterTuning, ReadSeek, SlicedFile, SlicedFileReader,
    layer_in_range, panel_in_range, read_entry, stated_distance_mm, stated_speed_mm_min,
    stated_wait_s,
};
use core_raster::Run;
use serde_json::Value;
use zip::ZipArchive;

use super::layer_name;
use crate::rle::{GREY_STEPS, decode};

/// The entry that names the machine and the resin, which no other archive we read carries.
const SETTINGS: &str = "anycubic_photon_resins.pwsp";
const LAYERS: &str = "layers_controller.conf";
const PRINT_INFO: &str = "print_info.json";
const SOFTWARE: &str = "software_info.conf";

/// Reads the PWSZ archive under any of its extensions. Layout is in `docs/formats/pwsz.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PwszReader;

impl SlicedFileReader for PwszReader {
    type Open<S: ReadSeek> = OpenPwsz<S>;

    fn extension(&self) -> &'static str {
        "pwsz"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut zip = ZipArchive::new(source)?;
        let settings = json(&mut zip, SETTINGS)?;
        let machine = &settings["machine_type"];
        let resin = active_resin(&settings["machine_extern"]);
        let slice = &resin["slicepara"];

        let side = |key: &str| {
            whole(&machine[key]).ok_or_else(|| FormatError::Missing {
                what: format!("{SETTINGS} machine_type.{key}"),
            })
        };
        let (width_px, height_px) = (side("res_x")?, side("res_y")?);
        panel_in_range(width_px, height_px)?;
        let layers = layer_entries(&json(&mut zip, LAYERS)?);
        let print_info = optional_json(&mut zip, PRINT_INFO)?;
        let software = optional_json(&mut zip, SOFTWARE)?;

        Ok(OpenPwsz {
            zip,
            facts: SlicedFile {
                format: "pwsz",
                version: machine["max_file_version"]
                    .as_u64()
                    .map(|version| version as u32),
                machine: text(&machine["name"]),
                slicer: text(&software["mark"]),
                resin: text(&resin["property"]["name"]),
                width_px,
                height_px,
                display_mm: number(&machine["print_xsize"]).zip(number(&machine["print_ysize"])),
                layer_height_mm: number(&slice["zthick"]).unwrap_or_default(),
                exposure_s: number(&slice["exposure_time"]).unwrap_or_default(),
                bottom_exposure_s: number(&slice["bott_time"]).unwrap_or_default(),
                bottom_layers: whole(&slice["bott_layers"]).unwrap_or_default(),
                print_time_s: print_info["print_time"].as_f64().map(|s| s as u32),
                volume_mm3: number(&print_info["volume"]).map(|ml| ml * 1000.0),
                grey_steps: GREY_STEPS,
                stated: stated(slice),
                layers,
            },
        })
    }
}

/// The user resin the settings mark active, or the first when none is marked.
fn active_resin(machine: &Value) -> &Value {
    let resins = &machine["user_resins"];
    let active = &machine["active_resins"][0];
    resins
        .as_array()
        .and_then(|all| {
            all.iter()
                .find(|resin| &resin["property"]["name"] == active)
        })
        .unwrap_or(&resins[0])
}

/// One entry per row of the layer controller, its top the floor plus the thickness.
fn layer_entries(controller: &Value) -> Vec<LayerEntry> {
    let rows = controller["paras"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    rows.iter()
        .map(|row| LayerEntry {
            z_mm: number(&row["layer_minheight"]).unwrap_or_default()
                + number(&row["layer_thickness"]).unwrap_or_default(),
            exposure_s: number(&row["exposure_time"]).unwrap_or_default(),
            offset: 0,
            size: 0,
        })
        .collect()
}

/// The motion and waits the resin's single-stage settings state; speeds are mm/s there.
fn stated(slice: &Value) -> PrinterTuning {
    let speed = |key: &str| number(&slice[key]).and_then(|mm_s| stated_speed_mm_min(mm_s * 60.0));
    let lift_mm = number(&slice["zup_height"]).and_then(stated_distance_mm);
    PrinterTuning {
        lift_distance_mm: lift_mm,
        retract_distance_mm: lift_mm,
        lift_speed_mm_min: speed("zup_speed"),
        retract_speed_mm_min: speed("zdown_speed"),
        light_off_delay_s: number(&slice["off_time"]).and_then(stated_wait_s),
        bottom_light_off_delay_s: number(&slice["bott_off_time"]).and_then(stated_wait_s),
        ..PrinterTuning::default()
    }
}

fn json<S: ReadSeek>(zip: &mut ZipArchive<S>, name: &str) -> Result<Value, FormatError> {
    let mut entry = zip.by_name(name).map_err(|_| FormatError::Missing {
        what: name.to_owned(),
    })?;
    let bytes = read_entry(&mut entry, name)?;
    serde_json::from_slice(&bytes).map_err(|source| FormatError::Encoding {
        what: "the archive's settings",
        reason: source.to_string(),
    })
}

/// An entry a file usually carries but can do without: absent is `Null`, broken an error.
fn optional_json<S: ReadSeek>(zip: &mut ZipArchive<S>, name: &str) -> Result<Value, FormatError> {
    match json(zip, name) {
        Err(FormatError::Missing { .. }) => Ok(Value::Null),
        read => read,
    }
}

fn number(value: &Value) -> Option<f32> {
    value.as_f64().map(|number| number as f32)
}

/// A count, held at `u32::MAX` past it so a bound downstream refuses it rather than a wrap.
fn whole(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .map(|whole| u32::try_from(whole).unwrap_or(u32::MAX))
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Whether an archive is a PWSZ, by the settings entry only it carries.
pub fn claims<S: ReadSeek>(source: &mut S) -> bool {
    ZipArchive::new(source).is_ok_and(|zip| zip.file_names().flatten().any(|name| name == SETTINGS))
}

/// A PWSZ archive with its settings read and its layers still in it.
pub struct OpenPwsz<S> {
    zip: ZipArchive<S>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenPwsz<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let name = layer_name(index);
        let mut entry = self
            .zip
            .by_name(&name)
            .map_err(|_| FormatError::Missing { what: name.clone() })?;
        decode(&read_entry(&mut entry, &name)?)
    }
}
