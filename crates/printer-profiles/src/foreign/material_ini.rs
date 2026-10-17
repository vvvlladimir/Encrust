//! An SLA material profile as `key = value` lines: a saved configuration, or one material's
//! preset. Motion and power are not in it; they belong to the vendor's own machines.

use std::collections::HashMap;
use std::path::Path;

use super::{ForeignResin, named};
use crate::{Compensation, MaterialProfile, PriceUnit, PrinterTuning, ProfileError, ResinDetails};

pub(super) fn read(text: &str, path: &Path) -> Result<ForeignResin, ProfileError> {
    let ini = Ini::parse(text);
    let exposure_s = ini
        .number("exposure_time")
        .ok_or_else(|| ProfileError::MissingSetting {
            path: path.to_owned(),
            key: "exposure_time",
        })?;
    let resin = MaterialProfile {
        name: ini.text("sla_material_settings_id").unwrap_or_default(),
        density_g_cm3: ini
            .number("material_density")
            .filter(|density| *density > 0.0)
            .unwrap_or(MaterialProfile::default().density_g_cm3),
        details: details(&ini),
        exposure_s,
        ..MaterialProfile::default()
    };
    let tuning = PrinterTuning {
        layer_height_mm: ini.number("layer_height").filter(|height| *height > 0.0),
        bottom_exposure_s: ini.number("initial_exposure_time"),
        // The exposure fades from the initial one over these layers: our transition.
        transition_layers: ini
            .material("faded_layers")
            .and_then(|count| count.parse().ok()),
        compensation: Some(compensation(&ini)),
        ..PrinterTuning::default()
    };
    Ok(ForeignResin {
        resin: resin.with_tuning(&tuning),
        machine: ini
            .text("printer_settings_id")
            .or_else(|| ini.text("printer_model")),
        panel_px: ini
            .count("display_pixels_x")
            .zip(ini.count("display_pixels_y"))
            .map(|(width, height)| [width, height]),
    })
}

fn compensation(ini: &Ini) -> Compensation {
    let neutral = Compensation::default();
    let factor = |axis: &str, or: f32| {
        ini.number(&format!("material_correction_{axis}"))
            .filter(|factor| *factor > 0.0)
            .map_or(or, |factor| factor * 100.0)
    };
    let foot_mm = ini
        .material("elefant_foot_compensation")
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|foot| foot.is_finite() && *foot > 0.0);
    Compensation {
        shrink_x_pct: factor("x", neutral.shrink_x_pct),
        shrink_y_pct: factor("y", neutral.shrink_y_pct),
        shrink_z_pct: factor("z", neutral.shrink_z_pct),
        // The first layers' outline moves in by this much; a negative outer offset of ours.
        bottom_outer_offset_mm: foot_mm.map_or(0.0, |foot| -foot),
        ..neutral
    }
}

fn details(ini: &Ini) -> ResinDetails {
    let neutral = ResinDetails::default();
    let litres = ini
        .number("bottle_volume")
        .filter(|ml| *ml > 0.0)
        .map(|ml| ml / 1000.0);
    let price = ini
        .number("bottle_cost")
        .filter(|cost| *cost > 0.0)
        .zip(litres);
    ResinDetails {
        color: ini
            .text("material_colour")
            .and_then(|colour| rgb(&colour))
            .unwrap_or(neutral.color),
        price: price.map_or(0.0, |(cost, litres)| cost / litres),
        price_per: if price.is_some() {
            PriceUnit::Litre
        } else {
            neutral.price_per
        },
        ..neutral
    }
}

/// `#RRGGBB` as its three channels.
fn rgb(colour: &str) -> Option<[u8; 3]> {
    let hex = colour.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// The keys of the file and their values, sections and comments aside.
struct Ini<'a>(HashMap<&'a str, &'a str>);

impl<'a> Ini<'a> {
    fn parse(text: &'a str) -> Self {
        let settings = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with(['#', ';', '[']))
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim(), value.trim()))
            .collect();
        Self(settings)
    }

    /// The value of a key, `None` where blank or `nil`, the format's "not set".
    fn raw(&self, key: &str) -> Option<&'a str> {
        self.0
            .get(key)
            .copied()
            .filter(|value| !value.is_empty() && *value != "nil")
    }

    /// A setting the material may override with its `material_ow_` twin, which then wins.
    fn material(&self, key: &str) -> Option<&'a str> {
        self.raw(&format!("material_ow_{key}"))
            .or_else(|| self.raw(key))
    }

    fn number(&self, key: &str) -> Option<f32> {
        self.raw(key)?
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
    }

    fn count(&self, key: &str) -> Option<u32> {
        self.raw(key)?.parse().ok()
    }

    fn text(&self, key: &str) -> Option<String> {
        named(self.raw(key)?)
    }
}
