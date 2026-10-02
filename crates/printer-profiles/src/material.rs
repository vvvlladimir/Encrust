use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Compensation;
use crate::ProfileError;

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
    /// Light penetration depth `Dp` of the resin's working curve, millimetres, where it
    /// has been measured. A resin that states none is carried on
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
}

/// The settings of a resin that depend on the machine it is printed on.
///
/// Every field is optional: what a table leaves out keeps the resin's own value. The
/// resin's name and density are not here, because neither changes with the printer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrinterTuning {
    pub layer_height_mm: Option<f32>,
    pub exposure_s: Option<f32>,
    pub bottom_exposure_s: Option<f32>,
    pub bottom_layers: Option<u32>,
    pub transition_layers: Option<u16>,
    pub light_off_delay_s: Option<f32>,
    pub light_pwm: Option<u8>,
    pub bottom_light_pwm: Option<u8>,
    pub lift_distance_mm: Option<f32>,
    pub lift_speed_mm_min: Option<f32>,
    pub retract_distance_mm: Option<f32>,
    pub retract_speed_mm_min: Option<f32>,
    pub bottom_lift_distance_mm: Option<f32>,
    pub bottom_lift_speed_mm_min: Option<f32>,
    pub bottom_retract_speed_mm_min: Option<f32>,
    pub waits: Option<Waits>,
    pub compensation: Option<Compensation>,
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

impl PrinterTuning {
    /// What has to be said about `tuned` that `base` does not already say.
    ///
    /// This is the inverse of applying a tuning: it is how the window turns a set of
    /// numbers the user edited for one machine back into that machine's table, leaving
    /// the resin's own numbers and every other machine's table alone.
    pub fn of_changes(base: &MaterialProfile, tuned: &MaterialProfile) -> Self {
        Self {
            layer_height_mm: changed(base.layer_height_mm, tuned.layer_height_mm),
            exposure_s: changed(base.exposure_s, tuned.exposure_s),
            bottom_exposure_s: changed(base.bottom_exposure_s, tuned.bottom_exposure_s),
            bottom_layers: changed(base.bottom_layers, tuned.bottom_layers),
            transition_layers: changed(base.transition_layers, tuned.transition_layers),
            light_off_delay_s: changed(base.light_off_delay_s, tuned.light_off_delay_s),
            light_pwm: changed(base.light_pwm, tuned.light_pwm),
            bottom_light_pwm: changed(base.bottom_light_pwm, tuned.bottom_light_pwm),
            lift_distance_mm: changed(base.lift_distance_mm, tuned.lift_distance_mm),
            lift_speed_mm_min: changed(base.lift_speed_mm_min, tuned.lift_speed_mm_min),
            retract_distance_mm: changed(base.retract_distance_mm, tuned.retract_distance_mm),
            retract_speed_mm_min: changed(base.retract_speed_mm_min, tuned.retract_speed_mm_min),
            bottom_lift_distance_mm: changed(
                base.bottom_lift_distance_mm,
                tuned.bottom_lift_distance_mm,
            ),
            bottom_lift_speed_mm_min: changed(
                base.bottom_lift_speed_mm_min,
                tuned.bottom_lift_speed_mm_min,
            ),
            bottom_retract_speed_mm_min: changed(
                base.bottom_retract_speed_mm_min,
                tuned.bottom_retract_speed_mm_min,
            ),
            waits: changed(base.waits, tuned.waits),
            compensation: changed(base.compensation, tuned.compensation),
        }
    }

    /// Whether this table says nothing at all, which is a machine on the resin's own
    /// numbers and a table not worth writing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn apply_to(&self, profile: &mut MaterialProfile) {
        over(&mut profile.layer_height_mm, self.layer_height_mm);
        over(&mut profile.exposure_s, self.exposure_s);
        over(&mut profile.bottom_exposure_s, self.bottom_exposure_s);
        over(&mut profile.bottom_layers, self.bottom_layers);
        over(&mut profile.transition_layers, self.transition_layers);
        over(&mut profile.light_off_delay_s, self.light_off_delay_s);
        over(&mut profile.light_pwm, self.light_pwm);
        over(&mut profile.bottom_light_pwm, self.bottom_light_pwm);
        over(&mut profile.lift_distance_mm, self.lift_distance_mm);
        over(&mut profile.lift_speed_mm_min, self.lift_speed_mm_min);
        over(&mut profile.retract_distance_mm, self.retract_distance_mm);
        over(&mut profile.retract_speed_mm_min, self.retract_speed_mm_min);
        over(
            &mut profile.bottom_lift_distance_mm,
            self.bottom_lift_distance_mm,
        );
        over(
            &mut profile.bottom_lift_speed_mm_min,
            self.bottom_lift_speed_mm_min,
        );
        over(
            &mut profile.bottom_retract_speed_mm_min,
            self.bottom_retract_speed_mm_min,
        );
        over(&mut profile.waits, self.waits);
        over(&mut profile.compensation, self.compensation);
    }
}

