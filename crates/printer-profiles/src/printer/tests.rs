use super::*;
use crate::PrinterTuning;

const SAMPLE: &str = r#"
name = "Test"
manufacturer = "Test"

[display]
width_px = 100
height_px = 50
width_mm = 10.0
height_mm = 5.0

[build_volume]
x = 10.0
y = 5.0
z = 100.0
"#;

fn path() -> &'static Path {
    Path::new("inline.toml")
}

#[test]
fn a_profile_that_names_no_machine_writes_its_own_name() {
    let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("a valid profile");
    assert_eq!(profile.machine_name(), "Test");

    let named = SAMPLE.replace(
        r#"manufacturer = "Test""#,
        "manufacturer = \"Test\"\nmachine_name = \"TEST Machine 1\"",
    );
    let profile = PrinterProfile::from_toml_str(&named, path()).expect("a valid profile");
    assert_eq!(profile.machine_name(), "TEST Machine 1");
    assert_eq!(
        profile.name, "Test",
        "the picker still shows the short name"
    );
}

#[test]
fn a_profile_that_states_no_floor_keeps_every_grey() {
    let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("a valid profile");
    assert_eq!(profile.display.grey_floor, 0);
}

#[test]
fn a_profile_that_says_nothing_reads_its_per_layer_tables() {
    let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("a valid profile");
    assert!(profile.firmware.per_layer_settings);
    assert!(!profile.firmware.variable_layer_height);

    let older = format!("{SAMPLE}\n[firmware]\nper_layer_settings = false\n");
    let profile = PrinterProfile::from_toml_str(&older, path()).expect("a valid profile");
    assert!(!profile.firmware.per_layer_settings);
}

#[test]
fn the_blank_machine_is_a_profile_that_loads() {
    let blank = PrinterProfile::default();
    let text = toml::to_string(&blank).expect("serialises");
    let parsed = PrinterProfile::from_toml_str(&text, path()).expect("a valid profile");
    assert_eq!(parsed, blank);
}

#[test]
fn a_profile_naming_no_format_gets_goo_and_one_naming_ctb_keeps_it() {
    let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("valid profile");
    assert_eq!(profile.output, OutputFormat::Goo);

    let chitu = SAMPLE.replace(
        "manufacturer = \"Test\"",
        "manufacturer = \"Test\"\noutput = \"ctb5\"",
    );
    let profile = PrinterProfile::from_toml_str(&chitu, path()).expect("valid profile");
    assert_eq!(profile.output, OutputFormat::Ctb5);
}

#[test]
fn a_profile_naming_no_connection_takes_files_on_a_stick() {
    let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("valid profile");
    assert_eq!(profile.connection, Connection::None);

    let networked = SAMPLE.replace(
        "manufacturer = \"Test\"",
        "manufacturer = \"Test\"\nconnection = \"sdcp\"",
    );
    let profile = PrinterProfile::from_toml_str(&networked, path()).expect("valid profile");
    assert_eq!(profile.connection, Connection::Sdcp);
}

#[test]
fn a_profile_lists_its_printers_on_the_network_and_writes_them_back() {
    let listed = SAMPLE.replace(
        "manufacturer = \"Test\"",
        "manufacturer = \"Test\"\nconnection = \"sdcp\"\n\n\
             [[machines]]\nname = \"Bench A\"\naddress = \"192.168.1.133\"\n\
             board = \"d85f46e4d7d90100\"\n\n\
             [[machines]]\naddress = \"192.168.1.140\"",
    );
    let profile = PrinterProfile::from_toml_str(&listed, path()).expect("valid profile");
    assert_eq!(profile.machines.len(), 2);
    assert_eq!(profile.machines[0].label(), "Bench A");
    assert_eq!(
        profile.machines[0].board.as_deref(),
        Some("d85f46e4d7d90100")
    );
    assert_eq!(
        profile.machines[1].label(),
        "192.168.1.140",
        "no name shows the address"
    );

    let written = profile
        .to_toml_string(path())
        .expect("a valid profile writes");
    let read = PrinterProfile::from_toml_str(&written, path()).expect("and reads back");
    assert_eq!(read.machines, profile.machines);
    let plain = PrinterProfile::from_toml_str(SAMPLE, path()).expect("valid profile");
    let written = plain
        .to_toml_string(path())
        .expect("a valid profile writes");
    assert!(
        !written.contains("machines"),
        "an empty list is left out: {written}"
    );
}

