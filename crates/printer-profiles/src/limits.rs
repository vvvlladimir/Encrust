//! What a machine's own slicer profile lets each resin setting take, and the narrower span
//! its maker advises. See `docs/decisions/0369-a-printer-states-the-limits-of-a-resin.md`.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::{MaterialProfile, ProfileError, SecondStage};

/// The span one setting may take on a machine, and the span its maker advises within it.
/// A bound left out does not hold.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn_min: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn_max: Option<f32>,
}

/// How far a value strays from its limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stray {
    /// Past what the machine takes: the window does not let it be typed.
    Refused,
    /// Inside what the machine takes, outside what its maker advises.
    Advised,
}

impl Limit {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The span a field offers: `fallback` with the bounds this limit states put in.
    pub fn range(&self, fallback: RangeInclusive<f32>) -> RangeInclusive<f32> {
        self.min.unwrap_or(*fallback.start())..=self.max.unwrap_or(*fallback.end())
    }

    /// How `value` strays, if it does.
    pub fn stray(&self, value: f32) -> Option<Stray> {
        let outside = |low: Option<f32>, high: Option<f32>| {
            low.is_some_and(|low| value < low) || high.is_some_and(|high| value > high)
        };
        if outside(self.min, self.max) {
            Some(Stray::Refused)
        } else if outside(self.warn_min, self.warn_max) {
            Some(Stray::Advised)
        } else {
            None
        }
    }

    /// The span `stray` holds the value to: the hard one when refused, else the advised one.
    pub fn span(&self, stray: Stray) -> (Option<f32>, Option<f32>) {
        match stray {
            Stray::Refused => (self.min, self.max),
            Stray::Advised => (self.warn_min, self.warn_max),
        }
    }

    /// Whether the bounds run in order: `min`, `warn_min`, `warn_max`, `max`.
    fn is_ordered(&self) -> bool {
        let bounds = [self.min, self.warn_min, self.warn_max, self.max];
        let stated: Vec<f32> = bounds.into_iter().flatten().collect();
        stated.windows(2).all(|pair| pair[0] <= pair[1])
    }
}

/// The limits of the second stage of a peel, field for field as `SecondStage`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecondStageLimits {
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub lift_distance_mm: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub lift_speed_mm_min: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub retract_distance_mm: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub retract_speed_mm_min: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_lift_distance_mm: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_lift_speed_mm_min: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_retract_distance_mm: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_retract_speed_mm_min: Limit,
}

/// The limits a machine puts on a resin's settings, keyed as `PrinterTuning` is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PrinterLimits {
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub layer_height_mm: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub exposure_s: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_exposure_s: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_layers: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub transition_layers: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub light_off_delay_s: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_light_off_delay_s: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub lift_distance_mm: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub lift_speed_mm_min: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub retract_speed_mm_min: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_lift_distance_mm: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_lift_speed_mm_min: Limit,
    #[serde(skip_serializing_if = "Limit::is_empty")]
    pub bottom_retract_speed_mm_min: Limit,
    #[serde(skip_serializing_if = "SecondStageLimits::is_empty")]
    pub second_stage: SecondStageLimits,
}

/// A setting of a resin that strays from the machine's limit for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitBreach {
    /// The setting, as the profile names it.
    pub field: &'static str,
    pub value: f32,
    pub stray: Stray,
    /// The bounds it strays from, as `Limit::span` gives them.
    pub span: (Option<f32>, Option<f32>),
}

impl PrinterLimits {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Every setting of `material` that strays from its limit here, in profile order.
    pub fn breaches(&self, material: &MaterialProfile) -> Vec<LimitBreach> {
        self.pairs(material)
            .into_iter()
            .filter_map(|(field, limit, value)| {
                let stray = limit.stray(value)?;
                Some(LimitBreach {
                    field,
                    value,
                    stray,
                    span: limit.span(stray),
                })
            })
            .collect()
    }

    /// Refuses a limit whose bounds run out of order, naming it.
    pub(crate) fn validate(&self) -> Result<(), ProfileError> {
        let material = MaterialProfile::default();
        match self
            .pairs(&material)
            .into_iter()
            .find(|(_, limit, _)| !limit.is_ordered())
        {
            Some((field, _, _)) => Err(ProfileError::DisorderedLimit { field }),
            None => Ok(()),
        }
    }

    fn pairs<'a>(&'a self, material: &MaterialProfile) -> Vec<(&'static str, &'a Limit, f32)> {
        let own = [
            (
                "layer_height_mm",
                &self.layer_height_mm,
                material.layer_height_mm,
            ),
            ("exposure_s", &self.exposure_s, material.exposure_s),
            (
                "bottom_exposure_s",
                &self.bottom_exposure_s,
                material.bottom_exposure_s,
            ),
            (
                "bottom_layers",
                &self.bottom_layers,
                material.bottom_layers as f32,
            ),
            (
                "transition_layers",
                &self.transition_layers,
                f32::from(material.transition_layers),
            ),
            (
                "light_off_delay_s",
                &self.light_off_delay_s,
                material.light_off_delay_s,
            ),
            (
                "bottom_light_off_delay_s",
                &self.bottom_light_off_delay_s,
                material.bottom_light_off_s(),
            ),
            (
                "lift_distance_mm",
                &self.lift_distance_mm,
                material.lift_distance_mm,
            ),
            (
                "lift_speed_mm_min",
                &self.lift_speed_mm_min,
                material.lift_speed_mm_min,
            ),
            (
                "retract_speed_mm_min",
                &self.retract_speed_mm_min,
                material.retract_speed_mm_min,
            ),
            (
                "bottom_lift_distance_mm",
                &self.bottom_lift_distance_mm,
                material.bottom_lift_distance_mm,
            ),
            (
                "bottom_lift_speed_mm_min",
                &self.bottom_lift_speed_mm_min,
                material.bottom_lift_speed_mm_min,
            ),
            (
                "bottom_retract_speed_mm_min",
                &self.bottom_retract_speed_mm_min,
                material.bottom_retract_speed_mm_min,
            ),
        ];
        let stage = self.second_stage.pairs(&material.second_stage);
        own.into_iter().chain(stage).collect()
    }
}

impl SecondStageLimits {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Each limit beside its field of `stage`, which `SecondStage::fields` lists in this order.
    fn pairs<'a>(
        &'a self,
        stage: &SecondStage,
    ) -> impl Iterator<Item = (&'static str, &'a Limit, f32)> {
        let limits = [
            &self.lift_distance_mm,
            &self.lift_speed_mm_min,
            &self.retract_distance_mm,
            &self.retract_speed_mm_min,
            &self.bottom_lift_distance_mm,
            &self.bottom_lift_speed_mm_min,
            &self.bottom_retract_distance_mm,
            &self.bottom_retract_speed_mm_min,
        ];
        stage
            .fields()
            .into_iter()
            .zip(limits)
            .map(|((field, value), limit)| (field, limit, value))
    }
}

#[cfg(test)]
mod tests;
