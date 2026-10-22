//! The fields of one support profile, in the groups both the Supports tool and the
//! Settings window lay them out in.

use std::ops::RangeInclusive;

use printer_profiles::{
    AutoProfile, BracePattern, Bracing, BracingPreset, Catalogue, ContactShape, Entry,
    PlatformShape, RaftShape, RaftStyle, SupportProfile,
};

use crate::i18n::tr;
use crate::ui::{
    Carried, Segment, Segmented, carried_row, count_row, describe, field_label, icon, nested,
    quiet_button, switch, theme,
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
        0.0..=1.0,
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
            0.0..=1.0,
        );
        mm(
            ui,
            &tr!("support-lower-depth"),
            was(saved, |p| p.small_pillar.lower_depth_mm),
            &mut pillar.lower_depth_mm,
            FINE_STEP,
            0.0..=1.0,
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

/// The most levels a bracing zone may take: past that the middle has nothing left to fill.
const MAX_BRACE_LEVELS: u32 = 6;

/// The struts tying tall trunks together: a ready preset, or every number of it once the
/// user asks to fine-tune. Which of the two shows is the window's, not the profile's; a
/// profile whose numbers are no preset opens on the numbers.
fn bracing(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    describe(ui, &tr!("support-bracing-help"));
    let id = ui.id().with(("bracing-fine-tune", &profile.name));
    let bracing = &mut profile.bracing;
    switch(ui, &mut bracing.enabled, &tr!("support-bracing"));
    if !bracing.enabled {
        return;
    }
    let mut fine = ui
        .data(|data| data.get_temp(id))
        .unwrap_or_else(|| BracingPreset::of(bracing).is_none());
    nested(ui, |ui| {
        if fine {
            bracing_numbers(ui, bracing, saved);
            fine = !quiet_button(ui, icon::BACK, &tr!("support-bracing-presets"), false, true)
                .clicked();
        } else {
            bracing_presets(ui, bracing);
            fine = quiet_button(
                ui,
                icon::PARAMETERS,
                &tr!("support-bracing-fine-tune"),
                false,
                true,
            )
            .clicked();
        }
    });
    ui.data_mut(|data| data.insert_temp(id, fine));
}

/// The ready presets, the one whose numbers the profile holds picked. Picking one writes
/// its numbers over the profile's.
fn bracing_presets(ui: &mut egui::Ui, bracing: &mut Bracing) {
    let presets = BracingPreset::ALL.map(|preset| Segment::new(Some(preset), preset_label(preset)));
    let mut picked = BracingPreset::of(bracing);
    let changed = Segmented::new(&presets)
        .width(ui.available_width())
        .show(ui, &mut picked);
    if changed && let Some(preset) = picked {
        *bracing = preset.bracing();
    }
    if let Some(preset) = picked {
        describe(ui, &preset_help(preset));
    } else {
        describe(ui, &tr!("support-bracing-preset-none"));
    }
}

fn preset_label(preset: BracingPreset) -> String {
    match preset {
        BracingPreset::Standard => tr!("support-bracing-standard"),
        BracingPreset::Strong => tr!("support-bracing-strong"),
        BracingPreset::Tall => tr!("support-bracing-tall"),
        BracingPreset::Small => tr!("support-bracing-small"),
    }
}

fn preset_help(preset: BracingPreset) -> String {
    match preset {
        BracingPreset::Standard => tr!("support-bracing-standard-help"),
        BracingPreset::Strong => tr!("support-bracing-strong-help"),
        BracingPreset::Tall => tr!("support-bracing-tall-help"),
        BracingPreset::Small => tr!("support-bracing-small-help"),
    }
}

/// Every number of the bracing, and how each zone up a pair is tied.
fn bracing_numbers(ui: &mut egui::Ui, bracing: &mut Bracing, saved: Option<&SupportProfile>) {
    mm(
        ui,
        &tr!("support-diameter"),
        was(saved, |p| p.bracing.diameter_mm),
        &mut bracing.diameter_mm,
        FINE_STEP,
        0.1..=3.0,
    );
    let mm_unit = tr!("unit-mm");
    number(
        ui,
        &tr!("support-span"),
        was(saved, |p| p.bracing.max_spacing_mm),
        &mut bracing.max_spacing_mm,
        &mm_unit,
        COARSE_STEP,
        1.0..=40.0,
        1,
    );
    number(
        ui,
        &tr!("support-first-at"),
        was(saved, |p| p.bracing.start_height_mm),
        &mut bracing.start_height_mm,
        &mm_unit,
        COARSE_STEP,
        0.0..=40.0,
        1,
    );
    number(
        ui,
        &tr!("support-every"),
        was(saved, |p| p.bracing.rise_mm),
        &mut bracing.rise_mm,
        &mm_unit,
        COARSE_STEP,
        1.0..=40.0,
        1,
    );
    ui.add_space(2.0);
    pattern_row(ui, &tr!("support-brace-top"), &mut bracing.top.pattern);
    levels_row(
        ui,
        &tr!("support-brace-top-levels"),
        &mut bracing.top.levels,
    );
    pattern_row(ui, &tr!("support-brace-middle"), &mut bracing.middle);
    pattern_row(
        ui,
        &tr!("support-brace-bottom"),
        &mut bracing.bottom.pattern,
    );
    levels_row(
        ui,
        &tr!("support-brace-bottom-levels"),
        &mut bracing.bottom.levels,
    );
}

/// How one zone ties a pair: a zigzag or a cross.
fn pattern_row(ui: &mut egui::Ui, label: &str, pattern: &mut BracePattern) {
    let patterns = [
        Segment::new(BracePattern::Zigzag, tr!("support-brace-zigzag")),
        Segment::new(BracePattern::Cross, tr!("support-brace-cross")),
    ];
    ui.horizontal(|ui| {
        field_label(ui, label, theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            Segmented::new(&patterns)
                .width(theme::FIELD_W)
                .show(ui, pattern);
        });
    });
}

