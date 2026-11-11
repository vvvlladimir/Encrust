use super::{
    COARSE_STEP, MAX_DENSITY, MAX_OVERHANG_DEG, MIN_DENSITY, MIN_OVERHANG_DEG, degrees, mm, number,
    was,
};
use crate::i18n::tr;
use crate::ui::describe;
use printer_profiles::{AUTO_SIZES, AutoProfile, SupportProfile};

/// What the automatic run holds up, how thickly, and how far the profile lifts a model.
pub(super) fn automatic(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
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
        &tr!("unit-times"),
        COARSE_STEP,
        MIN_DENSITY..=MAX_DENSITY,
        2,
    );
}

/// From which areas an island is held at a wider share of the contact spacing, against the
/// shipped profile; a wider share never starts below a narrower one. See ADR 0325.
pub fn islands(ui: &mut egui::Ui, auto: &mut AutoProfile, shipped: Option<&AutoProfile>) {
    describe(ui, &tr!("support-islands-help"));
    let island = Measure {
        label: |label| label,
        unit: tr!("unit-mm2"),
        step: COARSE_STEP,
        max: MAX_ISLAND_MM2,
        decimals: 1,
    };
    island.show(
        ui,
        &[tr!("support-island-wider"), tr!("support-island-full")],
        0.0,
        &mut auto.island_area_mm2,
        shipped.map(|auto| &auto.island_area_mm2[..]),
    );
}

/// Where a step grows a size by the resin it puts out, how far above a step of each size its
/// neighbourhood is held, and how far a face there may lean first; see `docs/decisions/0326`.
pub fn steps(ui: &mut egui::Ui, auto: &mut AutoProfile, shipped: Option<&AutoProfile>) {
    describe(ui, &tr!("support-steps-help"));
    let sizes: [String; AUTO_SIZES] = std::array::from_fn(|at| (at + 1).to_string());
    // The first is the least step held as one at all, which stays in the profile's file.
    let step = Measure {
        label: |size| tr!("support-step-size", size = size),
        unit: tr!("unit-mm3"),
        step: COARSE_STEP,
        max: MAX_STEP_MM3,
        decimals: 1,
    };
    let [least_step, larger_steps @ ..] = &mut auto.step_volume_mm3;
    step.show(
        ui,
        &sizes[1..],
        *least_step,
        larger_steps,
        shipped.map(|auto| &auto.step_volume_mm3[1..]),
    );
    let height = Measure {
        label: |size| tr!("support-step-reach", size = size),
        unit: tr!("unit-mm"),
        step: COARSE_STEP,
        max: MAX_STEP_HEIGHT_MM,
        decimals: 2,
    };
    height.show(
        ui,
        &sizes,
        0.0,
        &mut auto.step_height_mm,
        shipped.map(|auto| &auto.step_height_mm[..]),
    );
    degrees(
        ui,
        &tr!("support-step-lean"),
        shipped.map(|auto| auto.step_lean_deg),
        &mut auto.step_lean_deg,
        1.0..=auto.max_overhang_deg,
    );
    auto.step_lean_deg = auto.step_lean_deg.min(auto.max_overhang_deg);
}

/// The light supports that steady a part on a slope too gentle to sag.
pub fn steadying(ui: &mut egui::Ui, auto: &mut AutoProfile, shipped: Option<&AutoProfile>) {
    describe(ui, &tr!("support-steadying-help"));
    degrees(
        ui,
        &tr!("support-steady-lean"),
        shipped.map(|auto| auto.steady_lean_deg),
        &mut auto.steady_lean_deg,
        1.0..=auto.step_lean_deg,
    );
    auto.steady_lean_deg = auto.steady_lean_deg.min(auto.step_lean_deg);
    mm(
        ui,
        &tr!("support-steady-spacing"),
        shipped.map(|auto| auto.steady_spacing_mm),
        &mut auto.steady_spacing_mm,
        COARSE_STEP,
        0.5..=MAX_STEADY_SPACING_MM,
    );
    number(
        ui,
        &tr!("support-steady-area"),
        shipped.map(|auto| auto.steady_area_mm2),
        &mut auto.steady_area_mm2,
        &tr!("unit-mm2"),
        COARSE_STEP,
        0.0..=MAX_ISLAND_MM2,
        1,
    );
}

/// One measure an automatic profile steps by, shown as the least of it each step up takes.
pub(super) struct Measure {
    /// The row's label, from the name of its step.
    pub(super) label: fn(String) -> String,
    pub(super) unit: String,
    pub(super) step: f64,
    pub(super) max: f32,
    pub(super) decimals: usize,
}

impl Measure {
    /// A row per step of `steps`, each over `least` of it and none under `floor`, against
    /// `shipped`. A step never starts below the one before.
    pub(super) fn show(
        &self,
        ui: &mut egui::Ui,
        steps: &[String],
        floor: f32,
        least: &mut [f32],
        shipped: Option<&[f32]>,
    ) {
        for (at, name) in steps.iter().enumerate().take(least.len()) {
            let floor = if at == 0 { floor } else { least[at - 1] };
            number(
                ui,
                &(self.label)(name.clone()),
                shipped.and_then(|shipped| shipped.get(at).copied()),
                &mut least[at],
                &self.unit,
                self.step,
                floor..=self.max,
                self.decimals,
            );
        }
        let mut below = floor;
        for value in least.iter_mut() {
            *value = value.max(below);
            below = *value;
        }
    }
}

/// Bounds on where an automatic profile steps up: an island as large as a thumbnail and a
/// shelf as large as a plate.
pub(super) const MAX_ISLAND_MM2: f32 = 100.0;

pub(super) const MAX_STEP_MM3: f32 = 1_000.0;

/// How far above a step its neighbourhood may be held: past a model's own tip, nothing.
pub(super) const MAX_STEP_HEIGHT_MM: f32 = 20.0;

/// How far apart steadying supports may stand: a plate's width holds only a few.
const MAX_STEADY_SPACING_MM: f32 = 40.0;
