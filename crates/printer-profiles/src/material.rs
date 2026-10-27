use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Compensation;
use crate::{PrinterProfile, ProfileError};

mod second_stage;
mod tuning;

pub use second_stage::SecondStage;
pub use tuning::PrinterTuning;

/// Exposure and motion settings for one resin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialProfile {
    pub name: String,
    pub layer_height_mm: f32,
    pub exposure_s: f32,
    pub bottom_exposure_s: f32,
    pub bottom_layers: u32,
    pub light_off_delay_s: f32,
    pub lift_distance_mm: f32,
    pub lift_speed_mm_min: f32,
    pub retract_speed_mm_min: f32,

    /// How far the plate comes back down after a lift; normally the lift distance.
    #[serde(default = "default_retract_distance_mm")]
    pub retract_distance_mm: f32,
    #[serde(default = "default_bottom_lift_distance_mm")]
    pub bottom_lift_distance_mm: f32,
    #[serde(default = "default_bottom_lift_speed_mm_min")]
    pub bottom_lift_speed_mm_min: f32,
    #[serde(default = "default_bottom_retract_speed_mm_min")]
    pub bottom_retract_speed_mm_min: f32,
    #[serde(default)]
    pub second_stage: SecondStage,
    /// Light-off delay of the bottom layers, s; `None` is the normal layers' own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom_light_off_delay_s: Option<f32>,
    /// UV power for normal layers, 0 to 255.
    #[serde(default = "default_light_pwm")]
    pub light_pwm: u8,
    #[serde(default = "default_light_pwm")]
    pub bottom_light_pwm: u8,
    /// Layers over which the exposure fades from the bottom value to the normal one.
    #[serde(default)]
    pub transition_layers: u16,
    /// Resin density, used only to turn a print volume into a weight.
    #[serde(default = "default_density_g_cm3")]
    pub density_g_cm3: f32,
    /// Light penetration depth `Dp` of the resin's working curve, mm, where measured; otherwise
    /// `ASSUMED_PENETRATION_DEPTH_MM`.
    #[serde(default)]
    pub penetration_depth_mm: Option<f32>,
    /// The printer this resin was last tuned on, whose numbers a machine it has not been
    /// set up for starts from.
    #[serde(default)]
    pub last_printer: Option<String>,
    #[serde(default)]
    pub details: ResinDetails,
    #[serde(default)]
    pub waits: Waits,
    /// What the print comes out as against what was sliced, and the corrections for it.
    #[serde(default)]
    pub compensation: Compensation,

    /// What each printer changes about this resin, keyed by catalogue id. One resin is
    /// shared across the catalogue and retuned per machine; see docs/design/profiles.md.
    #[serde(default)]
    pub printers: BTreeMap<String, PrinterTuning>,
    /// The table `for_printer` laid over this resin, which the machine's locks stay under;
    /// empty on a resin as it is kept.
    #[serde(default, skip_serializing_if = "PrinterTuning::is_empty")]
    pub printer_table: PrinterTuning,
}

/// What a resin is, what it looks like and what it costs: nothing a machine changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResinDetails {
    /// The family on the bottle: standard, ABS-like, water washable, high clear.
    pub kind: String,
    /// sRGB, for telling resins apart at a glance.
    pub color: [u8; 3],
    /// Price of one `price_per` of resin, in `currency`. Zero is not priced.
    pub price: f32,
    pub currency: String,
    pub price_per: PriceUnit,
}

impl Default for ResinDetails {
    fn default() -> Self {
        Self {
            kind: String::new(),
            color: [0x80, 0x80, 0x80],
            price: 0.0,
            currency: "€".to_owned(),
            price_per: PriceUnit::Kilogram,
        }
    }
}

/// What a resin's price is quoted per.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PriceUnit {
    #[default]
    Kilogram,
    Litre,
}

/// How the printer waits around a layer: one light-off delay before the exposure, or
/// rests at the three points of a peel. See docs/formats/goo.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WaitMode {
    #[default]
    LightOff,
    Rest,
}

