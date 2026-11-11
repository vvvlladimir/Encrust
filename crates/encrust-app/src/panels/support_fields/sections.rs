use super::{COARSE_STEP, FINE_STEP, MAX_BRANCH_DEG, MIN_BRANCH_DEG, degrees, mm, number, was};
use crate::i18n::tr;
use crate::ui::theme;
use crate::ui::{Segment, Segmented, describe, nested, switch};
use printer_profiles::{
    Branching, ContactShape, Grid, PlatformShape, Raft, RaftShape, RaftStyle, SupportProfile,
};

/// The contact with the model and the cone that widens out of it.
pub(super) fn top(ui: &mut egui::Ui, profile: &mut SupportProfile, saved: Option<&SupportProfile>) {
    describe(ui, &tr!("support-tip-help"));
    let segments = [
        Segment::new(ContactShape::Plane, tr!("support-shape-plane")),
        Segment::new(ContactShape::Sphere, tr!("support-shape-sphere")),
    ];
    let width = ui.available_width();
    Segmented::new(&segments)
        .width(width)
        .show(ui, &mut profile.tip.shape);
    ui.add_space(theme::NOTE_GAP);
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

/// The thinnest a strut or trunk is built, the widest it grows, and the air it keeps from the
/// model; between the two, what it carries and how tall it stands decide (ADR 0424).
pub(super) fn main(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
    describe(ui, &tr!("support-pillar-help"));
    mm(
        ui,
        &tr!("support-least-diameter"),
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
pub(super) fn bottom(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
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
    ui.add_space(theme::NOTE_GAP);
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

/// Whether a support may stand on the part at all, and the thin strut between two parts of
/// the model, which needs neither a foot nor the width of a plate column.
pub(super) fn small_pillar(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
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
pub(super) fn branching(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
    let branching = &mut profile.branching;
    if !branching.enabled {
        describe(ui, &tr!("support-no-branching"));
        return;
    }
    nested(ui, |ui| {
        describe(ui, &tr!("support-merging-help"));
        branching_fields(ui, branching, saved);
    });
}

/// The numbers of a branching that is on.
pub(super) fn branching_fields(
    ui: &mut egui::Ui,
    branching: &mut Branching,
    saved: Option<&SupportProfile>,
) {
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
}

/// The square grid trunks stand on, in place of where the tips merge; see
/// `docs/decisions/0333`.
pub(super) fn grid(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
    let grid = &mut profile.grid;
    if !grid.enabled {
        describe(ui, &tr!("support-no-grid"));
        return;
    }
    nested(ui, |ui| {
        describe(ui, &tr!("support-grid-help"));
        grid_fields(ui, grid, saved);
    });
}

/// The numbers of a grid that is on.
pub(super) fn grid_fields(ui: &mut egui::Ui, grid: &mut Grid, saved: Option<&SupportProfile>) {
    number(
        ui,
        &tr!("support-grid-spacing"),
        was(saved, |p| p.grid.spacing_mm),
        &mut grid.spacing_mm,
        &tr!("unit-mm"),
        COARSE_STEP,
        Grid::SPACING_MM,
        1,
    );
}

/// The slab the supports stand on. It lifts the model by its own thickness on top of
/// the lift, so it is never printed into the part (ADR 0238).
pub(super) fn raft(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
    describe(ui, &tr!("support-raft-help"));
    let raft = &mut profile.raft;
    switch(ui, &mut raft.enabled, &tr!("support-raft"));
    if raft.enabled {
        nested(ui, |ui| raft_fields(ui, raft, saved));
    }
}

/// The style, outline and numbers of a raft that is on.
pub(super) fn raft_fields(ui: &mut egui::Ui, raft: &mut Raft, saved: Option<&SupportProfile>) {
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
        ui.add_space(theme::ICON_GAP);
        let shapes = [
            Segment::new(RaftShape::Hull, tr!("support-raft-hull")),
            Segment::new(RaftShape::Rectangle, tr!("support-raft-rectangle")),
        ];
        Segmented::new(&shapes)
            .width(width)
            .show(ui, &mut raft.shape);
    }
    ui.add_space(theme::ICON_GAP);
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
}
