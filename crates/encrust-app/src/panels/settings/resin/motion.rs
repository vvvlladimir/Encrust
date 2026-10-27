use printer_profiles::{MaterialProfile, PrinterTuning, SecondStage};

use super::COARSE_STEP;
use crate::i18n::tr;
use crate::ui::{number_row, subheading};

/// Motion: how the plate peels the layer off the film and comes back. What the printer
/// locks is not offered.
pub(super) fn rows(ui: &mut egui::Ui, values: &mut MaterialProfile, locked: &PrinterTuning) {
    let rows = [
        (
            tr!("print-lift"),
            locked.lift_distance_mm,
            &mut values.lift_distance_mm,
            Travel::Distance,
        ),
        (
            tr!("print-lift-speed"),
            locked.lift_speed_mm_min,
            &mut values.lift_speed_mm_min,
            Travel::Speed,
        ),
        (
            tr!("resin-retract"),
            locked.retract_distance_mm,
            &mut values.retract_distance_mm,
            Travel::Distance,
        ),
        (
            tr!("print-retract-speed"),
            locked.retract_speed_mm_min,
            &mut values.retract_speed_mm_min,
            Travel::Speed,
        ),
    ];
    travel_rows(ui, rows);
    ui.add_space(8.0);
    let bottom = [
        (
            tr!("resin-bottom-lift"),
            locked.bottom_lift_distance_mm,
            &mut values.bottom_lift_distance_mm,
            Travel::Distance,
        ),
        (
            tr!("resin-bottom-lift-speed"),
            locked.bottom_lift_speed_mm_min,
            &mut values.bottom_lift_speed_mm_min,
            Travel::Speed,
        ),
        (
            tr!("resin-bottom-retract-speed"),
            locked.bottom_retract_speed_mm_min,
            &mut values.bottom_retract_speed_mm_min,
            Travel::Speed,
        ),
    ];
    travel_rows(ui, bottom);
    if locked.second_stage.is_none() {
        ui.add_space(8.0);
        second_stage(ui, &mut values.second_stage);
    }
}

/// The second stage of each peel: on up past the first lift, and slower for the last
/// stretch back down. Zero distances leave one stage.
fn second_stage(ui: &mut egui::Ui, stage: &mut SecondStage) {
    subheading(ui, &tr!("resin-second-stage"));
    let rows = [
        (
            tr!("print-lift"),
            None,
            &mut stage.lift_distance_mm,
            Travel::Stage,
        ),
        (
            tr!("print-lift-speed"),
            None,
            &mut stage.lift_speed_mm_min,
            Travel::Speed,
        ),
        (
            tr!("resin-retract"),
            None,
            &mut stage.retract_distance_mm,
            Travel::Stage,
        ),
        (
            tr!("print-retract-speed"),
            None,
            &mut stage.retract_speed_mm_min,
            Travel::Speed,
        ),
        (
            tr!("resin-bottom-lift"),
            None,
            &mut stage.bottom_lift_distance_mm,
            Travel::Stage,
        ),
        (
            tr!("resin-bottom-lift-speed"),
            None,
            &mut stage.bottom_lift_speed_mm_min,
            Travel::Speed,
        ),
        (
            tr!("resin-bottom-retract"),
            None,
            &mut stage.bottom_retract_distance_mm,
            Travel::Stage,
        ),
        (
            tr!("resin-bottom-retract-speed"),
            None,
            &mut stage.bottom_retract_speed_mm_min,
            Travel::Speed,
        ),
    ];
    travel_rows(ui, rows);
}

/// Whether a lift or retract is a distance or a speed, which sets its unit and its range.
#[derive(Clone, Copy)]
enum Travel {
    Distance,
    /// A second stage's distance, which may be zero.
    Stage,
    Speed,
}

/// The rows of a peel the printer leaves to the resin.
fn travel_rows<const N: usize>(
    ui: &mut egui::Ui,
    rows: [(String, Option<f32>, &mut f32, Travel); N],
) {
    for (label, locked, value, travel) in rows {
        if locked.is_some() {
            continue;
        }
        match travel {
            Travel::Distance => {
                number_row(
                    ui,
                    &label,
                    value,
                    &tr!("unit-mm"),
                    COARSE_STEP,
                    0.1..=100.0,
                    2,
                );
            }
            Travel::Stage => {
                number_row(
                    ui,
                    &label,
                    value,
                    &tr!("unit-mm"),
                    COARSE_STEP,
                    0.0..=100.0,
                    2,
                );
            }
            Travel::Speed => {
                number_row(ui, &label, value, &tr!("unit-mm-min"), 1.0, 1.0..=1000.0, 1);
            }
        };
    }
}