/// The rests of `WaitMode::Rest`, seconds each.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Waits {
    pub mode: WaitMode,
    pub before_lift_s: f32,
    pub after_lift_s: f32,
    pub after_retract_s: f32,
}

impl Waits {
    /// Whether the printer rests rather than holding the light off.
    pub fn resting(&self) -> bool {
        self.mode == WaitMode::Rest
    }

    /// The rests before lift, after lift and after retract the printer is told to take:
    /// zero each unless the resin waits by resting.
    pub fn rests_s(&self) -> [f32; 3] {
        match self.mode {
            WaitMode::LightOff => [0.0; 3],
            WaitMode::Rest => [self.before_lift_s, self.after_lift_s, self.after_retract_s],
        }
    }
}

fn default_retract_distance_mm() -> f32 {
    6.0
}

fn default_bottom_lift_distance_mm() -> f32 {
    8.0
}

fn default_bottom_lift_speed_mm_min() -> f32 {
    65.0
}

fn default_bottom_retract_speed_mm_min() -> f32 {
    150.0
}

fn default_light_pwm() -> u8 {
    255
}

/// Typical photopolymer resin, 1.1 g/cm3.
fn default_density_g_cm3() -> f32 {
    1.1
}

impl MaterialProfile {
    pub fn from_toml_str(source: &str, path: &Path) -> Result<Self, ProfileError> {
        let profile: Self = toml::from_str(source).map_err(|source| ProfileError::Parse {
            path: path.to_owned(),
            source,
        })?;
        profile.validate()?;
        Ok(profile)
    }

    /// Writes the resin, tuning tables and all, refusing one that would not load.
    pub fn save(&self, path: &Path) -> Result<(), ProfileError> {
        let source = self.to_toml_string(path)?;
        std::fs::write(path, source).map_err(|source| ProfileError::Io {
            path: path.to_owned(),
            source,
        })
    }

