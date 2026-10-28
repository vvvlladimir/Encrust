use std::path::{Path, PathBuf};

use printer_profiles::{
    Catalogue, Connection, MaterialProfile, OutputFormat, PrinterProfile, WaitMode, Waits,
};

fn asset(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/profiles")
        .join(name)
}

#[test]
fn shipped_profile_parses_and_its_pitch_is_plausible() {
    let profile = PrinterProfile::load(&asset("printers/elegoo-mars-4-ultra.toml"))
        .expect("shipped profile is valid");

    assert_eq!(profile.manufacturer, "Elegoo");
    assert!(profile.mirror_x);

    // A 9" 9K panel sits around 18 um; anything outside 10..40 um means a wrong spec.
    let (pitch_x, pitch_y) = profile.display.pixel_pitch_mm();
    assert!(
        (0.010..0.040).contains(&pitch_x),
        "unexpected x pitch {pitch_x}"
    );
    assert!(
        (0.010..0.040).contains(&pitch_y),
        "unexpected y pitch {pitch_y}"
    );
}

#[test]
fn every_shipped_panel_has_a_plausible_pixel_pitch() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");

    for entry in catalogue.printers() {
        // From about 18 um on a 12K panel to about 140 um on the 4K screen of a 550 mm
        // machine; outside that range a transcribed specification is wrong.
        let (pitch_x, pitch_y) = entry.profile.display.pixel_pitch_mm();
        assert!(
            (0.010..0.150).contains(&pitch_x) && (0.010..0.150).contains(&pitch_y),
            "{} has an implausible pitch of {pitch_x} x {pitch_y} mm",
            entry.id
        );
        assert!(
            entry.profile.build_volume.x >= entry.profile.display.width_mm,
            "{} claims a build volume narrower than its own panel",
            entry.id
        );
    }
}

#[test]
fn every_shipped_printer_names_the_format_its_firmware_reads() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");

    let expected = [
        ("elegoo-mars-4-ultra", OutputFormat::Goo),
        ("elegoo-saturn-4-ultra", OutputFormat::Goo),
        ("elegoo-mars-3-pro", OutputFormat::CtbEncrypted),
        ("phrozen-sonic-mini-8k", OutputFormat::CtbEncrypted),
        ("uniformation-gktwo", OutputFormat::CtbEncrypted),
        ("creality-ld-002h", OutputFormat::Ctb5),
    ];
    for (id, format) in expected {
        let entry = catalogue.printer(id).expect("a shipped machine");
        assert_eq!(entry.profile.output, format, "{id} writes the wrong format");
    }
}

#[test]
fn only_the_shipped_machines_with_wifi_are_reachable_over_the_network() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");

    let expected = [
        ("elegoo-mars-4-ultra", Connection::Sdcp),
        ("elegoo-saturn-4-ultra", Connection::Sdcp),
        ("elegoo-mars-3-pro", Connection::None),
        ("phrozen-sonic-mini-8k", Connection::None),
        ("uniformation-gktwo", Connection::None),
    ];
    for (id, connection) in expected {
        let entry = catalogue.printer(id).expect("a shipped machine");
        assert_eq!(
            entry.profile.connection, connection,
            "{id} is reached the wrong way"
        );
    }
}

#[test]
fn the_saturn_4_ultra_reads_the_header_alone() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let entry = catalogue
        .printer("elegoo-saturn-4-ultra")
        .expect("a shipped machine");

    assert!(
        !entry.profile.firmware.per_layer_settings,
        "a tilting vat stalls on the per-layer lift, see ADR 0142"
    );
}

#[test]
fn every_tilting_vat_states_no_lift_of_its_own() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    for id in [
        "elegoo-saturn-4-ultra",
        "elegoo-saturn-4-ultra-16k",
        "elegoo-saturn-5-ultra",
        "elegoo-mars-5-ultra",
    ] {
        let entry = catalogue.printer(id).expect("a shipped machine");
        let printed = MaterialProfile::default().on(&entry.profile);

        // The vendor's own files for these carry 0.05 in every peel field, see ADR 0301.
        for value in [
            printed.lift_distance_mm,
            printed.lift_speed_mm_min,
            printed.retract_distance_mm,
            printed.retract_speed_mm_min,
            printed.bottom_lift_distance_mm,
            printed.bottom_lift_speed_mm_min,
            printed.bottom_retract_speed_mm_min,
        ] {
            assert_eq!(value, 0.05, "{id}: the tilting vat runs its own motion");
        }
    }
}

#[test]
fn the_saturn_4_ultra_rests_and_keeps_the_resins_rests() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let entry = catalogue
        .printer("elegoo-saturn-4-ultra")
        .expect("a shipped machine");
    let resin = MaterialProfile {
        waits: Waits {
            mode: WaitMode::LightOff,
            after_retract_s: 1.3,
            ..Waits::default()
        },
        ..MaterialProfile::default()
    };

    let printed = resin.on(&entry.profile);
    assert_eq!(
        printed.waits.mode,
        WaitMode::Rest,
        "the vendor slicer offers no other"
    );
    assert_eq!(
        printed.waits.after_retract_s, 1.3,
        "the rests stay the resin's"
    );
}
