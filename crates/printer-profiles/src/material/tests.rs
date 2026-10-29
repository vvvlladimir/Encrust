use super::*;

#[test]
fn a_resin_carried_to_another_height_cures_every_layer_as_before() {
    for penetration_depth_mm in [None, Some(0.12)] {
        let resin = MaterialProfile {
            penetration_depth_mm,
            ..MaterialProfile::default()
        };
        let carried = resin.rescaled_to(0.03);
        assert!((carried.layer_height_mm - 0.03).abs() < f32::EPSILON);
        for thickness_mm in [0.02, 0.03, 0.05, 0.1] {
            let before = resin.exposure_at(resin.exposure_s, thickness_mm);
            let after = carried.exposure_at(carried.exposure_s, thickness_mm);
            assert!(
                (before - after).abs() < 1e-4,
                "at {thickness_mm} mm: {before} s before, {after} s after"
            );
        }
    }
}

#[test]
fn carrying_a_resin_to_another_height_leaves_the_bottom_block_alone() {
    let resin = MaterialProfile::default();
    let carried = resin.rescaled_to(0.1);
    assert!(
        carried.exposure_s > resin.exposure_s,
        "a thicker layer takes longer"
    );
    assert!((carried.bottom_exposure_s - resin.bottom_exposure_s).abs() < f32::EPSILON);
}

fn path() -> &'static Path {
    Path::new("inline.toml")
}

#[test]
fn a_machine_holding_its_wait_mode_leaves_the_waiting_open_until_it_locks_the_wait() {
    let held = PrinterTuning {
        wait_mode: Some(WaitMode::LightOff),
        ..PrinterTuning::default()
    };
    assert_eq!(held.held_wait_mode(), Some(WaitMode::LightOff));
    assert!(
        !held.locks_waiting(),
        "the light-off delay is still the resin's"
    );

    let locked = PrinterTuning {
        light_off_delay_s: Some(1.0),
        ..held
    };
    assert!(
        locked.locks_waiting(),
        "mode and delay are both the machine's"
    );
}

#[test]
fn default_survives_a_toml_round_trip() {
    let original = MaterialProfile::default();
    let text = toml::to_string(&original).expect("serialises");
    let parsed: MaterialProfile = toml::from_str(&text).expect("parses back");
    assert_eq!(original, parsed);
}

#[test]
fn bottom_layers_are_exposed_longer() {
    let profile = MaterialProfile::default();
    assert!(profile.bottom_exposure_s > profile.exposure_s);
}

#[test]
fn a_profile_without_the_motion_fields_takes_the_defaults() {
    let profile = MaterialProfile::from_toml_str(
        r#"
name = "Old profile"
layer_height_mm = 0.05
exposure_s = 2.5
bottom_exposure_s = 30.0
bottom_layers = 6
light_off_delay_s = 0.5
lift_distance_mm = 6.0
lift_speed_mm_min = 65.0
retract_speed_mm_min = 150.0
"#,
        path(),
    )
    .expect("valid profile");

    assert_eq!(profile.light_pwm, 255);
    assert_eq!(profile.transition_layers, 0);
    assert!((profile.retract_distance_mm - 6.0).abs() < f32::EPSILON);
}

#[test]
fn zero_exposure_is_rejected() {
    let err = MaterialProfile::from_toml_str(
        r#"
name = "Broken"
layer_height_mm = 0.05
exposure_s = 0.0
bottom_exposure_s = 30.0
bottom_layers = 6
light_off_delay_s = 0.5
lift_distance_mm = 6.0
lift_speed_mm_min = 65.0
retract_speed_mm_min = 150.0
"#,
        path(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositive {
            field: "exposure_s",
            ..
        }
    ));
}

#[test]
fn a_negative_rest_is_rejected() {
    let resin = MaterialProfile {
        waits: Waits {
            after_lift_s: -1.0,
            ..Waits::default()
        },
        ..MaterialProfile::default()
    };
    let text = toml::to_string(&resin).expect("serialises");
    let error = MaterialProfile::from_toml_str(&text, path()).expect_err("rejected");
    assert!(matches!(
        error,
        ProfileError::Negative {
            field: "waits.after_lift_s",
            ..
        }
    ));
}

#[test]
fn a_printer_not_yet_set_up_starts_from_the_last_one_tuned() {
    let mut resin = MaterialProfile::default();
    resin.printers.insert(
        "mars".to_owned(),
        PrinterTuning {
            exposure_s: Some(3.3),
            ..PrinterTuning::default()
        },
    );
    resin.last_printer = Some("mars".to_owned());

    let fresh = resin.starting_point("saturn");
    assert!(
        (fresh.exposure_s - 3.3).abs() < 1e-6,
        "the Mars numbers carry over"
    );
    assert!(
        !resin.is_tuned_for("saturn"),
        "starting from them tunes nothing yet"
    );

    resin.last_printer = None;
    let bare = resin.starting_point("saturn");
    assert!((bare.exposure_s - MaterialProfile::default().exposure_s).abs() < 1e-6);
}

#[test]
fn resting_replaces_the_light_off_delay() {
    let mut resin = MaterialProfile::default();
    assert!((resin.wait_per_layer_s() - resin.light_off_delay_s).abs() < 1e-6);
    resin.waits = Waits {
        mode: WaitMode::Rest,
        before_lift_s: 1.0,
        after_lift_s: 0.5,
        after_retract_s: 2.0,
    };
    assert!((resin.wait_per_layer_s() - 3.5).abs() < 1e-6);
    assert!(resin.light_off_s().abs() < 1e-6);
}

