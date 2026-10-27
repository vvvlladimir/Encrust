#![expect(clippy::expect_used, reason = "a test fails loudly on a bad fixture")]

use std::path::{Path, PathBuf};

use printer_profiles::{Catalogue, ForeignResin, PriceUnit, ProfileError, WaitMode};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read(name: &str) -> ForeignResin {
    let path = fixture(name);
    let text = std::fs::read_to_string(&path).expect("the fixture is checked in");
    ForeignResin::from_str(&text, &path).expect("the fixture is a resin profile")
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

/// The test resin was entered with numbers no default has: 2.37 s, 31.3 s, 7 bottom layers.
#[test]
fn a_cfgx_gives_every_setting_it_holds() {
    let foreign = read("saturn-4-ultra.cfgx");
    let resin = &foreign.resin;
    assert_eq!(
        resin.name, "ENCRUST-TEST",
        "the leading line break is trimmed"
    );
    assert!(close(resin.exposure_s, 2.37) && close(resin.bottom_exposure_s, 31.3));
    assert_eq!((resin.bottom_layers, resin.transition_layers), (7, 6));
    assert!(close(resin.layer_height_mm, 0.035));
    assert_eq!((resin.light_pwm, resin.bottom_light_pwm), (211, 233));
    assert_eq!(resin.waits.mode, WaitMode::Rest);
    assert_eq!(
        resin
            .waits
            .rests_s()
            .map(|rest| (rest * 10.0).round() as u32),
        [13, 7, 21],
        "before lift, after lift, after retract as entered"
    );
    assert!(close(resin.density_g_cm3, 1.13));
    assert_eq!(resin.details.color, [0x4E, 0xA8, 0x45], "5154885 as RGB");
    assert_eq!(resin.details.kind, "enctype");
    assert!(close(resin.details.price, 37.0));
    assert_eq!(resin.details.currency, "$");
    assert_eq!(resin.details.price_per, PriceUnit::Kilogram);

    let compensation = &resin.compensation;
    assert!(close(compensation.shrink_x_pct, 101.234));
    assert!(close(compensation.shrink_z_pct, 99.891));
    assert!(close(compensation.hole_offset_mm, 0.031));
    assert!(close(compensation.outer_offset_mm, 0.047));
    assert!(close(compensation.bottom_outer_offset_mm, 0.067));
    assert!(close(compensation.parity_offset_mm, 0.019));
    assert!(close(compensation.layer_time_s, 5.43));

    assert_eq!(foreign.machine.as_deref(), Some("ELEGOO Saturn 4 Ultra"));
    assert_eq!(foreign.panel_px, Some([11_520, 5_120]));
}

#[test]
fn a_tilting_vats_placeholder_peel_keeps_our_defaults() {
    let resin = read("saturn-4-ultra.cfgx").resin;
    let ours = printer_profiles::MaterialProfile::default();
    assert_eq!(
        (resin.lift_distance_mm, resin.lift_speed_mm_min),
        (ours.lift_distance_mm, ours.lift_speed_mm_min),
        "the file states the layer height and 0.05 mm/min where a peel would be"
    );
}

#[test]
fn a_peeling_machines_cfgx_gives_its_lifts_and_holds_the_light_off() {
    let foreign = read("photon.cfgx");
    let resin = &foreign.resin;
    assert!(close(resin.lift_distance_mm, 5.3) && close(resin.bottom_lift_distance_mm, 6.7));
    assert!(
        close(resin.retract_distance_mm, 5.8),
        "back down as far as both stages went up, less the slow last stretch"
    );
    let stage = resin.second_stage;
    assert!(close(stage.lift_distance_mm, 2.0) && close(stage.lift_speed_mm_min, 180.0));
    assert!(close(stage.retract_distance_mm, 1.5) && close(stage.retract_speed_mm_min, 60.0));
    assert!(
        close(stage.bottom_lift_distance_mm, 3.0) && close(stage.bottom_retract_distance_mm, 2.0)
    );
    assert!(
        close(resin.bottom_retract_distance_mm(), 7.7),
        "6.7 + 3 - 2"
    );
    assert_eq!(resin.bottom_light_off_delay_s, Some(1.5));
    assert!(close(resin.lift_speed_mm_min, 61.0) && close(resin.bottom_lift_speed_mm_min, 71.0));
    assert!(close(resin.retract_speed_mm_min, 173.0));
    assert!(close(resin.bottom_retract_speed_mm_min, 157.0));
    assert_eq!(resin.waits.mode, WaitMode::LightOff);
    assert_eq!(foreign.panel_px, Some([1_440, 2_560]));
}

#[test]
fn a_saved_material_ini_gives_exposure_fade_correction_and_bottle() {
    let foreign = read("saved-configuration.ini");
    let resin = &foreign.resin;
    assert_eq!(resin.name, "ENCRUST-TEST");
    assert!(close(resin.exposure_s, 2.37) && close(resin.bottom_exposure_s, 31.3));
    assert_eq!(
        (resin.bottom_layers, resin.transition_layers),
        (1, 8),
        "the override's 10 faded layers, as the archive's numFade shows: one first layer and \
         a ramp to layer 9"
    );
    assert!(close(resin.compensation.shrink_x_pct, 101.234));
    assert!(close(resin.compensation.shrink_y_pct, 100.567));
    assert!(
        close(resin.compensation.bottom_outer_offset_mm, -0.053),
        "an elephant foot is taken off by a negative outer offset"
    );
    assert_eq!(resin.details.color, [0x12, 0xAB, 0x34]);
    assert!(close(resin.details.price, 37.0), "37 for a 1000 ml bottle");
    assert_eq!(resin.details.price_per, PriceUnit::Litre);
    assert_eq!(foreign.machine, None, "the printer model is left blank");
    assert_eq!(foreign.panel_px, Some([11_520, 5_120]));
}

#[test]
fn a_cfgx_is_set_up_on_the_machine_it_names() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    assert_eq!(
        catalogue.printer_for_foreign(&read("saturn-4-ultra.cfgx")),
        Some("elegoo-saturn-4-ultra")
    );
    assert_eq!(
        catalogue.printer_for_foreign(&read("saved-configuration.ini")),
        None,
        "five machines share the panel and the file names none"
    );
}

