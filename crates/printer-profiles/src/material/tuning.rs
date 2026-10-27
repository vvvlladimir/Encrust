use serde::{Deserialize, Serialize};

use crate::{Compensation, MaterialProfile, SecondStage, WaitMode, Waits};

/// The settings of a resin that depend on the machine it is printed on.
/// Every field is optional: what a table leaves out keeps the resin's own value.
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
    pub second_stage: Option<SecondStage>,
    pub bottom_light_off_delay_s: Option<f32>,
    pub waits: Option<Waits>,
    /// How the printer waits, over `waits`, for a machine that takes one mode only and
    /// leaves the rests to the resin.
    pub wait_mode: Option<WaitMode>,
    pub compensation: Option<Compensation>,
}

impl PrinterTuning {
    /// What has to be said about `tuned` that `base` does not already say: the inverse of a tuning,
    /// turning numbers edited for one machine back into that machine's table.
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
            second_stage: changed(base.second_stage, tuned.second_stage),
            bottom_light_off_delay_s: changed(
                base.bottom_light_off_delay_s,
                tuned.bottom_light_off_delay_s,
            )
            .flatten(),
            waits: changed(base.waits, tuned.waits),
            wait_mode: None,
            compensation: changed(base.compensation, tuned.compensation),
        }
    }

    /// These locks less every field `stated` gives a value of its own: a machine's lock is
    /// the default a resin's table for it overrides; see `docs/decisions/0364`.
    #[must_use]
    pub fn unstated_by(&self, stated: &Self) -> Self {
        fn keep<T: Copy>(lock: Option<T>, stated: Option<T>) -> Option<T> {
            if stated.is_some() { None } else { lock }
        }
        Self {
            layer_height_mm: keep(self.layer_height_mm, stated.layer_height_mm),
            exposure_s: keep(self.exposure_s, stated.exposure_s),
            bottom_exposure_s: keep(self.bottom_exposure_s, stated.bottom_exposure_s),
            bottom_layers: keep(self.bottom_layers, stated.bottom_layers),
            transition_layers: keep(self.transition_layers, stated.transition_layers),
            light_off_delay_s: keep(self.light_off_delay_s, stated.light_off_delay_s),
            light_pwm: keep(self.light_pwm, stated.light_pwm),
            bottom_light_pwm: keep(self.bottom_light_pwm, stated.bottom_light_pwm),
            lift_distance_mm: keep(self.lift_distance_mm, stated.lift_distance_mm),
            lift_speed_mm_min: keep(self.lift_speed_mm_min, stated.lift_speed_mm_min),
            retract_distance_mm: keep(self.retract_distance_mm, stated.retract_distance_mm),
            retract_speed_mm_min: keep(self.retract_speed_mm_min, stated.retract_speed_mm_min),
            bottom_lift_distance_mm: keep(
                self.bottom_lift_distance_mm,
                stated.bottom_lift_distance_mm,
            ),
            bottom_lift_speed_mm_min: keep(
                self.bottom_lift_speed_mm_min,
                stated.bottom_lift_speed_mm_min,
            ),
            bottom_retract_speed_mm_min: keep(
                self.bottom_retract_speed_mm_min,
                stated.bottom_retract_speed_mm_min,
            ),
            second_stage: keep(self.second_stage, stated.second_stage),
            bottom_light_off_delay_s: keep(
                self.bottom_light_off_delay_s,
                stated.bottom_light_off_delay_s,
            ),
            waits: keep(self.waits, stated.waits),
            // The mode is what the firmware can do, which no resin changes.
            wait_mode: self.wait_mode,
            compensation: keep(self.compensation, stated.compensation),
        }
    }

    /// A table stating `resin`'s own value for every field these locks hold, so that a resin
    /// another program set up for the machine keeps what it says there.
    #[must_use]
    pub fn read_from(&self, resin: &MaterialProfile) -> Self {
        fn read<T: Copy, U>(lock: Option<U>, value: T) -> Option<T> {
            lock.map(|_| value)
        }
        Self {
            layer_height_mm: read(self.layer_height_mm, resin.layer_height_mm),
            exposure_s: read(self.exposure_s, resin.exposure_s),
            bottom_exposure_s: read(self.bottom_exposure_s, resin.bottom_exposure_s),
            bottom_layers: read(self.bottom_layers, resin.bottom_layers),
            transition_layers: read(self.transition_layers, resin.transition_layers),
            light_off_delay_s: read(self.light_off_delay_s, resin.light_off_delay_s),
            light_pwm: read(self.light_pwm, resin.light_pwm),
            bottom_light_pwm: read(self.bottom_light_pwm, resin.bottom_light_pwm),
            lift_distance_mm: read(self.lift_distance_mm, resin.lift_distance_mm),
            lift_speed_mm_min: read(self.lift_speed_mm_min, resin.lift_speed_mm_min),
            retract_distance_mm: read(self.retract_distance_mm, resin.retract_distance_mm),
            retract_speed_mm_min: read(self.retract_speed_mm_min, resin.retract_speed_mm_min),
            bottom_lift_distance_mm: read(
                self.bottom_lift_distance_mm,
                resin.bottom_lift_distance_mm,
            ),
            bottom_lift_speed_mm_min: read(
                self.bottom_lift_speed_mm_min,
                resin.bottom_lift_speed_mm_min,
            ),
            bottom_retract_speed_mm_min: read(
                self.bottom_retract_speed_mm_min,
                resin.bottom_retract_speed_mm_min,
            ),
            second_stage: read(self.second_stage, resin.second_stage),
            bottom_light_off_delay_s: self
                .bottom_light_off_delay_s
                .and(resin.bottom_light_off_delay_s),
            waits: read(self.waits, resin.waits),
            wait_mode: None,
            compensation: read(self.compensation, resin.compensation),
        }
    }

    /// Every field `other` states put over this table's own.
    #[must_use]
    pub fn and(mut self, other: &Self) -> Self {
        fn put<T: Copy>(target: &mut Option<T>, value: Option<T>) {
            if value.is_some() {
                *target = value;
            }
        }
        put(&mut self.layer_height_mm, other.layer_height_mm);
        put(&mut self.exposure_s, other.exposure_s);
        put(&mut self.bottom_exposure_s, other.bottom_exposure_s);
        put(&mut self.bottom_layers, other.bottom_layers);
        put(&mut self.transition_layers, other.transition_layers);
        put(&mut self.light_off_delay_s, other.light_off_delay_s);
        put(&mut self.light_pwm, other.light_pwm);
        put(&mut self.bottom_light_pwm, other.bottom_light_pwm);
        put(&mut self.lift_distance_mm, other.lift_distance_mm);
        put(&mut self.lift_speed_mm_min, other.lift_speed_mm_min);
        put(&mut self.retract_distance_mm, other.retract_distance_mm);
        put(&mut self.retract_speed_mm_min, other.retract_speed_mm_min);
        put(&mut self.bottom_lift_distance_mm, other.bottom_lift_distance_mm);
        put(&mut self.bottom_lift_speed_mm_min, other.bottom_lift_speed_mm_min);
        put(&mut self.bottom_retract_speed_mm_min, other.bottom_retract_speed_mm_min);
        put(&mut self.second_stage, other.second_stage);
        put(&mut self.bottom_light_off_delay_s, other.bottom_light_off_delay_s);
        put(&mut self.waits, other.waits);
        put(&mut self.wait_mode, other.wait_mode);
        put(&mut self.compensation, other.compensation);
        self
    }

    /// Whether this table says nothing at all, which is a machine on the resin's own
    /// numbers and a table not worth writing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The one way of waiting this machine takes, where it takes only one.
    pub fn held_wait_mode(&self) -> Option<WaitMode> {
        self.wait_mode
            .or_else(|| self.waits.map(|waits| waits.mode))
    }

    /// Whether this machine leaves the resin nothing to say about how it waits.
    pub fn locks_waiting(&self) -> bool {
        match self.held_wait_mode() {
            None => false,
            Some(WaitMode::LightOff) => self.light_off_delay_s.is_some(),
            Some(WaitMode::Rest) => self.waits.is_some(),
        }
    }

    /// Whether this machine runs every part of the peel itself, leaving the resin no motion.
    pub fn locks_motion(&self) -> bool {
        [
            self.lift_distance_mm,
            self.lift_speed_mm_min,
            self.retract_distance_mm,
            self.retract_speed_mm_min,
            self.bottom_lift_distance_mm,
            self.bottom_lift_speed_mm_min,
            self.bottom_retract_speed_mm_min,
        ]
        .iter()
        .all(Option::is_some)
            && self.second_stage.is_some()
    }

    pub(super) fn apply_to(&self, profile: &mut MaterialProfile) {
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
        over(&mut profile.second_stage, self.second_stage);
        if self.bottom_light_off_delay_s.is_some() {
            profile.bottom_light_off_delay_s = self.bottom_light_off_delay_s;
        }
        over(&mut profile.waits, self.waits);
        over(&mut profile.waits.mode, self.wait_mode);
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
