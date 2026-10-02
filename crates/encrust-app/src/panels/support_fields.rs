//! The fields of one support profile, in the groups both the Supports tool and the
//! Settings screen lay them out in.

use std::ops::RangeInclusive;

use printer_profiles::{ContactShape, PlatformShape, RaftShape, SupportProfile};

use crate::ui::{Segment, Segmented, describe, nested, number_row, switch};

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

    /// Draws the group's fields, then puts the widths back in order.
    pub fn show(self, ui: &mut egui::Ui, profile: &mut SupportProfile) {
        match self {
            Self::Top => top(ui, profile),
            Self::Main => main(ui, profile),
            Self::Bottom => bottom(ui, profile),
            Self::SmallPillar => small_pillar(ui, profile),
            Self::Branching => branching(ui, profile),
            Self::Raft => raft(ui, profile),
            Self::Bracing => bracing(ui, profile),
            Self::Automatic => automatic(ui, profile),
        }
        keep_in_order(profile);
    }
}

fn mm(ui: &mut egui::Ui, label: &str, value: &mut f32, step: f64, range: RangeInclusive<f32>) {
    number_row(ui, label, value, "mm", step, range, 2);
}

fn degrees(ui: &mut egui::Ui, label: &str, value: &mut f32, range: RangeInclusive<f32>) {
    number_row(ui, label, value, "deg", ANGLE_STEP, range, 0);
}

/// The contact with the model and the cone that widens out of it.
fn top(ui: &mut egui::Ui, profile: &mut SupportProfile) {
    let segments = [
        Segment::new(ContactShape::Cone, "Cone"),
        Segment::new(ContactShape::Sphere, "Sphere"),
        Segment::new(ContactShape::Plane, "Plane"),
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
        &mut tip.contact_diameter_mm,
        FINE_STEP,
        0.05..=2.0,
    );
    mm(ui, "Bite", &mut tip.contact_depth_mm, FINE_STEP, 0.05..=1.0);
    let top = &mut profile.top;
    mm(
        ui,
        "Upper",
        &mut top.upper_diameter_mm,
        FINE_STEP,
        0.05..=4.0,
    );
    mm(
        ui,
        "Lower",
        &mut top.lower_diameter_mm,
        COARSE_STEP,
        0.05..=6.0,
    );
    mm(ui, "Length", &mut top.length_mm, COARSE_STEP, 0.2..=10.0);
}

/// The column itself, and the air it keeps between itself and the model.
fn main(ui: &mut egui::Ui, profile: &mut SupportProfile) {
    mm(
        ui,
        "Diameter",
        &mut profile.middle.diameter_mm,
        COARSE_STEP,
        0.3..=6.0,
    );
    mm(
        ui,
        "Clearance",
        &mut profile.clearance_mm,
        FINE_STEP,
        0.0..=3.0,
    );
}

/// The foot the column stands on.
fn bottom(ui: &mut egui::Ui, profile: &mut SupportProfile) {
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
        &mut bottom.platform_diameter_mm,
        COARSE_STEP,
        1.0..=30.0,
    );
    mm(
        ui,
        "Thickness",
        &mut bottom.platform_thickness_mm,
        COARSE_STEP,
        0.2..=5.0,
    );
    mm(
        ui,
        "Flare upper",
        &mut bottom.upper_diameter_mm,
        COARSE_STEP,
        0.1..=10.0,
    );
    mm(
        ui,
        "Flare lower",
        &mut bottom.lower_diameter_mm,
        COARSE_STEP,
        0.1..=15.0,
    );
}

/// Whether a support may stand on the part at all, and the thin strut between two parts
/// of the model, which needs neither a foot nor the width a column standing on the plate
/// needs.
fn small_pillar(ui: &mut egui::Ui, profile: &mut SupportProfile) {
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
             into a surface.",
        );
        mm(
            ui,
            "Diameter",
            &mut pillar.diameter_mm,
            FINE_STEP,
            0.1..=3.0,
        );
        mm(
            ui,
            "Upper depth",
            &mut pillar.upper_depth_mm,
            FINE_STEP,
            0.05..=1.0,
        );
        mm(
            ui,
            "Lower depth",
            &mut pillar.lower_depth_mm,
            FINE_STEP,
            0.05..=1.0,
        );
    });
}

