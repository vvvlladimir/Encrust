use std::path::{Path, PathBuf};

use printer_profiles::{Catalogue, Connection, OutputFormat, PrinterProfile, SupportProfile};

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
        ("elegoo-mars-3-pro", OutputFormat::Ctb4),
        ("phrozen-sonic-mini-8k", OutputFormat::Ctb4),
        ("uniformation-gktwo", OutputFormat::Ctb4),
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
fn every_shipped_support_preset_matches_the_built_in_one() {
    let expected = [
        ("light.toml", SupportProfile::light()),
        ("medium.toml", SupportProfile::medium()),
        ("heavy.toml", SupportProfile::heavy()),
    ];

    for (file, built_in) in expected {
        let loaded = SupportProfile::load(&asset(&format!("supports/{file}")))
            .expect("a shipped support profile is valid");
        assert_eq!(
            loaded, built_in,
            "{file} has drifted from the preset the window offers"
        );
    }
}

#[test]
fn every_shipped_printer_says_its_numbers_are_unverified() {
    let dir = asset("printers");
    let files = std::fs::read_dir(&dir).expect("the shipped catalogue is a directory");
    let mut machines = 0;

    for file in files {
        let path = file.expect("a readable directory entry").path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("a readable profile");
        assert!(
            source.contains("UNVERIFIED"),
            "{} ships numbers without saying nobody measured them",
            path.display()
        );
        machines += 1;
    }

    // The catalogue is generated from a source of about 150 machines, of which the
    // containers we write cover roughly a third; see docs/design/profiles.md.
    assert!(machines >= 40, "only {machines} machines are shipped");
}
