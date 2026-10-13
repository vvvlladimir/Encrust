//! The fields of one support profile, in the groups both the Supports tool and the
//! Settings window lay them out in.

use std::ops::RangeInclusive;

use printer_profiles::{
    Catalogue, ContactShape, Entry, PlatformShape, RaftShape, RaftStyle, SupportProfile,
};

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

    pub fn label(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Main => "Pillar",
            Self::Bottom => "Bottom",
            Self::SmallPillar => "Small pillar",
            Self::Branching => "Branching",
            Self::Raft => "Raft",
            Self::Bracing => "Cross bracing",
            Self::Automatic => "Automatic",
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
    number(ui, label, was, value, "mm", step, range, 2);
}

fn degrees(
    ui: &mut egui::Ui,
    label: &str,
    was: Option<f32>,
    value: &mut f32,
    range: RangeInclusive<f32>,
) {
    number(ui, label, was, value, "deg", ANGLE_STEP, range, 0);
}

/// The contact with the model and the cone that widens out of it.
fn top(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    let segments = [
        Segment::new(ContactShape::Plane, "Plane"),
        Segment::new(ContactShape::Sphere, "Sphere"),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut profile.tip.shape);
    ui.add_space(4.0);
    let tip = &mut profile.tip;
    mm(
        ui,
        "Contact",
        was(saved, |p| p.tip.contact_diameter_mm),
        &mut tip.contact_diameter_mm,
        FINE_STEP,
        0.05..=2.0,
    );
    mm(
        ui,
        "Bite",
        was(saved, |p| p.tip.contact_depth_mm),
        &mut tip.contact_depth_mm,
        FINE_STEP,
        0.05..=1.0,
    );
    let length = &mut profile.top.length_mm;
    mm(
        ui,
        "Length",
        was(saved, |p| p.top.length_mm),
        length,
        COARSE_STEP,
        0.2..=10.0,
    );
}

/// The column itself, the widest its trunk grows, and the air it keeps from the model.
fn main(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    mm(
        ui,
        "Diameter",
        was(saved, |p| p.middle.diameter_mm),
        &mut profile.middle.diameter_mm,
        COARSE_STEP,
        0.3..=6.0,
    );
    mm(
        ui,
        "Clearance",
        was(saved, |p| p.clearance_mm),
        &mut profile.clearance_mm,
        FINE_STEP,
        0.0..=3.0,
    );
    // Every trunk widens towards its base, branched or not (ADR 0242).
    mm(
        ui,
        "Trunk cap",
        was(saved, |p| p.branching.max_trunk_diameter_mm),
        &mut profile.branching.max_trunk_diameter_mm,
        COARSE_STEP,
        0.3..=10.0,
    );
}