fn changed<T: Copy + PartialEq>(base: T, tuned: T) -> Option<T> {
    (base != tuned).then_some(tuned)
}

fn over<T: Copy>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
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
        self.validate()?;
        let source = toml::to_string_pretty(self).map_err(|source| ProfileError::Serialise {
            path: path.to_owned(),
            source,
        })?;
        std::fs::write(path, source).map_err(|source| ProfileError::Io {
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
    ///
    /// Transition layers ramp linearly from the bottom exposure to the normal one, which
    /// is what every MSLA slicer does and what the printer expects to see per layer.
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

    /// This resin as the given printer needs it, with that printer's tuning applied.
    ///
    /// A printer the resin says nothing about gets the untuned numbers, which are a
    /// starting point rather than a calibration; `is_tuned_for` tells the two apart.
    #[must_use]
    pub fn for_printer(&self, printer_id: &str) -> Self {
        let mut tuned = self.clone();
        tuned.printers = BTreeMap::new();
        if let Some(tuning) = self.printers.get(printer_id) {
            tuning.apply_to(&mut tuned);
        }
        tuned
    }

    /// This resin as measured at `layer_height_mm`: the normal exposure is carried along
    /// the working curve, so a layer of the new height cures as a layer of the old one did.
    /// The bottom block keeps its seconds, being exposed to stick rather than to cure
    /// through; see `docs/decisions/0128-the-exposure-follows-the-layer-in-sight.md`.
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
        self.for_printer(from)
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

    fn validate_values(&self) -> Result<(), ProfileError> {
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
        ];
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resin_carried_to_another_height_cures_every_layer_as_before() {
        for penetration_depth_mm in [None, Some(0.12)] {
            let resin = MaterialProfile {
                penetration_depth_mm,
                ..MaterialProfile::default()
            };
            let carried = resin.rescaled_to(0.03);
            assert!((carried.layer_height_mm - 0.03).abs() < f32::EPSILON);
            for thickness_mm in [0.02, 0.03, 0.05, 0.1] {
                let before = resin.exposure_at(resin.exposure_s, thickness_mm);
                let after = carried.exposure_at(carried.exposure_s, thickness_mm);
                assert!(
                    (before - after).abs() < 1e-4,
                    "at {thickness_mm} mm: {before} s before, {after} s after"
                );
            }
        }
    }

    #[test]
    fn carrying_a_resin_to_another_height_leaves_the_bottom_block_alone() {
        let resin = MaterialProfile::default();
        let carried = resin.rescaled_to(0.1);
        assert!(
            carried.exposure_s > resin.exposure_s,
            "a thicker layer takes longer"
        );
        assert!((carried.bottom_exposure_s - resin.bottom_exposure_s).abs() < f32::EPSILON);
    }

    fn path() -> &'static Path {
        Path::new("inline.toml")
    }

    #[test]
    fn default_survives_a_toml_round_trip() {
        let original = MaterialProfile::default();
        let text = toml::to_string(&original).expect("serialises");
        let parsed: MaterialProfile = toml::from_str(&text).expect("parses back");
        assert_eq!(original, parsed);
    }

    #[test]
    fn bottom_layers_are_exposed_longer() {
        let profile = MaterialProfile::default();
        assert!(profile.bottom_exposure_s > profile.exposure_s);
    }

    #[test]
    fn a_profile_without_the_motion_fields_takes_the_defaults() {
        let profile = MaterialProfile::from_toml_str(
            r#"
name = "Old profile"
layer_height_mm = 0.05
exposure_s = 2.5
bottom_exposure_s = 30.0
bottom_layers = 6
light_off_delay_s = 0.5
lift_distance_mm = 6.0
lift_speed_mm_min = 65.0
retract_speed_mm_min = 150.0
"#,
            path(),
        )
        .expect("valid profile");

        assert_eq!(profile.light_pwm, 255);
        assert_eq!(profile.transition_layers, 0);
        assert!((profile.retract_distance_mm - 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn zero_exposure_is_rejected() {
        let err = MaterialProfile::from_toml_str(
            r#"
name = "Broken"
layer_height_mm = 0.05
exposure_s = 0.0
bottom_exposure_s = 30.0
bottom_layers = 6
light_off_delay_s = 0.5
lift_distance_mm = 6.0
lift_speed_mm_min = 65.0
retract_speed_mm_min = 150.0
"#,
            path(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProfileError::NonPositive {
                field: "exposure_s",
                ..
            }
        ));
    }

    #[test]
    fn a_negative_rest_is_rejected() {
        let resin = MaterialProfile {
            waits: Waits {
                after_lift_s: -1.0,
                ..Waits::default()
            },
            ..MaterialProfile::default()
        };
        let text = toml::to_string(&resin).expect("serialises");
        let error = MaterialProfile::from_toml_str(&text, path()).expect_err("rejected");
        assert!(matches!(
            error,
            ProfileError::Negative {
                field: "waits.after_lift_s",
                ..
            }
        ));
    }

    #[test]
    fn a_printer_not_yet_set_up_starts_from_the_last_one_tuned() {
        let mut resin = MaterialProfile::default();
        resin.printers.insert(
            "mars".to_owned(),
            PrinterTuning {
                exposure_s: Some(3.3),
                ..PrinterTuning::default()
            },
        );
        resin.last_printer = Some("mars".to_owned());

        let fresh = resin.starting_point("saturn");
        assert!(
            (fresh.exposure_s - 3.3).abs() < 1e-6,
            "the Mars numbers carry over"
        );
        assert!(
            !resin.is_tuned_for("saturn"),
            "starting from them tunes nothing yet"
        );

        resin.last_printer = None;
        let bare = resin.starting_point("saturn");
        assert!((bare.exposure_s - MaterialProfile::default().exposure_s).abs() < 1e-6);
    }

    #[test]
    fn resting_replaces_the_light_off_delay() {
        let mut resin = MaterialProfile::default();
        assert!((resin.wait_per_layer_s() - resin.light_off_delay_s).abs() < 1e-6);
        resin.waits = Waits {
            mode: WaitMode::Rest,
            before_lift_s: 1.0,
            after_lift_s: 0.5,
            after_retract_s: 2.0,
        };
        assert!((resin.wait_per_layer_s() - 3.5).abs() < 1e-6);
        assert!(resin.light_off_s().abs() < 1e-6);
    }

    #[test]
    fn a_resin_is_priced_by_weight_or_by_volume() {
        let mut resin = MaterialProfile::default();
        assert_eq!(resin.cost(500.0, 450_000.0), None, "no price, no cost");
        resin.details.price = 20.0;
        let by_weight = resin.cost(500.0, 450_000.0).expect("priced");
        assert!((by_weight - 10.0).abs() < 1e-4, "half a kilo at 20 a kilo");
        resin.details.price_per = PriceUnit::Litre;
        let by_volume = resin.cost(500.0, 450_000.0).expect("priced");
        assert!((by_volume - 9.0).abs() < 1e-4, "0.45 l at 20 a litre");
    }

    #[test]
    fn a_tuning_carries_its_own_waits() {
        let base = MaterialProfile::default();
        let tuned = MaterialProfile {
            waits: Waits {
                mode: WaitMode::Rest,
                after_retract_s: 1.0,
                ..Waits::default()
            },
            ..base.clone()
        };
        let tuning = PrinterTuning::of_changes(&base, &tuned);
        let mut resin = base;
        resin.printers.insert("mars".to_owned(), tuning);
        assert_eq!(resin.for_printer("mars").waits, tuned.waits);
        let text = toml::to_string_pretty(&resin).expect("tables after values");
        assert_eq!(
            MaterialProfile::from_toml_str(&text, path()).expect("loads"),
            resin
        );
    }

    #[test]
    fn malformed_toml_reports_its_path() {
        let err = MaterialProfile::from_toml_str("name = ", path()).unwrap_err();
        assert!(matches!(err, ProfileError::Parse { .. }));
    }

    #[test]
    fn without_transition_layers_the_exposure_steps_straight_down() {
        let profile = MaterialProfile::default();
        assert!((profile.exposure_of_layer_s(5) - 30.0).abs() < 1e-6);
        assert!((profile.exposure_of_layer_s(6) - 2.5).abs() < 1e-6);
        assert!(profile.is_bottom_layer(5));
        assert!(!profile.is_bottom_layer(6));
    }

    #[test]
    fn transition_layers_ramp_from_the_bottom_exposure_to_the_normal_one() {
        let profile = MaterialProfile {
            bottom_layers: 2,
            bottom_exposure_s: 10.0,
            exposure_s: 2.0,
            transition_layers: 3,
            ..MaterialProfile::default()
        };

        // Four equal steps of -2 s across three transition layers: 8, 6, 4, then 2.
        assert!((profile.exposure_of_layer_s(2) - 8.0).abs() < 1e-5);
        assert!((profile.exposure_of_layer_s(3) - 6.0).abs() < 1e-5);
        assert!((profile.exposure_of_layer_s(4) - 4.0).abs() < 1e-5);
        assert!((profile.exposure_of_layer_s(5) - 2.0).abs() < 1e-5);
    }

    const TUNED: &str = r#"
name = "Tuned"
layer_height_mm = 0.05
exposure_s = 2.5
bottom_exposure_s = 30.0
bottom_layers = 6
light_off_delay_s = 0.5
lift_distance_mm = 6.0
lift_speed_mm_min = 65.0
retract_speed_mm_min = 150.0

[printers.fast-machine]
exposure_s = 1.8
lift_speed_mm_min = 120.0
"#;

    #[test]
    fn a_tuning_replaces_only_the_settings_it_names() {
        let resin = MaterialProfile::from_toml_str(TUNED, path()).expect("valid profile");
        let tuned = resin.for_printer("fast-machine");

        assert!((tuned.exposure_s - 1.8).abs() < 1e-6);
        assert!((tuned.lift_speed_mm_min - 120.0).abs() < 1e-6);
        assert!((tuned.bottom_exposure_s - 30.0).abs() < 1e-6);
        assert_eq!(tuned.name, resin.name);
        assert!(resin.is_tuned_for("fast-machine"));
    }

    #[test]
    fn a_printer_the_resin_says_nothing_about_keeps_the_resins_own_numbers() {
        let resin = MaterialProfile::from_toml_str(TUNED, path()).expect("valid profile");
        let tuned = resin.for_printer("other-machine");

        assert!((tuned.exposure_s - resin.exposure_s).abs() < 1e-6);
        assert!(!resin.is_tuned_for("other-machine"));
    }

    #[test]
    fn a_tuning_read_back_off_two_profiles_reproduces_them() {
        let resin = MaterialProfile::from_toml_str(TUNED, path()).expect("valid profile");
        let mut edited = resin.for_printer("fast-machine");
        edited.bottom_exposure_s = 21.0;

        let tuning = PrinterTuning::of_changes(&resin, &edited);
        assert!(!tuning.is_empty());
        assert_eq!(tuning.bottom_exposure_s, Some(21.0));
        assert_eq!(
            tuning.bottom_layers, None,
            "a setting the user did not touch stays the resin's own"
        );

        let mut round_tripped = resin.clone();
        round_tripped
            .printers
            .insert("fast-machine".to_owned(), tuning);
        assert_eq!(round_tripped.for_printer("fast-machine"), edited);
    }

    #[test]
    fn an_unedited_resin_needs_no_table() {
        let resin = MaterialProfile::default();
        assert!(PrinterTuning::of_changes(&resin, &resin).is_empty());
    }

    #[test]
    fn a_zero_in_a_tuning_is_rejected_and_names_its_printer() {
        let broken = TUNED.replace("exposure_s = 1.8", "exposure_s = 0.0");
        let err = MaterialProfile::from_toml_str(&broken, path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::NonPositiveTuning {
                field: "exposure_s",
                ..
            }
        ));
    }

    #[test]
    fn a_misspelled_tuning_key_is_rejected_rather_than_ignored() {
        let broken = TUNED.replace("lift_speed_mm_min = 120.0", "lift_speed = 120.0");
        let err = MaterialProfile::from_toml_str(&broken, path()).unwrap_err();
        assert!(matches!(err, ProfileError::Parse { .. }));
    }
}
