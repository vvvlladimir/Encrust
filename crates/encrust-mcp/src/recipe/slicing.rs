//! How a job's model is cut into layers and each layer drawn as a mask: the command line's
//! Slicing and Masks flags, which the check and the printer file both follow.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

/// The layers and masks, each field absent for the resin's or the printer's own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Slicing {
    /// Layer thickness, mm, as `--layer-height`; the thickest one with `adaptive`.
    pub layer_height_mm: Option<f32>,
    /// Thick layers where the surface is vertical and thin where it is shallow.
    #[serde(default)]
    pub adaptive: bool,
    /// The stair-step an adaptive stack may leave, mm.
    pub cusp_mm: Option<f32>,
    /// The thinnest layer an adaptive stack may use, mm.
    pub min_layer_height_mm: Option<f32>,
    /// Exposure over bands of print height; the bottom layers keep the resin's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exposure_bands: Vec<Band>,
    /// Shades edge pixels by how much the model covers them (true) or not at all (false).
    pub anti_alias: Option<bool>,
    /// Greys an anti-aliased edge is rounded to, white included.
    #[schemars(range(min = 1))]
    pub grey_levels: Option<u8>,
    /// Fades every edge over a box this many pixels either side.
    #[schemars(range(max = 8))]
    pub blur_px: Option<u8>,
    /// Takes every island out of the masks, and this many layers more of what hangs there.
    pub remove_islands_passes: Option<u32>,
}

/// One band of `--exposure-at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Band {
    pub from_mm: f32,
    pub to_mm: f32,
    pub exposure_s: f32,
}

impl Slicing {
    pub(super) fn push_flags(&self, flags: &mut Vec<String>) {
        if let Some(mm) = self.layer_height_mm {
            flags.push(format!("--layer-height={mm}"));
        }
        if self.adaptive {
            flags.push("--adaptive".into());
        }
        for (flag, value) in [
            ("--cusp", self.cusp_mm),
            ("--min-layer-height", self.min_layer_height_mm),
        ] {
            if let Some(value) = value {
                flags.push(format!("{flag}={value}"));
            }
        }
        for band in &self.exposure_bands {
            let Band {
                from_mm,
                to_mm,
                exposure_s,
            } = band;
            flags.push(format!("--exposure-at={from_mm}:{to_mm}:{exposure_s}"));
        }
        match self.anti_alias {
            Some(true) => flags.push("--anti-alias".into()),
            Some(false) => flags.push("--no-anti-alias".into()),
            None => {}
        }
        if let Some(levels) = self.grey_levels {
            flags.push(format!("--grey-levels={levels}"));
        }
        if let Some(px) = self.blur_px {
            flags.push(format!("--blur={px}"));
        }
        if let Some(passes) = self.remove_islands_passes {
            flags.extend([
                "--remove-islands".into(),
                format!("--hanging-passes={passes}"),
            ]);
        }
    }
}