    /// The profile as the TOML it is kept in; `path` is where it is going, for the error.
    pub fn to_toml_string(&self, path: &Path) -> Result<String, ProfileError> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|source| ProfileError::Serialise {
            path: path.to_owned(),
            source,
        })
    }

    pub fn load(path: &Path) -> Result<Self, ProfileError> {
        let source = std::fs::read_to_string(path).map_err(|source| ProfileError::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::from_toml_str(&source, path)
    }

    /// Exposure of layer `index`, counted from the plate, in seconds.
    /// Transition layers ramp linearly from the bottom exposure to the normal one.
    pub fn exposure_of_layer_s(&self, index: u32) -> f32 {
        if index < self.bottom_layers {
            return self.bottom_exposure_s;
        }
        let step = index - self.bottom_layers;
        if step >= u32::from(self.transition_layers) {
            return self.exposure_s;
        }
        let fraction = (step + 1) as f32 / (f32::from(self.transition_layers) + 1.0);
        self.bottom_exposure_s + (self.exposure_s - self.bottom_exposure_s) * fraction
    }

    /// True while layer `index` still belongs to the bottom block.
    pub fn is_bottom_layer(&self, index: u32) -> bool {
        index < self.bottom_layers
    }

    /// This resin with every setting `tuning` states put over its own.
    #[must_use]
    pub fn with_tuning(mut self, tuning: &PrinterTuning) -> Self {
        tuning.apply_to(&mut self);
        self
    }

    /// This resin as the given printer needs it, with that printer's tuning applied.
    /// An unmentioned printer gets untuned numbers, a starting point; see `is_tuned_for`.
    #[must_use]
    pub fn for_printer(&self, printer_id: &str) -> Self {
        let mut tuned = self.clone();
        tuned.printers = BTreeMap::new();
        if let Some(tuning) = self.printers.get(printer_id) {
            tuning.apply_to(&mut tuned);
            tuned.printer_table = tuning.clone();
        }
        tuned
    }

    /// This resin as `printer` prints it: every setting the machine locks put over its own,
    /// but where the resin's table for the machine states one; see `docs/decisions/0364`.
    #[must_use]
    pub fn on(&self, printer: &PrinterProfile) -> Self {
        let locks = printer.locked.unstated_by(&self.printer_table);
        self.clone().with_tuning(&locks)
    }

    /// This resin as measured at `layer_height_mm`: normal exposure follows the working curve, the
    /// bottom block keeps its seconds; see `docs/decisions/0128`.
    #[must_use]
    pub fn rescaled_to(&self, layer_height_mm: f32) -> Self {
        Self {
            exposure_s: self.exposure_at(self.exposure_s, layer_height_mm),
            layer_height_mm,
            ..self.clone()
        }
    }

    /// `exposure_s`, measured at this resin's height, carried to a layer of `thickness_mm`.
    pub fn exposure_at(&self, exposure_s: f32, thickness_mm: f32) -> f32 {
        crate::exposure_for_mm(
            exposure_s,
            self.layer_height_mm,
            thickness_mm,
            self.penetration_depth_mm,
        )
    }

    /// The numbers a printer starts from: its own table, or else the table of the printer
    /// this resin was last tuned on, or else the resin's own numbers.
    #[must_use]
    pub fn starting_point(&self, printer_id: &str) -> Self {
        let from = match self.last_printer.as_deref() {
            Some(last) if !self.is_tuned_for(printer_id) && self.is_tuned_for(last) => last,
            _ => printer_id,
        };
        let mut start = self.for_printer(from);
        if from != printer_id {
            // Another machine's table says nothing about what this one locks.
            start.printer_table = PrinterTuning::default();
        }
        start
    }

    /// Seconds the printer stands still over one layer besides its exposure.
    pub fn wait_per_layer_s(&self) -> f32 {
        let light_off = match self.waits.mode {
            WaitMode::LightOff => self.light_off_delay_s,
            WaitMode::Rest => 0.0,
        };
        light_off + self.waits.rests_s().iter().sum::<f32>()
    }

    /// The light-off delay the printer is told, zero while the resin waits by resting.
    pub fn light_off_s(&self) -> f32 {
        match self.waits.mode {
            WaitMode::LightOff => self.light_off_delay_s,
            WaitMode::Rest => 0.0,
        }
    }

    /// The bottom layers' light-off delay the printer is told, zero while resting.
    pub fn bottom_light_off_s(&self) -> f32 {
        match self.waits.mode {
            WaitMode::LightOff => self
                .bottom_light_off_delay_s
                .unwrap_or(self.light_off_delay_s),
            WaitMode::Rest => 0.0,
        }
    }

    /// The light-off delay of layer `index`.
    pub fn light_off_of_layer_s(&self, index: u32) -> f32 {
        if self.is_bottom_layer(index) {
            self.bottom_light_off_s()
        } else {
            self.light_off_s()
        }
    }

    /// The whole climb of a normal layer's lift, both stages, mm.
    pub fn lift_total_mm(&self) -> f32 {
        self.lift_distance_mm + self.second_stage.lift_distance_mm
    }

    /// The whole climb of a bottom layer's lift, both stages, mm.
    pub fn bottom_lift_total_mm(&self) -> f32 {
        self.bottom_lift_distance_mm + self.second_stage.bottom_lift_distance_mm
    }

    /// The whole way back down after a normal lift, both stages, mm.
    pub fn retract_total_mm(&self) -> f32 {
        self.retract_distance_mm + self.second_stage.retract_distance_mm
    }

    /// How far a bottom layer's first retract stage runs, mm: all the way the lift went
    /// but the slow last stretch of the second.
    pub fn bottom_retract_distance_mm(&self) -> f32 {
        (self.bottom_lift_total_mm() - self.second_stage.bottom_retract_distance_mm).max(0.0)
    }

    /// What `weight_g` grams, `volume_mm3` cubic millimetres, of this resin cost, or
    /// `None` for a resin with no price.
    pub fn cost(&self, weight_g: f32, volume_mm3: f32) -> Option<f32> {
        let details = &self.details;
        let amount = match details.price_per {
            PriceUnit::Kilogram => weight_g / 1000.0,
            PriceUnit::Litre => volume_mm3 / 1_000_000.0,
        };
        (details.price > 0.0).then_some(details.price * amount)
    }

    /// How this resin's price is quoted, the way a sliced file states it: `€/L`, `$/kg`.
    pub fn price_label(&self) -> String {
        let per = match self.details.price_per {
            PriceUnit::Kilogram => "kg",
            PriceUnit::Litre => "L",
        };
        format!("{}/{}", self.details.currency, per)
    }

    /// Whether this resin carries numbers measured for that printer.
    pub fn is_tuned_for(&self, printer_id: &str) -> bool {
        self.printers.contains_key(printer_id)
    }

    fn validate(&self) -> Result<(), ProfileError> {
        self.validate_values()?;
        for printer in self.printers.keys() {
            self.for_printer(printer)
                .validate_values()
                .map_err(|error| match error {
                    ProfileError::NonPositive { field, value } => ProfileError::NonPositiveTuning {
                        printer: printer.clone(),
                        field,
                        value,
                    },
                    other => other,
                })?;
        }
        Ok(())
    }

    pub(crate) fn validate_values(&self) -> Result<(), ProfileError> {
        let checks = [
            ("layer_height_mm", self.layer_height_mm),
            ("exposure_s", self.exposure_s),
            ("bottom_exposure_s", self.bottom_exposure_s),
            ("lift_speed_mm_min", self.lift_speed_mm_min),
            ("retract_speed_mm_min", self.retract_speed_mm_min),
            ("bottom_lift_speed_mm_min", self.bottom_lift_speed_mm_min),
            (
                "bottom_retract_speed_mm_min",
                self.bottom_retract_speed_mm_min,
            ),
            ("density_g_cm3", self.density_g_cm3),
        ];
        for (field, value) in checks {
            if value <= 0.0 {
                return Err(ProfileError::NonPositive { field, value });
            }
        }
        let waits = &self.waits;
        let others = [
            ("waits.before_lift_s", waits.before_lift_s),
            ("waits.after_lift_s", waits.after_lift_s),
            ("waits.after_retract_s", waits.after_retract_s),
            ("details.price", self.details.price),
            (
                "bottom_light_off_delay_s",
                self.bottom_light_off_delay_s.unwrap_or(0.0),
            ),
        ];
        let others = others.into_iter().chain(self.second_stage.fields());
        for (field, value) in others {
            if value < 0.0 {
                return Err(ProfileError::Negative { field, value });
            }
        }
        Ok(())
    }
}

