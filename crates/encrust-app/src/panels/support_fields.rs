//! The fields of one support profile, in the groups both the Supports tool and the
//! Settings window lay them out in.

use std::ops::RangeInclusive;

use printer_profiles::{Catalogue, Entry, SupportProfile};

use crate::i18n::tr;
use crate::ui::{Carried, carried_row, describe, field_label, theme};

mod automatic;
mod bracing;
mod sections;
use automatic::automatic;
pub use automatic::{held_up, sizes, steadying, steps};
use bracing::{bracing, bracing_fields};
use sections::{
    bottom, branching, branching_fields, grid, grid_fields, main, raft, raft_fields, small_pillar,
    top,
};

/// Millimetres per point of drag. The tip is measured in tenths and the foot in whole
/// millimetres, so they do not share a step.
const FINE_STEP: f64 = 0.01;
const COARSE_STEP: f64 = 0.05;
const ANGLE_STEP: f64 = 0.5;

/// Bounds on the density field. A tenth is a support where the part cannot do without
/// one; four is a bed of them.
const MIN_DENSITY: f32 = 0.1;
const MAX_DENSITY: f32 = 4.0;

/// Bounds on the overhang angle, degrees from vertical. Nothing is a wall and everything
/// is a ceiling, so neither end is worth reaching.
const MIN_OVERHANG_DEG: f32 = 5.0;
const MAX_OVERHANG_DEG: f32 = 85.0;

/// Bounds on how far a branch may lean, degrees from vertical. Upright never merges and
/// anything past the profile's own ceiling stops descending; see `docs/decisions/0040`.
const MIN_BRANCH_DEG: f32 = 5.0;
const MAX_BRANCH_DEG: f32 = 80.0;

/// One group of a profile's fields, from the tip down to the plate and then the run that
/// places them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Top,
    Main,
    Bottom,
    SmallPillar,
    Branching,
    Grid,
    Raft,
    Bracing,
    Automatic,
}

impl Group {
    pub fn label(self) -> String {
        match self {
            Self::Top => tr!("support-group-top"),
            Self::Main => tr!("support-group-pillar"),
            Self::Bottom => tr!("support-group-bottom"),
            Self::SmallPillar => tr!("support-group-small-pillar"),
            Self::Branching => tr!("support-group-branching"),
            Self::Grid => tr!("support-group-grid"),
            Self::Raft => tr!("support-group-raft"),
            Self::Bracing => tr!("support-group-bracing"),
            Self::Automatic => tr!("support-group-automatic"),
        }
    }

    /// The switch that turns the whole group on, which its heading carries.
    pub fn switch(self, profile: &mut SupportProfile) -> Option<&mut bool> {
        match self {
            Self::Branching => Some(&mut profile.branching.enabled),
            Self::Grid => Some(&mut profile.grid.enabled),
            Self::Raft => Some(&mut profile.raft.enabled),
            Self::Bracing => Some(&mut profile.bracing.enabled),
            _ => None,
        }
    }

    /// What a group with a switch does, and what stands in its place while it is off.
    fn help_and_off(self) -> Option<(String, String)> {
        match self {
            Self::Branching => Some((tr!("support-branching-help"), tr!("support-no-branching"))),
            Self::Grid => Some((tr!("support-grid-help"), tr!("support-no-grid"))),
            Self::Raft => Some((tr!("support-raft-help"), tr!("support-no-raft"))),
            Self::Bracing => Some((tr!("support-bracing-help"), tr!("support-no-bracing"))),
            _ => None,
        }
    }

    /// A group switched off in its heading: the line saying what stands in its place.
    pub fn off(self, ui: &mut egui::Ui) {
        if let Some((help, off)) = self.help_and_off() {
            describe(ui, &help);
            field_label(ui, &off, theme::colors().text_mid, "");
        }
    }

    /// The group's fields under a switch its heading carries, while that switch is on; a
    /// group with none draws as [`Self::show`] does.
    pub fn fields(
        self,
        ui: &mut egui::Ui,
        profile: &mut SupportProfile,
        saved: Option<&SupportProfile>,
    ) {
        if let Some((help, _)) = self.help_and_off() {
            describe(ui, &help);
        }
        match self {
            Self::Branching => branching_fields(ui, &mut profile.branching, saved),
            Self::Grid => grid_fields(ui, &mut profile.grid, saved),
            Self::Raft => raft_fields(ui, &mut profile.raft, saved),
            Self::Bracing => bracing_fields(ui, profile, saved),
            _ => return self.show(ui, profile, saved),
        }
        keep_in_order(profile);
    }

