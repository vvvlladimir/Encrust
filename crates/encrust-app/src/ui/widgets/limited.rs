use std::ops::RangeInclusive;

use egui::Ui;
use egui::emath::Numeric;
use printer_profiles::{Limit, Stray};

use super::field::{Field, boxed_field, row};
use super::hint;
use crate::i18n::tr;
use crate::ui::theme;

/// What a machine holds one setting to, and why its maker advises the span it does.
#[derive(Clone, Copy)]
pub struct Held<'a> {
    pub limit: &'a Limit,
    pub why: Option<&'a str>,
}

/// A `number_row` held to a machine's limit (ADR 0369): its box takes only what the machine
/// takes, and a value past that or past what the maker advises is boxed in `danger`, with
/// the span it strays from under it.
#[allow(clippy::too_many_arguments)]
pub fn limited_row<N: Numeric>(
    ui: &mut Ui,
    label: &str,
    value: &mut N,
    unit: &str,
    speed: f64,
    range: RangeInclusive<N>,
    decimals: Option<usize>,
    held: Held<'_>,
) -> bool {
    let fallback = range.start().to_f64() as f32..=range.end().to_f64() as f32;
    let taken = held.limit.range(fallback);
    let range = N::from_f64(f64::from(*taken.start()))..=N::from_f64(f64::from(*taken.end()));
    let stray = held.limit.stray(value.to_f64() as f32);
    let field = Field {
        name: label,
        unit,
        width: theme::FIELD_W,
        flagged: stray.is_some(),
    };
    let changed = row(ui, label, |ui| {
        boxed_field(ui, &field, value, speed, range, decimals)
    });
    limit_verdict(ui, held, value.to_f64() as f32, unit, decimals);
    changed
}

/// Under a row drawn some other way, the span `value` strays from and why, if it strays.
pub fn limit_verdict(ui: &mut Ui, held: Held<'_>, value: f32, unit: &str, decimals: Option<usize>) {
    let Some(stray) = held.limit.stray(value) else {
        return;
    };
    hint(ui, &verdict(held.limit.span(stray), stray, unit, decimals));
    if let (Stray::Advised, Some(why)) = (stray, held.why) {
        hint(ui, why);
    }
}

/// The one line saying which span a value strays from.
fn verdict(
    span: (Option<f32>, Option<f32>),
    stray: Stray,
    unit: &str,
    decimals: Option<usize>,
) -> String {
    let figure = |value: f32| format!("{value:.*}", decimals.unwrap_or(0));
    match (stray, span) {
        (Stray::Refused, (Some(low), Some(high))) => {
            tr!(
                "limit-takes-between",
                low = figure(low),
                high = figure(high),
                unit = unit
            )
        }
        (Stray::Refused, (Some(low), None)) => {
            tr!("limit-takes-from", low = figure(low), unit = unit)
        }
        (Stray::Refused, (None, high)) => {
            tr!(
                "limit-takes-up-to",
                high = figure(high.unwrap_or(0.0)),
                unit = unit
            )
        }
        (Stray::Advised, (Some(low), Some(high))) => {
            tr!(
                "limit-advised-between",
                low = figure(low),
                high = figure(high),
                unit = unit
            )
        }
        (Stray::Advised, (Some(low), None)) => {
            tr!("limit-advised-from", low = figure(low), unit = unit)
        }
        (Stray::Advised, (None, high)) => {
            tr!(
                "limit-advised-up-to",
                high = figure(high.unwrap_or(0.0)),
                unit = unit
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    /// A lift the machine takes from 5 to 15 mm, and whose maker advises 8 to 12.
    const LIFT: Limit = Limit {
        min: Some(5.0),
        max: Some(15.0),
        warn_min: Some(8.0),
        warn_max: Some(12.0),
    };

    /// The row on its own, holding `lift_mm`, after the theme has been put on.
    fn harness(lift_mm: f32) -> egui_kittest::Harness<'static, f32> {
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(320.0, 120.0))
            .build_ui_state(
                move |ui, lift_mm: &mut f32| {
                    if dressed {
                        let held = Held {
                            limit: &LIFT,
                            why: Some("Too low a lift tears the layer."),
                        };
                        limited_row(ui, "Lift", lift_mm, "mm", 0.1, 0.1..=100.0, Some(1), held);
                    } else {
                        theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                lift_mm,
            );
        harness.run_steps(3);
        harness
    }

    #[test]
    fn a_value_past_the_advice_states_the_advised_span_and_why() {
        let harness = harness(6.0);
        harness.get_by_label("Its maker advises 8.0–12.0 mm.");
        harness.get_by_label("Too low a lift tears the layer.");
    }

    #[test]
    fn a_value_past_what_the_machine_takes_states_the_hard_span_alone() {
        let harness = harness(20.0);
        harness.get_by_label("This machine takes 5.0–15.0 mm.");
        assert!(
            harness
                .query_by_label("Too low a lift tears the layer.")
                .is_none(),
            "the maker's reason is for its advice, not for the machine's bound"
        );
    }

    #[test]
    fn a_typed_figure_is_held_to_what_the_machine_takes() {
        let mut harness = harness(10.0);
        harness
            .get_by_role_and_label(Role::SpinButton, "Lift")
            .click();
        harness.run_steps(3);
        harness
            .get_by_role_and_label(Role::SpinButton, "Lift")
            .type_text("40");
        harness.run_steps(3);
        harness.key_press(egui::Key::Enter);
        harness.run_steps(3);
        assert_eq!(*harness.state(), 15.0, "the box goes no further than 15 mm");
    }

    #[test]
    fn a_value_inside_the_advice_says_nothing() {
        let harness = harness(10.0);
        assert!(harness.query_by_label_contains("advises").is_none());
    }
}