impl Default for MaterialProfile {
    fn default() -> Self {
        Self {
            name: "Generic resin".to_owned(),
            layer_height_mm: 0.05,
            exposure_s: 2.5,
            bottom_exposure_s: 30.0,
            bottom_layers: 6,
            light_off_delay_s: 0.5,
            lift_distance_mm: 6.0,
            lift_speed_mm_min: 65.0,
            retract_speed_mm_min: 150.0,
            retract_distance_mm: default_retract_distance_mm(),
            bottom_lift_distance_mm: default_bottom_lift_distance_mm(),
            bottom_lift_speed_mm_min: default_bottom_lift_speed_mm_min(),
            bottom_retract_speed_mm_min: default_bottom_retract_speed_mm_min(),
            second_stage: SecondStage::default(),
            bottom_light_off_delay_s: None,
            light_pwm: default_light_pwm(),
            bottom_light_pwm: default_light_pwm(),
            transition_layers: 0,
            density_g_cm3: default_density_g_cm3(),
            penetration_depth_mm: None,
            last_printer: None,
            details: ResinDetails::default(),
            waits: Waits::default(),
            compensation: Compensation::default(),
            printers: BTreeMap::new(),
            printer_table: PrinterTuning::default(),
        }
    }
}

#[cfg(test)]
mod tests;