    /// Draws the group's fields, then puts the widths back in order. With `saved`, a
    /// number that has moved off it is marked and can be put back.
    pub fn show(
        self,
        ui: &mut egui::Ui,
        profile: &mut SupportProfile,
        saved: Option<&SupportProfile>,
    ) {
        match self {
            Self::Top => top(ui, profile, saved),
            Self::Main => main(ui, profile, saved),
            Self::Bottom => bottom(ui, profile, saved),
            Self::SmallPillar => small_pillar(ui, profile, saved),
            Self::Branching => branching(ui, profile, saved),
            Self::Grid => grid(ui, profile, saved),
            Self::Raft => raft(ui, profile, saved),
            Self::Bracing => bracing(ui, profile, saved),
            Self::Automatic => automatic(ui, profile, saved),
        }
        keep_in_order(profile);
    }
}

/// The profile of `catalogue` the one in hand was taken from: the saved one of its name,
/// which is what an edit is measured against and what Save writes over.
pub fn saved_as<'a>(
    catalogue: &'a Catalogue,
    profile: &SupportProfile,
) -> Option<&'a Entry<SupportProfile>> {
    catalogue
        .supports()
        .find(|entry| entry.profile.name == profile.name)
}

/// What one number is in the saved profile, where there is one to measure against.
fn was(saved: Option<&SupportProfile>, of: impl Fn(&SupportProfile) -> f32) -> Option<f32> {
    saved.map(of)
}

/// A named number of the profile. Where it has moved off what `was` saved, its label is
/// in the warning colour and a button beside it puts the saved value back.
#[allow(clippy::too_many_arguments)]
fn number(
    ui: &mut egui::Ui,
    label: &str,
    was: Option<f32>,
    value: &mut f32,
    unit: &str,
    step: f64,
    range: RangeInclusive<f32>,
    decimals: usize,
) {
    // Half of the last digit shown: what reads the same is the same.
    let shown = 0.5 * 10f32.powi(-(decimals as i32));
    let moved = was.filter(|was| (was - *value).abs() >= shown);
    if carried_row(ui, label, value, unit, step, range, decimals, moved) == Carried::Reverted
        && let Some(was) = moved
    {
        *value = was;
    }
}

fn mm(
    ui: &mut egui::Ui,
    label: &str,
    was: Option<f32>,
    value: &mut f32,
    step: f64,
    range: RangeInclusive<f32>,
) {
    number(ui, label, was, value, &tr!("unit-mm"), step, range, 2);
}

fn degrees(
    ui: &mut egui::Ui,
    label: &str,
    was: Option<f32>,
    value: &mut f32,
    range: RangeInclusive<f32>,
) {
    number(
        ui,
        label,
        was,
        value,
        &tr!("unit-deg"),
        ANGLE_STEP,
        range,
        0,
    );
}

/// A ring wider than the one above it turns a support inside out where the two cross.
/// The fields are independent, so the order is restored here rather than forbidden there.
pub fn keep_in_order(profile: &mut SupportProfile) {
    profile.branching.max_trunk_diameter_mm = profile
        .branching
        .max_trunk_diameter_mm
        .max(profile.middle.diameter_mm);
    profile.bottom.lower_diameter_mm = profile
        .bottom
        .lower_diameter_mm
        .max(profile.middle.diameter_mm);
    profile.bottom.platform_diameter_mm = profile
        .bottom
        .platform_diameter_mm
        .max(profile.branching.max_trunk_diameter_mm)
        .max(profile.bottom.lower_diameter_mm);
    // A thinned pillar wider than the one it replaces is not a thinning.
    profile.small_pillar.diameter_mm = profile
        .small_pillar
        .diameter_mm
        .min(profile.middle.diameter_mm);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edited_profile_is_measured_against_the_saved_one_of_its_name() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue loads");
        let mut edited = SupportProfile::shipped("medium").expect("the medium preset ships");
        edited.bracing.diameter_mm += 0.3;

        let saved = saved_as(&catalogue, &edited).expect("Medium is shipped");
        assert_eq!(
            saved.profile,
            SupportProfile::shipped("medium").expect("the medium preset ships")
        );
        assert_eq!(
            was(Some(&saved.profile), |profile| profile.bracing.diameter_mm),
            Some(
                SupportProfile::shipped("medium")
                    .expect("the medium preset ships")
                    .bracing
                    .diameter_mm
            ),
            "which is what the field's button puts back"
        );
    }

    #[test]
    fn a_profile_of_a_name_nobody_saved_has_nothing_to_go_back_to() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue loads");
        let mut renamed = SupportProfile::shipped("medium").expect("the medium preset ships");
        renamed.name = "Nobody's".to_owned();
        assert!(saved_as(&catalogue, &renamed).is_none());
    }
}