#[test]
fn a_profile_without_a_panel_is_found_by_name_alone() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let mut foreign = read("saved-configuration.ini");
    foreign.panel_px = None;
    foreign.machine = Some("Elegoo Saturn 4 Ultra".to_owned());
    assert_eq!(
        catalogue.printer_for_foreign(&foreign),
        Some("elegoo-saturn-4-ultra")
    );
}

#[test]
fn a_profile_naming_no_resin_is_named_after_its_file() {
    let foreign = ForeignResin::from_str("exposure_time = 3\n", Path::new("bench grey.ini"))
        .expect("an exposure is a resin");
    assert_eq!(foreign.resin.name, "bench grey");
}

#[test]
fn a_cfgx_that_is_not_json_is_refused() {
    let refused = ForeignResin::from_str("{ material:", Path::new("broken.cfgx"));
    assert!(
        matches!(refused, Err(ProfileError::Json { .. })),
        "{refused:?}"
    );
}

#[test]
fn a_file_of_another_kind_is_refused() {
    let refused = ForeignResin::from_str("", Path::new("resin.lyr"));
    assert!(matches!(refused, Err(ProfileError::UnknownFormat { .. })));
    assert!(!ForeignResin::reads(Path::new("resin.toml")));
}

#[test]
fn a_profile_with_no_exposure_is_refused() {
    let refused = ForeignResin::from_str("layer_height = 0.05\n", Path::new("print.ini"));
    assert!(matches!(
        refused,
        Err(ProfileError::MissingSetting {
            key: "exposure_time",
            ..
        })
    ));
    let refused = ForeignResin::from_str(r#"{"printerinfo": {}}"#, Path::new("machine.cfgx"));
    assert!(matches!(
        refused,
        Err(ProfileError::MissingSetting {
            key: "material",
            ..
        })
    ));
}

#[test]
fn a_cfgx_gives_its_buffer_with_the_switch_as_it_stands() {
    let path = fixture("saturn-4-ultra.cfgx");
    let text = std::fs::read_to_string(&path).expect("the fixture is checked in");
    let mut root: serde_json::Value = serde_json::from_str(&text).expect("the fixture is JSON");
    let print = &mut root["material"][0]["slice"][0]["print"];
    for (key, value) in [
        ("bufferenable", serde_json::json!(true)),
        ("bufferheight", serde_json::json!(3.7)),
        ("bufferMotrospeed", serde_json::json!(97.0)),
        ("bufferlightoffdelay", serde_json::json!(4.3)),
    ] {
        print[key]["currentvalue"] = value;
    }
    let foreign = ForeignResin::from_str(&root.to_string(), &path).expect("still a resin");
    let buffer = foreign.resin.buffer.expect("the file has a buffer");
    assert!(buffer.enabled);
    assert!(
        close(buffer.height_mm, 3.7)
            && close(buffer.speed_mm_min, 97.0)
            && close(buffer.light_off_delay_s, 4.3),
        "3.7 mm at 97 mm/min, then 4.3 s, got {buffer:?}"
    );
}
