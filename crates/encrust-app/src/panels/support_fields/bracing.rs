use super::{COARSE_STEP, FINE_STEP, mm, number, was};
use crate::i18n::tr;
use crate::ui::{
    Segment, Segmented, bare_switch, count_row, describe, field_label, icon, icon_toggle, kept,
    nested, switch, theme,
};
use printer_profiles::{BracePattern, Bracing, BracingPreset, SupportProfile};

/// The most levels a bracing zone may take: past that the middle has nothing left to fill.
pub(super) const MAX_BRACE_LEVELS: u32 = 6;

/// The struts tying tall trunks together: a preset or every number once fine-tuning is asked
/// for. The window decides which shows; a non-preset profile opens on the numbers.
pub(super) fn bracing(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
    describe(ui, &tr!("support-bracing-help"));
    let (id, mut fine) = fine_tune(ui, profile);
    let bracing = &mut profile.bracing;
    bracing_switch(ui, &mut bracing.enabled, &mut fine);
    kept::set(ui.ctx(), id, &fine);
    if !bracing.enabled {
        return;
    }
    nested(ui, |ui| {
        if fine {
            bracing_numbers(ui, bracing, saved);
        } else {
            bracing_presets(ui, bracing);
        }
    });
}

/// A bracing that is on, under a switch its heading carries: the presets, the switch to
/// every number, then the buttress or every number.
pub(super) fn bracing_fields(
    ui: &mut egui::Ui,
    profile: &mut SupportProfile,
    saved: Option<&SupportProfile>,
) {
    let (id, mut fine) = fine_tune(ui, profile);
    let bracing = &mut profile.bracing;
    if !fine {
        bracing_presets(ui, bracing);
    }
    switch(ui, &mut fine, &tr!("support-bracing-fine-tune"));
    kept::set(ui.ctx(), id, &fine);
    if fine {
        bracing_numbers(ui, bracing, saved);
    } else {
        buttress(ui, bracing, saved);
    }
}

/// Where the choice of every number over the presets is kept, and that choice: on for a
/// profile whose numbers are no preset's.
fn fine_tune(ui: &egui::Ui, profile: &SupportProfile) -> (egui::Id, bool) {
    let id = ui.id().with(("bracing-fine-tune", &profile.name));
    let fine =
        kept::get(ui.ctx(), id).unwrap_or_else(|| BracingPreset::of(&profile.bracing).is_none());
    (id, fine)
}

/// The row that turns bracing on, and before its switch the one that trades the presets
/// for every number and back.
pub(super) fn bracing_switch(ui: &mut egui::Ui, on: &mut bool, fine: &mut bool) {
    let label = tr!("support-bracing");
    ui.horizontal(|ui| {
        ui.set_min_height(theme::ROW_H);
        field_label(ui, &label, theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            bare_switch(ui, on, &label);
            let tip = tr!("support-bracing-fine-tune");
            if *on && icon_toggle(ui, icon::PARAMETERS, &tip, *fine).clicked() {
                *fine = !*fine;
            }
        });
    });
}

/// The ready presets, the one whose numbers the profile holds picked. Picking one writes
/// its numbers over the profile's.
pub(super) fn bracing_presets(ui: &mut egui::Ui, bracing: &mut Bracing) {
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

pub(super) fn preset_label(preset: BracingPreset) -> String {
    match preset {
        BracingPreset::Standard => tr!("support-bracing-standard"),
        BracingPreset::Strong => tr!("support-bracing-strong"),
        BracingPreset::Tall => tr!("support-bracing-tall"),
        BracingPreset::Small => tr!("support-bracing-small"),
    }
}

pub(super) fn preset_help(preset: BracingPreset) -> String {
    match preset {
        BracingPreset::Standard => tr!("support-bracing-standard-help"),
        BracingPreset::Strong => tr!("support-bracing-strong-help"),
        BracingPreset::Tall => tr!("support-bracing-tall-help"),
        BracingPreset::Small => tr!("support-bracing-small-help"),
    }
}

/// Every number of the bracing, and how each zone up a pair is tied.
pub(super) fn bracing_numbers(
    ui: &mut egui::Ui,
    bracing: &mut Bracing,
    saved: Option<&SupportProfile>,
) {
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
    ui.add_space(2.0);
    buttress(ui, bracing, saved);
}

/// The legs propping a tall trunk the bracing leaves free at the top.
pub(super) fn buttress(ui: &mut egui::Ui, bracing: &mut Bracing, saved: Option<&SupportProfile>) {
    let buttress = &mut bracing.buttress;
    switch(ui, &mut buttress.enabled, &tr!("support-buttress"));
    if !buttress.enabled {
        return;
    }
    mm(
        ui,
        &tr!("support-buttress-pair-above"),
        was(saved, |p| p.bracing.buttress.pair_above_mm),
        &mut buttress.pair_above_mm,
        COARSE_STEP,
        1.0..=200.0,
    );
    mm(
        ui,
        &tr!("support-buttress-one-above"),
        was(saved, |p| p.bracing.buttress.one_above_mm),
        &mut buttress.one_above_mm,
        COARSE_STEP,
        1.0..=200.0,
    );
}

/// How one zone ties a pair: a zigzag or a cross.
pub(super) fn pattern_row(ui: &mut egui::Ui, label: &str, pattern: &mut BracePattern) {
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

pub(super) fn levels_row(ui: &mut egui::Ui, label: &str, levels: &mut u32) {
    count_row(ui, label, levels, "", 0.05, 0..=MAX_BRACE_LEVELS);
}
