//! The fields of one support profile, in the groups both the Supports tool and the
//! Settings window lay them out in.

use std::ops::RangeInclusive;

use printer_profiles::{
    Catalogue, ContactShape, Entry, PlatformShape, RaftShape, RaftStyle, SupportProfile,
};

use crate::i18n::tr;
use crate::ui::{Carried, Segment, Segmented, carried_row, describe, nested, switch};

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
    Raft,
    Bracing,
    Automatic,
}

impl Group {
    pub const ALL: [Self; 8] = [
        Self::Top,
        Self::Main,
        Self::Bottom,
        Self::SmallPillar,
        Self::Branching,
        Self::Raft,
        Self::Bracing,
        Self::Automatic,
    ];

    pub fn label(self) -> String {
        match self {
            Self::Top => tr!("support-group-top"),
            Self::Main => tr!("support-group-pillar"),
            Self::Bottom => tr!("support-group-bottom"),
            Self::SmallPillar => tr!("support-group-small-pillar"),
            Self::Branching => tr!("support-group-branching"),
            Self::Raft => tr!("support-group-raft"),
            Self::Bracing => tr!("support-group-bracing"),
            Self::Automatic => tr!("support-group-automatic"),
        }
    }

    /// The switch that turns the whole group on, which its heading carries.
    pub fn switch(self, profile: &mut SupportProfile) -> Option<&mut bool> {
        match self {
            Self::Branching => Some(&mut profile.branching.enabled),
            _ => None,
        }
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

/// The contact with the model and the cone that widens out of it.
fn top(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    describe(ui, &tr!("support-tip-help"));
    let segments = [
        Segment::new(ContactShape::Plane, tr!("support-shape-plane")),
        Segment::new(ContactShape::Sphere, tr!("support-shape-sphere")),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut profile.tip.shape);
    ui.add_space(4.0);
    let tip = &mut profile.tip;
    mm(
        ui,
        &tr!("support-contact"),
        was(saved, |p| p.tip.contact_diameter_mm),
        &mut tip.contact_diameter_mm,
        FINE_STEP,
        0.05..=2.0,
    );
    mm(
        ui,
        &tr!("support-bite"),
        was(saved, |p| p.tip.contact_depth_mm),
        &mut tip.contact_depth_mm,
        FINE_STEP,
        0.05..=1.0,
    );
    let length = &mut profile.top.length_mm;
    mm(
        ui,
        &tr!("support-length"),
        was(saved, |p| p.top.length_mm),
        length,
        COARSE_STEP,
        0.2..=10.0,
    );
}

/// The column itself, the widest its trunk grows, and the air it keeps from the model.
fn main(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    describe(ui, &tr!("support-body-help"));
    mm(
        ui,
        &tr!("support-diameter"),
        was(saved, |p| p.middle.diameter_mm),
        &mut profile.middle.diameter_mm,
        COARSE_STEP,
        0.3..=6.0,
    );
    mm(
        ui,
        &tr!("support-clearance"),
        was(saved, |p| p.clearance_mm),
        &mut profile.clearance_mm,
        FINE_STEP,
        0.0..=3.0,
    );
    // Every trunk widens towards its base, branched or not (ADR 0242).
    mm(
        ui,
        &tr!("support-trunk-cap"),
        was(saved, |p| p.branching.max_trunk_diameter_mm),
        &mut profile.branching.max_trunk_diameter_mm,
        COARSE_STEP,
        0.3..=10.0,
    );
}

/// The foot the column stands on.
fn bottom(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    describe(ui, &tr!("support-foot-help"));
    let segments = [
        Segment::new(PlatformShape::Cylinder, tr!("support-foot-round")),
        Segment::new(PlatformShape::Cone, tr!("support-foot-tapered")),
        Segment::new(PlatformShape::Prism, tr!("support-foot-prism")),
        Segment::new(PlatformShape::Cube, tr!("support-foot-cube")),
        Segment::new(PlatformShape::Skate, tr!("support-foot-skate")),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut profile.bottom.shape);
    ui.add_space(4.0);
    let bottom = &mut profile.bottom;
    mm(
        ui,
        &tr!("support-diameter"),
        was(saved, |p| p.bottom.platform_diameter_mm),
        &mut bottom.platform_diameter_mm,
        COARSE_STEP,
        1.0..=30.0,
    );
    mm(
        ui,
        &tr!("support-thickness"),
        was(saved, |p| p.bottom.platform_thickness_mm),
        &mut bottom.platform_thickness_mm,
        COARSE_STEP,
        0.2..=5.0,
    );
    mm(
        ui,
        &tr!("support-flare"),
        was(saved, |p| p.bottom.lower_diameter_mm),
        &mut bottom.lower_diameter_mm,
        COARSE_STEP,
        0.1..=15.0,
    );
}

/// Whether a support may stand on the part at all, and the thin strut between two parts
/// of the model, which needs neither a foot nor the width a column standing on the plate
/// needs.
fn small_pillar(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    switch(
        ui,
        &mut profile.land_on_model,
        &tr!("support-land-on-model"),
    );
    if !profile.land_on_model {
        describe(ui, &tr!("support-land-on-model-off"));
        return;
    }
    let pillar = &mut profile.small_pillar;
    switch(ui, &mut pillar.enabled, &tr!("support-thin-down"));
    if !pillar.enabled {
        describe(ui, &tr!("support-thin-down-off"));
        return;
    }
    nested(ui, |ui| {
        describe(ui, &tr!("support-small-pillar-note"));
        mm(
            ui,
            &tr!("support-diameter"),
            was(saved, |p| p.small_pillar.diameter_mm),
            &mut pillar.diameter_mm,
            FINE_STEP,
            0.1..=3.0,
        );
        mm(
            ui,
            &tr!("support-upper-depth"),
            was(saved, |p| p.small_pillar.upper_depth_mm),
            &mut pillar.upper_depth_mm,
            FINE_STEP,
            0.05..=1.0,
        );
        mm(
            ui,
            &tr!("support-lower-depth"),
            was(saved, |p| p.small_pillar.lower_depth_mm),
            &mut pillar.lower_depth_mm,
            FINE_STEP,
            0.05..=1.0,
        );
        mm(
            ui,
            &tr!("support-max-length"),
            was(saved, |p| p.small_pillar.max_length_mm),
            &mut pillar.max_length_mm,
            COARSE_STEP,
            0.5..=50.0,
        );
    });
}

/// How nearby tips merge into shared trunks on the way down.
fn branching(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    let branching = &mut profile.branching;
    if !branching.enabled {
        describe(ui, &tr!("support-no-branching"));
        return;
    }
    nested(ui, |ui| {
        describe(ui, &tr!("support-branching-help"));
        let angle = &mut branching.max_angle_deg;
        degrees(
            ui,
            &tr!("support-branch-angle"),
            was(saved, |p| p.branching.max_angle_deg),
            angle,
            MIN_BRANCH_DEG..=MAX_BRANCH_DEG,
        );
        let reach = &mut branching.max_merge_distance_mm;
        number(
            ui,
            &tr!("support-merge-within"),
            was(saved, |p| p.branching.max_merge_distance_mm),
            reach,
            &tr!("unit-mm"),
            COARSE_STEP,
            1.0..=40.0,
            1,
        );
    });
}

/// The slab the supports stand on. It lifts the model by its own thickness on top of
/// the lift, so it is never printed into the part (ADR 0238).
fn raft(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    describe(ui, &tr!("support-raft-help"));
    let raft = &mut profile.raft;
    switch(ui, &mut raft.enabled, &tr!("support-raft"));
    if !raft.enabled {
        return;
    }
    nested(ui, |ui| {
        let width = ui.available_width();
        let styles = [
            Segment::new(RaftStyle::Skates, tr!("support-raft-skates")),
            Segment::new(RaftStyle::Grid, tr!("support-raft-grid")),
            Segment::new(RaftStyle::Connected, tr!("support-raft-connected")),
        ];
        Segmented::new(&styles)
            .width(width)
            .show(ui, &mut raft.style);
        if raft.style.has_outline() {
            ui.add_space(2.0);
            let shapes = [
                Segment::new(RaftShape::Hull, tr!("support-raft-hull")),
                Segment::new(RaftShape::Rectangle, tr!("support-raft-rectangle")),
            ];
            Segmented::new(&shapes)
                .width(width)
                .show(ui, &mut raft.shape);
        }
        ui.add_space(2.0);
        mm(
            ui,
            &tr!("support-margin"),
            was(saved, |p| p.raft.margin_mm),
            &mut raft.margin_mm,
            COARSE_STEP,
            0.0..=20.0,
        );
        mm(
            ui,
            &tr!("support-thickness"),
            was(saved, |p| p.raft.thickness_mm),
            &mut raft.thickness_mm,
            COARSE_STEP,
            0.2..=6.0,
        );
        degrees(
            ui,
            &tr!("support-wall-slope"),
            was(saved, |p| p.raft.slope_deg),
            &mut raft.slope_deg,
            0.0..=75.0,
        );
    });
}

/// The struts tying tall trunks together.
fn bracing(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    describe(ui, &tr!("support-bracing-help"));
    let bracing = &mut profile.bracing;
    switch(ui, &mut bracing.enabled, &tr!("support-bracing"));
    if !bracing.enabled {
        return;
    }
    nested(ui, |ui| {
        mm(
            ui,
            &tr!("support-diameter"),
            was(saved, |p| p.bracing.diameter_mm),
            &mut bracing.diameter_mm,
            FINE_STEP,
            0.1..=3.0,
        );
        let span = &mut bracing.max_spacing_mm;
        number(
            ui,
            &tr!("support-span"),
            was(saved, |p| p.bracing.max_spacing_mm),
            span,
            &tr!("unit-mm"),
            COARSE_STEP,
            1.0..=40.0,
            1,
        );
        let start = &mut bracing.start_height_mm;
        number(
            ui,
            &tr!("support-first-at"),
            was(saved, |p| p.bracing.start_height_mm),
            start,
            &tr!("unit-mm"),
            COARSE_STEP,
            0.0..=40.0,
            1,
        );
        let rise = &mut bracing.rise_mm;
        number(
            ui,
            &tr!("support-every"),
            was(saved, |p| p.bracing.rise_mm),
            rise,
            &tr!("unit-mm"),
            COARSE_STEP,
            1.0..=40.0,
            1,
        );
    });
}

/// What the automatic run holds up, how thickly, and how far the profile lifts a model.
fn automatic(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    held_up(ui, profile, saved);
    describe(ui, &tr!("support-lift-help"));
    mm(
        ui,
        &tr!("support-lift"),
        was(saved, |p| p.z_lift_mm),
        &mut profile.z_lift_mm,
        COARSE_STEP,
        0.0..=30.0,
    );
}

/// What the automatic run holds up, and how thickly: the automatic group beside a model,
/// whose lift is its own.
pub fn held_up(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    describe(ui, &tr!("support-held-up-help"));
    let overhang = &mut profile.max_overhang_deg;
    degrees(
        ui,
        &tr!("support-overhang"),
        was(saved, |p| p.max_overhang_deg),
        overhang,
        MIN_OVERHANG_DEG..=MAX_OVERHANG_DEG,
    );
    mm(
        ui,
        &tr!("support-contact-spacing"),
        was(saved, |p| p.contact_spacing_mm),
        &mut profile.contact_spacing_mm,
        COARSE_STEP,
        0.5..=20.0,
    );
    number(
        ui,
        &tr!("support-density"),
        was(saved, |p| p.density),
        &mut profile.density,
        "x",
        COARSE_STEP,
        MIN_DENSITY..=MAX_DENSITY,
        2,
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
        let mut edited = SupportProfile::medium();
        edited.bracing.diameter_mm += 0.3;

        let saved = saved_as(&catalogue, &edited).expect("Medium is shipped");
        assert_eq!(saved.profile, SupportProfile::medium());
        assert_eq!(
            was(Some(&saved.profile), |profile| profile.bracing.diameter_mm),
            Some(SupportProfile::medium().bracing.diameter_mm),
            "which is what the field's button puts back"
        );
    }

    #[test]
    fn a_profile_of_a_name_nobody_saved_has_nothing_to_go_back_to() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue loads");
        let mut renamed = SupportProfile::medium();
        renamed.name = "Nobody's".to_owned();
        assert!(saved_as(&catalogue, &renamed).is_none());
    }
}