#[test]
fn a_resin_is_priced_by_weight_or_by_volume() {
    let mut resin = MaterialProfile::default();
    assert_eq!(resin.cost(500.0, 450_000.0), None, "no price, no cost");
    resin.details.price = 20.0;
    let by_weight = resin.cost(500.0, 450_000.0).expect("priced");
    assert!((by_weight - 10.0).abs() < 1e-4, "half a kilo at 20 a kilo");
    resin.details.price_per = PriceUnit::Litre;
    let by_volume = resin.cost(500.0, 450_000.0).expect("priced");
    assert!((by_volume - 9.0).abs() < 1e-4, "0.45 l at 20 a litre");
}

#[test]
fn a_tuning_carries_its_own_waits() {
    let base = MaterialProfile::default();
    let tuned = MaterialProfile {
        waits: Waits {
            mode: WaitMode::Rest,
            after_retract_s: 1.0,
            ..Waits::default()
        },
        ..base.clone()
    };
    let tuning = PrinterTuning::of_changes(&base, &tuned);
    let mut resin = base;
    resin.printers.insert("mars".to_owned(), tuning);
    assert_eq!(resin.for_printer("mars").waits, tuned.waits);
    let text = toml::to_string_pretty(&resin).expect("tables after values");
    assert_eq!(
        MaterialProfile::from_toml_str(&text, path()).expect("loads"),
        resin
    );
}

#[test]
fn malformed_toml_reports_its_path() {
    let err = MaterialProfile::from_toml_str("name = ", path()).unwrap_err();
    assert!(matches!(err, ProfileError::Parse { .. }));
}

#[test]
fn without_transition_layers_the_exposure_steps_straight_down() {
    let profile = MaterialProfile::default();
    assert!((profile.exposure_of_layer_s(5) - 30.0).abs() < 1e-6);
    assert!((profile.exposure_of_layer_s(6) - 2.5).abs() < 1e-6);
    assert!(profile.is_bottom_layer(5));
    assert!(!profile.is_bottom_layer(6));
}

#[test]
fn transition_layers_ramp_from_the_bottom_exposure_to_the_normal_one() {
    let profile = MaterialProfile {
        bottom_layers: 2,
        bottom_exposure_s: 10.0,
        exposure_s: 2.0,
        transition_layers: 3,
        ..MaterialProfile::default()
    };

    // Four equal steps of -2 s across three transition layers: 8, 6, 4, then 2.
    assert!((profile.exposure_of_layer_s(2) - 8.0).abs() < 1e-5);
    assert!((profile.exposure_of_layer_s(3) - 6.0).abs() < 1e-5);
    assert!((profile.exposure_of_layer_s(4) - 4.0).abs() < 1e-5);
    assert!((profile.exposure_of_layer_s(5) - 2.0).abs() < 1e-5);
}

const TUNED: &str = r#"
name = "Tuned"
layer_height_mm = 0.05
exposure_s = 2.5
bottom_exposure_s = 30.0
bottom_layers = 6
light_off_delay_s = 0.5
lift_distance_mm = 6.0
lift_speed_mm_min = 65.0
retract_speed_mm_min = 150.0

[printers.fast-machine]
exposure_s = 1.8
lift_speed_mm_min = 120.0
"#;

#[test]
fn a_tuning_replaces_only_the_settings_it_names() {
    let resin = MaterialProfile::from_toml_str(TUNED, path()).expect("valid profile");
    let tuned = resin.for_printer("fast-machine");

    assert!((tuned.exposure_s - 1.8).abs() < 1e-6);
    assert!((tuned.lift_speed_mm_min - 120.0).abs() < 1e-6);
    assert!((tuned.bottom_exposure_s - 30.0).abs() < 1e-6);
    assert_eq!(tuned.name, resin.name);
    assert!(resin.is_tuned_for("fast-machine"));
}

#[test]
fn a_printer_the_resin_says_nothing_about_keeps_the_resins_own_numbers() {
    let resin = MaterialProfile::from_toml_str(TUNED, path()).expect("valid profile");
    let tuned = resin.for_printer("other-machine");

    assert!((tuned.exposure_s - resin.exposure_s).abs() < 1e-6);
    assert!(!resin.is_tuned_for("other-machine"));
}

#[test]
fn a_tuning_read_back_off_two_profiles_reproduces_them() {
    let resin = MaterialProfile::from_toml_str(TUNED, path()).expect("valid profile");
    let mut edited = resin.for_printer("fast-machine");
    edited.bottom_exposure_s = 21.0;

    let tuning = PrinterTuning::of_changes(&resin, &edited);
    assert!(!tuning.is_empty());
    assert_eq!(tuning.bottom_exposure_s, Some(21.0));
    assert_eq!(
        tuning.bottom_layers, None,
        "a setting the user did not touch stays the resin's own"
    );

    let mut round_tripped = resin.clone();
    round_tripped
        .printers
        .insert("fast-machine".to_owned(), tuning);
    let mut back = round_tripped.for_printer("fast-machine");
    back.printer_table = edited.printer_table.clone();
    assert_eq!(back, edited);
}

#[test]
fn an_unedited_resin_needs_no_table() {
    let resin = MaterialProfile::default();
    assert!(PrinterTuning::of_changes(&resin, &resin).is_empty());
}

#[test]
fn a_zero_in_a_tuning_is_rejected_and_names_its_printer() {
    let broken = TUNED.replace("exposure_s = 1.8", "exposure_s = 0.0");
    let err = MaterialProfile::from_toml_str(&broken, path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositiveTuning {
            field: "exposure_s",
            ..
        }
    ));
}

#[test]
fn a_misspelled_tuning_key_is_rejected_rather_than_ignored() {
    let broken = TUNED.replace("lift_speed_mm_min = 120.0", "lift_speed = 120.0");
    let err = MaterialProfile::from_toml_str(&broken, path()).unwrap_err();
    assert!(matches!(err, ProfileError::Parse { .. }));
}