/// How nearby tips merge into shared trunks on the way down.
fn branching(ui: &mut egui::Ui, profile: &mut SupportProfile) {
    let branching = &mut profile.branching;
    switch(ui, &mut branching.enabled, "Branching");
    if !branching.enabled {
        describe(ui, "Every tip stands on a column of its own.");
        return;
    }
    nested(ui, |ui| {
        let angle = &mut branching.max_angle_deg;
        degrees(ui, "Branch angle", angle, MIN_BRANCH_DEG..=MAX_BRANCH_DEG);
        let reach = &mut branching.max_merge_distance_mm;
        number_row(ui, "Merge within", reach, "mm", COARSE_STEP, 1.0..=40.0, 1);
        let trunk = &mut branching.max_trunk_diameter_mm;
        mm(ui, "Trunk cap", trunk, COARSE_STEP, 0.3..=10.0);
    });
}

/// The slab the supports stand on, and how far the model stands off the plate.
fn raft(ui: &mut egui::Ui, profile: &mut SupportProfile) {
    mm(
        ui,
        "Model lift",
        &mut profile.z_lift_mm,
        COARSE_STEP,
        0.0..=30.0,
    );
    let raft = &mut profile.raft;
    switch(ui, &mut raft.enabled, "Raft under the feet");
    if !raft.enabled {
        return;
    }
    nested(ui, |ui| {
        let segments = [
            Segment::new(RaftShape::Hull, "Hull"),
            Segment::new(RaftShape::Rectangle, "Rectangle"),
        ];
        let width = ui.available_width();
        Segmented::new(&segments)
            .width(width)
            .show(ui, &mut raft.shape);
        ui.add_space(2.0);
        number_row(
            ui,
            "Area",
            &mut raft.area_ratio,
            "x",
            COARSE_STEP,
            1.0..=4.0,
            2,
        );
        mm(
            ui,
            "Thickness",
            &mut raft.thickness_mm,
            COARSE_STEP,
            0.2..=6.0,
        );
        degrees(ui, "Wall slope", &mut raft.slope_deg, 0.0..=75.0);
    });
}

/// The struts tying tall trunks together.
fn bracing(ui: &mut egui::Ui, profile: &mut SupportProfile) {
    let bracing = &mut profile.bracing;
    switch(ui, &mut bracing.enabled, "Brace tall trunks");
    if !bracing.enabled {
        return;
    }
    nested(ui, |ui| {
        mm(
            ui,
            "Diameter",
            &mut bracing.diameter_mm,
            FINE_STEP,
            0.1..=3.0,
        );
        let span = &mut bracing.max_spacing_mm;
        number_row(ui, "Span", span, "mm", COARSE_STEP, 1.0..=40.0, 1);
        let start = &mut bracing.start_height_mm;
        number_row(ui, "First at", start, "mm", COARSE_STEP, 0.0..=40.0, 1);
        let rise = &mut bracing.rise_mm;
        number_row(ui, "Every", rise, "mm", COARSE_STEP, 1.0..=40.0, 1);
    });
}

/// What the automatic run holds up, and how thickly.
fn automatic(ui: &mut egui::Ui, profile: &mut SupportProfile) {
    let overhang = &mut profile.max_overhang_deg;
    degrees(
        ui,
        "Overhang",
        overhang,
        MIN_OVERHANG_DEG..=MAX_OVERHANG_DEG,
    );
    mm(
        ui,
        "Contact spacing",
        &mut profile.contact_spacing_mm,
        COARSE_STEP,
        0.5..=20.0,
    );
    number_row(
        ui,
        "Density",
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
    // The contact is not in the chain: a head wider than the neck under it is a nail, not
    // an inversion. See `docs/decisions/0131`.
    profile.top.lower_diameter_mm = profile
        .top
        .lower_diameter_mm
        .max(profile.top.upper_diameter_mm);
    profile.middle.diameter_mm = profile
        .middle
        .diameter_mm
        .max(profile.top.lower_diameter_mm);
    profile.branching.max_trunk_diameter_mm = profile
        .branching
        .max_trunk_diameter_mm
        .max(profile.middle.diameter_mm);
    profile.bottom.lower_diameter_mm = profile
        .bottom
        .lower_diameter_mm
        .max(profile.bottom.upper_diameter_mm);
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
