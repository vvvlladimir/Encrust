//! The printer, resin and slicing a recipe names, set on the window as its menus set them, so
//! the plate is drawn on the job's machine and Slice writes what the job's check checked.

use std::num::NonZeroU8;
use std::path::Path;

use anyhow::{Context, Result};
use core_format::ExposureRange;
use core_geometry::Scalar;
use core_slicer::AdaptiveSettings;
use printer_profiles::{MaterialProfile, PrinterProfile};
use serde::Deserialize;

use crate::panels::Window;

/// The layers and masks a recipe asks for, each absent for the resin's own.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Slicing {
    pub layer_height_mm: Option<Scalar>,
    #[serde(default)]
    pub adaptive: bool,
    pub cusp_mm: Option<Scalar>,
    pub min_layer_height_mm: Option<Scalar>,
    #[serde(default)]
    pub exposure_bands: Vec<Band>,
    pub anti_alias: Option<bool>,
    pub grey_levels: Option<u8>,
    pub blur_px: Option<u8>,
    pub remove_islands_passes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Band {
    pub from_mm: f32,
    pub to_mm: f32,
    pub exposure_s: f32,
}

/// Puts the window on `printer` and `resin`, catalogue ids or TOML paths, where it is on
/// another, then sets `slicing` over the resin's own.
pub fn set_machine(
    window: &mut Window,
    (printer, resin): (&str, &str),
    slicing: Option<&Slicing>,
) -> Result<()> {
    if !printer.is_empty() && window.machine.slicing.printer_id.as_deref() != Some(printer) {
        let (profile, id) = if is_toml(printer) {
            let path = Path::new(printer);
            let loaded =
                PrinterProfile::load(path).with_context(|| format!("reading {printer}"))?;
            (loaded, None)
        } else {
            let entry = window.machine.slicing.catalogue.printer(printer)?;
            (entry.profile.clone(), Some(printer.to_owned()))
        };
        crate::profiles::apply_printer(window, profile, id);
    }
    let held = &mut window.machine.slicing;
    if !resin.is_empty() && held.resin_id.as_deref() != Some(resin) {
        let (profile, id) = if is_toml(resin) {
            let loaded = MaterialProfile::load(Path::new(resin))
                .with_context(|| format!("reading {resin}"))?;
            (loaded, None)
        } else {
            (
                held.catalogue.resin(resin)?.profile.clone(),
                Some(resin.to_owned()),
            )
        };
        held.resin_id = id;
        held.set_material(profile);
    }
    let asked = slicing.cloned().unwrap_or_default();
    if let Some(mm) = asked.layer_height_mm {
        held.set_layer_height(mm);
    }
    held.adaptive = asked.adaptive.then(|| {
        let default = AdaptiveSettings::default();
        let max_height_mm = held.layer_height_mm();
        AdaptiveSettings {
            cusp_mm: asked.cusp_mm.unwrap_or(default.cusp_mm),
            min_height_mm: asked.min_layer_height_mm.unwrap_or(max_height_mm / 2.0),
            max_height_mm,
        }
    });
    held.exposure = asked
        .exposure_bands
        .iter()
        .map(|band| ExposureRange {
            from_mm: band.from_mm,
            to_mm: band.to_mm,
            exposure_s: band.exposure_s,
        })
        .collect();
    let edges = &mut held.edges;
    if let Some(on) = asked.anti_alias {
        edges.anti_alias = on;
    }
    if let Some(levels) = asked.grey_levels {
        edges.grey_levels = NonZeroU8::new(levels);
    }
    if let Some(px) = asked.blur_px {
        edges.blur_px = px;
    }
    held.remove_islands = asked.remove_islands_passes.is_some();
    held.hanging_passes = asked.remove_islands_passes.unwrap_or(0);
    Ok(())
}

fn is_toml(named: &str) -> bool {
    Path::new(named)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
}