#[test]
fn a_printer_listed_without_an_address_is_refused() {
    let listed = SAMPLE.replace(
        "manufacturer = \"Test\"",
        "manufacturer = \"Test\"\n\n[[machines]]\nname = \"Bench A\"\naddress = \" \"",
    );
    assert!(matches!(
        PrinterProfile::from_toml_str(&listed, path()),
        Err(ProfileError::NoAddress { index: 0 })
    ));
}

#[test]
fn a_locked_setting_overrides_the_resin_and_one_left_open_keeps_it() {
    let locked = format!("{SAMPLE}\n[locked]\nlift_speed_mm_min = 0.05\nlight_pwm = 255\n");
    let profile = PrinterProfile::from_toml_str(&locked, path()).expect("valid profile");
    let resin = MaterialProfile {
        lift_speed_mm_min: 65.0,
        light_pwm: 200,
        exposure_s: 2.5,
        ..MaterialProfile::default()
    };

    let printed = resin.on(&profile);
    assert_eq!((printed.lift_speed_mm_min, printed.light_pwm), (0.05, 255));
    assert_eq!(
        printed.exposure_s, 2.5,
        "what is not locked stays the resin's"
    );

    let written = profile
        .to_toml_string(path())
        .expect("a valid profile writes");
    let read = PrinterProfile::from_toml_str(&written, path()).expect("and reads back");
    assert_eq!(read.locked, profile.locked);
    let plain = PrinterProfile::from_toml_str(SAMPLE, path()).expect("valid profile");
    let written = plain
        .to_toml_string(path())
        .expect("a valid profile writes");
    assert!(
        !written.contains("locked"),
        "nothing locked is left out: {written}"
    );
}

#[test]
fn a_resins_table_for_the_machine_overrides_its_lock_and_another_machines_does_not() {
    let locked = format!("{SAMPLE}\n[locked]\ntransition_layers = 8\n");
    let profile = PrinterProfile::from_toml_str(&locked, path()).expect("valid profile");
    let mut resin = MaterialProfile::default();
    let zero = PrinterTuning {
        transition_layers: Some(0),
        ..PrinterTuning::default()
    };
    resin.printers.insert("ceres".to_owned(), zero.clone());
    resin.printers.insert("other".to_owned(), zero);
    resin.last_printer = Some("other".to_owned());

    let here = resin.starting_point("ceres").on(&profile);
    assert_eq!(here.transition_layers, 0, "the table for this machine wins");
    let fresh = resin.starting_point("new-machine").on(&profile);
    assert_eq!(
        fresh.transition_layers, 8,
        "another machine's table is no statement here"
    );
    assert_eq!(MaterialProfile::default().on(&profile).transition_layers, 8);
}

#[test]
fn a_locked_speed_of_zero_is_refused() {
    let locked = format!("{SAMPLE}\n[locked]\nlift_speed_mm_min = 0.0\n");
    assert!(matches!(
        PrinterProfile::from_toml_str(&locked, path()),
        Err(ProfileError::NonPositiveLocked {
            field: "lift_speed_mm_min",
            ..
        })
    ));
}

#[test]
fn pitch_is_area_over_pixel_count() {
    let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("valid profile");
    assert_eq!(profile.display.pixel_pitch_mm(), (0.1, 0.1));
    assert!(!profile.mirror_x);
}

#[test]
fn zero_dimension_is_rejected() {
    let broken = SAMPLE.replace("width_mm = 10.0", "width_mm = 0.0");
    let err = PrinterProfile::from_toml_str(&broken, path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositive {
            field: "display.width_mm",
            ..
        }
    ));
}

#[test]
fn malformed_toml_reports_its_path() {
    let err = PrinterProfile::from_toml_str("name = ", path()).unwrap_err();
    assert!(matches!(err, ProfileError::Parse { .. }));
}
