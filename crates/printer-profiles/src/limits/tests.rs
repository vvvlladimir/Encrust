use std::path::Path;

use super::*;
use crate::PrinterProfile;

/// A machine whose firmware takes a lift of 5 to 15 mm and whose maker advises 8 to 12.
fn limited() -> PrinterLimits {
    PrinterLimits {
        lift_distance_mm: Limit {
            min: Some(5.0),
            max: Some(15.0),
            warn_min: Some(8.0),
            warn_max: Some(12.0),
        },
        ..PrinterLimits::default()
    }
}

fn resin_lifting(lift_distance_mm: f32) -> MaterialProfile {
    MaterialProfile {
        lift_distance_mm,
        ..MaterialProfile::default()
    }
}

#[test]
fn a_value_inside_the_advised_span_strays_from_nothing() {
    assert_eq!(limited().breaches(&resin_lifting(10.0)), []);
}

#[test]
fn a_value_past_the_advice_is_advised_against_with_the_advised_span() {
    let breaches = limited().breaches(&resin_lifting(13.0));
    assert_eq!(
        breaches,
        [LimitBreach {
            field: "lift_distance_mm",
            value: 13.0,
            stray: Stray::Advised,
            span: (Some(8.0), Some(12.0)),
        }]
    );
}

#[test]
fn a_value_past_what_the_machine_takes_is_refused_with_the_hard_span() {
    let breaches = limited().breaches(&resin_lifting(2.0));
    assert_eq!(breaches[0].stray, Stray::Refused);
    assert_eq!(breaches[0].span, (Some(5.0), Some(15.0)));
}

#[test]
fn a_second_stage_setting_is_named_by_its_table() {
    let limits = PrinterLimits {
        second_stage: SecondStageLimits {
            lift_speed_mm_min: Limit {
                max: Some(2700.0),
                ..Limit::default()
            },
            ..SecondStageLimits::default()
        },
        ..PrinterLimits::default()
    };
    let mut resin = MaterialProfile::default();
    resin.second_stage.lift_speed_mm_min = 3000.0;
    let breaches = limits.breaches(&resin);
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].field, "second_stage.lift_speed_mm_min");
}

#[test]
fn the_range_takes_the_bounds_it_states_and_the_fallback_for_the_rest() {
    let speed = Limit {
        max: Some(2700.0),
        ..Limit::default()
    };
    assert_eq!(speed.range(1.0..=1000.0), 1.0..=2700.0);
    assert_eq!(limited().lift_distance_mm.range(0.1..=100.0), 5.0..=15.0);
}

#[test]
fn a_limit_out_of_order_is_refused_by_its_field() {
    let source = r#"
name = "Test"
manufacturer = "Test"

[display]
width_px = 8
height_px = 4
width_mm = 0.8
height_mm = 0.4

[build_volume]
x = 0.8
y = 0.4
z = 10.0

[limits]
layer_height_mm = { min = 0.01, max = 0.2, warn_max = 0.3 }
"#;
    let error = PrinterProfile::from_toml_str(source, Path::new("inline.toml"))
        .expect_err("the advice reaches past the hard bound");
    assert!(matches!(
        error,
        ProfileError::DisorderedLimit {
            field: "layer_height_mm"
        }
    ));
}

#[test]
fn limits_survive_a_round_trip_through_toml() {
    let profile = PrinterProfile {
        limits: limited(),
        ..PrinterProfile::default()
    };
    let text = toml::to_string(&profile).expect("a profile serialises");
    let back = PrinterProfile::from_toml_str(&text, Path::new("inline.toml"))
        .expect("what was written reads back");
    assert_eq!(back.limits, limited());
}