/// The foot the column stands on.
fn bottom(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    let segments = [
        Segment::new(PlatformShape::Cylinder, "Round"),
        Segment::new(PlatformShape::Cone, "Tapered"),
        Segment::new(PlatformShape::Prism, "Prism"),
        Segment::new(PlatformShape::Cube, "Cube"),
        Segment::new(PlatformShape::Skate, "Skate"),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut profile.bottom.shape);
    ui.add_space(4.0);
    let bottom = &mut profile.bottom;
    mm(
        ui,
        "Diameter",
        was(saved, |p| p.bottom.platform_diameter_mm),
        &mut bottom.platform_diameter_mm,
        COARSE_STEP,
        1.0..=30.0,
    );
    mm(
        ui,
        "Thickness",
        was(saved, |p| p.bottom.platform_thickness_mm),
        &mut bottom.platform_thickness_mm,
        COARSE_STEP,
        0.2..=5.0,
    );
    mm(
        ui,
        "Flare",
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
    switch(ui, &mut profile.land_on_model, "Stand on the model");
    if !profile.land_on_model {
        describe(
            ui,
            "A support with the model under it leans aside for the plate, or is dropped. \
             Nothing is left cured onto the part.",
        );
        return;
    }
    let pillar = &mut profile.small_pillar;
    switch(ui, &mut pillar.enabled, "Thin these down");
    if !pillar.enabled {
        describe(
            ui,
            "A support that lands on the model keeps the regular pillar's width.",
        );
        return;
    }
    nested(ui, |ui| {
        describe(
            ui,
            "Used where a support runs from one part of the model to another. Both ends sink \
             into a surface; a longer run keeps the regular width.",
        );
        mm(
            ui,
            "Diameter",
            was(saved, |p| p.small_pillar.diameter_mm),
            &mut pillar.diameter_mm,
            FINE_STEP,
            0.1..=3.0,
        );
        mm(
            ui,
            "Upper depth",
            was(saved, |p| p.small_pillar.upper_depth_mm),
            &mut pillar.upper_depth_mm,
            FINE_STEP,
            0.05..=1.0,
        );
        mm(
            ui,
            "Lower depth",
            was(saved, |p| p.small_pillar.lower_depth_mm),
            &mut pillar.lower_depth_mm,
            FINE_STEP,
            0.05..=1.0,
        );
        mm(
            ui,
            "Max length",
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
        describe(ui, "Every tip stands on a column of its own.");
        return;
    }
    nested(ui, |ui| {
        let angle = &mut branching.max_angle_deg;
        degrees(
            ui,
            "Branch angle",
            was(saved, |p| p.branching.max_angle_deg),
            angle,
            MIN_BRANCH_DEG..=MAX_BRANCH_DEG,
        );
        let reach = &mut branching.max_merge_distance_mm;
        number(
            ui,
            "Merge within",
            was(saved, |p| p.branching.max_merge_distance_mm),
            reach,
            "mm",
            COARSE_STEP,
            1.0..=40.0,
            1,
        );
    });
}

/// The slab the supports stand on. It lifts the model by its own thickness on top of
/// the lift, so it is never printed into the part (ADR 0238).
fn raft(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    let raft = &mut profile.raft;
    switch(ui, &mut raft.enabled, "Raft under the feet");
    if !raft.enabled {
        return;
    }
    nested(ui, |ui| {
        let width = ui.available_width();
        let styles = [
            Segment::new(RaftStyle::Skates, "Skates"),
            Segment::new(RaftStyle::Grid, "Grid"),
            Segment::new(RaftStyle::Connected, "Connected"),
        ];
        Segmented::new(&styles)
            .width(width)
            .show(ui, &mut raft.style);
        if raft.style.has_outline() {
            ui.add_space(2.0);
            let shapes = [
                Segment::new(RaftShape::Hull, "Hull"),
                Segment::new(RaftShape::Rectangle, "Rectangle"),
            ];
            Segmented::new(&shapes)
                .width(width)
                .show(ui, &mut raft.shape);
        }
        ui.add_space(2.0);
        mm(
            ui,
            "Margin",
            was(saved, |p| p.raft.margin_mm),
            &mut raft.margin_mm,
            COARSE_STEP,
            0.0..=20.0,
        );
        mm(
            ui,
            "Thickness",
            was(saved, |p| p.raft.thickness_mm),
            &mut raft.thickness_mm,
            COARSE_STEP,
            0.2..=6.0,
        );
        degrees(
            ui,
            "Wall slope",
            was(saved, |p| p.raft.slope_deg),
            &mut raft.slope_deg,
            0.0..=75.0,
        );
    });
}

/// The struts tying tall trunks together.
fn bracing(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    let bracing = &mut profile.bracing;
    switch(ui, &mut bracing.enabled, "Brace tall trunks");
    if !bracing.enabled {
        return;
    }
    nested(ui, |ui| {
        mm(
            ui,
            "Diameter",
            was(saved, |p| p.bracing.diameter_mm),
            &mut bracing.diameter_mm,
            FINE_STEP,
            0.1..=3.0,
        );
        let span = &mut bracing.max_spacing_mm;
        number(
            ui,
            "Span",
            was(saved, |p| p.bracing.max_spacing_mm),
            span,
            "mm",
            COARSE_STEP,
            1.0..=40.0,
            1,
        );
        let start = &mut bracing.start_height_mm;
        number(
            ui,
            "First at",
            was(saved, |p| p.bracing.start_height_mm),
            start,
            "mm",
            COARSE_STEP,
            0.0..=40.0,
            1,
        );
        let rise = &mut bracing.rise_mm;
        number(
            ui,
            "Every",
            was(saved, |p| p.bracing.rise_mm),
            rise,
            "mm",
            COARSE_STEP,
            1.0..=40.0,
            1,
        );
    });
}

/// What the automatic run holds up, and how thickly.
fn automatic(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    let overhang = &mut profile.max_overhang_deg;
    degrees(
        ui,
        "Overhang",
        was(saved, |p| p.max_overhang_deg),
        overhang,
        MIN_OVERHANG_DEG..=MAX_OVERHANG_DEG,
    );
    mm(
        ui,
        "Contact spacing",
        was(saved, |p| p.contact_spacing_mm),
        &mut profile.contact_spacing_mm,
        COARSE_STEP,
        0.5..=20.0,
    );
    number(
        ui,
        "Density",
        was(saved, |p| p.density),
        &mut profile.density,
        "x",
        COARSE_STEP,
        MIN_DENSITY..=MAX_DENSITY,
        2,
    );
    mm(
        ui,
        "Lift",
        was(saved, |p| p.z_lift_mm),
        &mut profile.z_lift_mm,
        COARSE_STEP,
        0.0..=30.0,
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
