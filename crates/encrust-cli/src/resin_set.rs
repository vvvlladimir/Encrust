//! The window's Print settings for one run: exposure, motion and waits typed over the resin's
//! own numbers, which the resin file keeps.

use printer_profiles::{MaterialProfile, WaitMode};

/// Print settings typed for this run, each absent one the resin's.
#[derive(Debug, Clone, Default, clap::Args)]
#[command(next_help_heading = "Exposure and motion")]
pub struct ResinSet {
    /// Seconds each layer above the bottom ones is lit.
    #[arg(long, value_name = "S", value_parser = not_negative)]
    pub exposure: Option<f32>,

    /// Seconds each bottom layer is lit.
    #[arg(long, value_name = "S", value_parser = not_negative)]
    pub bottom_exposure: Option<f32>,

    /// How many layers count as bottom layers.
    #[arg(long, value_name = "N")]
    pub bottom_layers: Option<u32>,

    /// Layers the exposure steps down over from the bottom one to the normal one.
    #[arg(long, value_name = "N")]
    pub transition_layers: Option<u16>,

    /// Seconds the light stays off between layers.
    #[arg(long, value_name = "S", value_parser = not_negative)]
    pub light_off_delay: Option<f32>,

    /// How far the plate lifts after each layer, millimetres.
    #[arg(long, value_name = "MM", value_parser = not_negative)]
    pub lift_distance: Option<f32>,

    /// How fast it lifts, millimetres a minute.
    #[arg(long, value_name = "MM/MIN", value_parser = not_negative)]
    pub lift_speed: Option<f32>,

    /// How fast it comes back down, millimetres a minute.
    #[arg(long, value_name = "MM/MIN", value_parser = not_negative)]
    pub retract_speed: Option<f32>,

    /// Seconds the printer rests before each lift; any wait makes it rest rather than hold
    /// the light off.
    #[arg(long, value_name = "S", value_parser = not_negative)]
    pub wait_before_lift: Option<f32>,

    /// Seconds it rests after each lift.
    #[arg(long, value_name = "S", value_parser = not_negative)]
    pub wait_after_lift: Option<f32>,

    /// Seconds it rests after each retract.
    #[arg(long, value_name = "S", value_parser = not_negative)]
    pub wait_after_retract: Option<f32>,

    /// The light's power over the layers, 0 to 255, where the printer takes it.
    #[arg(long, value_name = "0-255")]
    pub light_power: Option<u8>,

    /// The light's power over the bottom layers, 0 to 255.
    #[arg(long, value_name = "0-255")]
    pub bottom_light_power: Option<u8>,
}

impl ResinSet {
    /// `material` with every number typed here in place of its own.
    pub fn apply(&self, material: &mut MaterialProfile) {
        let set = |into: &mut f32, value: Option<f32>| *into = value.unwrap_or(*into);
        set(&mut material.exposure_s, self.exposure);
        set(&mut material.bottom_exposure_s, self.bottom_exposure);
        set(&mut material.light_off_delay_s, self.light_off_delay);
        set(&mut material.lift_distance_mm, self.lift_distance);
        set(&mut material.lift_speed_mm_min, self.lift_speed);
        set(&mut material.retract_speed_mm_min, self.retract_speed);
        material.bottom_layers = self.bottom_layers.unwrap_or(material.bottom_layers);
        material.transition_layers = self.transition_layers.unwrap_or(material.transition_layers);
        material.light_pwm = self.light_power.unwrap_or(material.light_pwm);
        material.bottom_light_pwm = self.bottom_light_power.unwrap_or(material.bottom_light_pwm);
        let waits = [
            self.wait_before_lift,
            self.wait_after_lift,
            self.wait_after_retract,
        ];
        if waits.iter().any(Option::is_some) {
            let rests = &mut material.waits;
            rests.mode = WaitMode::Rest;
            set(&mut rests.before_lift_s, waits[0]);
            set(&mut rests.after_lift_s, waits[1]);
            set(&mut rests.after_retract_s, waits[2]);
        }
    }
}

/// A time, distance or speed: finite and not below nought.
fn not_negative(text: &str) -> Result<f32, String> {
    let value: f32 = text.trim().parse().map_err(|error| format!("{error}"))?;
    (value.is_finite() && value >= 0.0)
        .then_some(value)
        .ok_or_else(|| format!("not below nought, got {text:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_typed_replace_the_resins_and_the_rest_stay() {
        let mut material = MaterialProfile::default();
        let before = material.clone();
        let set = ResinSet {
            exposure: Some(2.5),
            bottom_layers: Some(6),
            wait_after_lift: Some(1.0),
            ..ResinSet::default()
        };
        set.apply(&mut material);
        assert_eq!(material.exposure_s, 2.5);
        assert_eq!(material.bottom_layers, 6);
        assert_eq!(material.waits.mode, WaitMode::Rest, "a wait makes it rest");
        assert_eq!(material.waits.after_lift_s, 1.0);
        assert_eq!(material.bottom_exposure_s, before.bottom_exposure_s);
        assert_eq!(material.lift_speed_mm_min, before.lift_speed_mm_min);
    }

    #[test]
    fn a_negative_time_is_refused() {
        assert!(not_negative("-1").is_err());
        assert_eq!(not_negative("0"), Ok(0.0));
    }
}