fn levels_row(ui: &mut egui::Ui, label: &str, levels: &mut u32) {
    count_row(ui, label, levels, "", 0.05, 0..=MAX_BRACE_LEVELS);
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
    mm(
        ui,
        &tr!("support-steep-contact-spacing"),
        was(saved, |p| p.steep_contact_spacing_mm),
        &mut profile.steep_contact_spacing_mm,
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

/// Where an automatic profile moves from one of its shapes to the next, named by `shapes`
/// lightest first, against the profile it ships as. A heavier shape never starts below a
/// lighter one. See `docs/decisions/0325`.
pub fn sizes(
    ui: &mut egui::Ui,
    auto: &mut AutoProfile,
    shipped: Option<&AutoProfile>,
    shapes: &[String; 3],
) {
    let heavier = &shapes[1..];
    let island = Measure {
        label: |shape| tr!("support-size-island", shape = shape),
        unit: tr!("unit-mm2"),
        step: COARSE_STEP,
        max: MAX_ISLAND_MM2,
        decimals: 1,
    };
    island.show(
        ui,
        heavier,
        &mut auto.island_area_mm2,
        shipped.map(|auto| &auto.island_area_mm2[..]),
    );
    let load = Measure {
        label: |shape| tr!("support-size-load", shape = shape),
        unit: tr!("unit-mm3"),
        step: LOAD_STEP,
        max: MAX_LOAD_MM3,
        decimals: 0,
    };
    load.show(
        ui,
        heavier,
        &mut auto.overhang_volume_mm3,
        shipped.map(|auto| &auto.overhang_volume_mm3[..]),
    );
    // The first is the least step held as one at all, which stays in the profile's file.
    let step = Measure {
        label: |shape| tr!("support-size-step", shape = shape),
        unit: tr!("unit-mm3"),
        step: COARSE_STEP,
        max: MAX_STEP_MM3,
        decimals: 1,
    };
    step.show(
        ui,
        heavier,
        &mut auto.step_volume_mm3[1..],
        shipped.map(|auto| &auto.step_volume_mm3[1..]),
    );
}

/// One measure an automatic profile sizes by, shown as the least of it each heavier shape
/// takes.
struct Measure {
    /// The row's label, naming the shape.
    label: fn(String) -> String,
    unit: String,
    step: f64,
    max: f32,
    decimals: usize,
}

impl Measure {
    /// A row per shape of `shapes`, each over `least` of it, against `shipped`. A heavier
    /// shape never starts below a lighter one.
    fn show(
        &self,
        ui: &mut egui::Ui,
        shapes: &[String],
        least: &mut [f32],
        shipped: Option<&[f32]>,
    ) {
        for (at, shape) in shapes.iter().enumerate().take(least.len()) {
            let floor = if at == 0 { 0.0 } else { least[at - 1] };
            number(
                ui,
                &(self.label)(shape.clone()),
                shipped.and_then(|shipped| shipped.get(at).copied()),
                &mut least[at],
                &self.unit,
                self.step,
                floor..=self.max,
                self.decimals,
            );
        }
        for at in 1..least.len() {
            least[at] = least[at].max(least[at - 1]);
        }
    }
}

/// Bounds and step on where an automatic profile moves to a heavier shape: an island as
/// large as a thumbnail, a part as heavy as a bust, and a shelf as large as a plate.
const MAX_ISLAND_MM2: f32 = 100.0;
const MAX_LOAD_MM3: f32 = 100_000.0;
const MAX_STEP_MM3: f32 = 1_000.0;
const LOAD_STEP: f64 = 10.0;

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
