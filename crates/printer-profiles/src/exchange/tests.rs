use serde::{Deserialize, Serialize};

use super::*;
use crate::Source;

const MARS: &str = "elegoo-mars-3-pro";
const SATURN: &str = "elegoo-saturn-4-ultra";

/// The shipped catalogue over a directory of its own, which every write lands in.
fn writable(name: &str) -> Catalogue {
    let dir = std::env::temp_dir().join(format!("encrust-exchange-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Catalogue::with_root(&dir).expect("a missing directory reads as empty")
}

#[test]
fn a_name_makes_a_file_stem() {
    assert_eq!(slug("Saturn 4 Ultra"), "saturn-4-ultra");
    assert_eq!(slug("  ABS-like (tough)! "), "abs-like-tough");
    assert_eq!(unique_id("Смола", |_| false), "profile");
    assert_eq!(
        unique_id("Grey", |id| id == "grey" || id == "grey-2"),
        "grey-3"
    );
}

/// A farm of one model set up once is handed to another desk with its printers named.
#[test]
fn a_printer_carries_the_machines_it_sends_to_out_and_back_in() {
    let mut catalogue = writable("machines");
    let mut saturn = catalogue
        .printer(SATURN)
        .expect("the Saturn ships")
        .profile
        .clone();
    saturn.connection = crate::Connection::Sdcp;
    saturn.machines = vec![crate::NetworkMachine {
        name: "Bench A".to_owned(),
        address: "192.168.1.133".to_owned(),
        board: Some("d85f46e4d7d90100".to_owned()),
    }];
    catalogue.save_printer(SATURN, &saturn).expect("writable");
    let text = catalogue
        .export(Kind::Printer, SATURN)
        .expect("the Saturn is there");

    let mut elsewhere = writable("machines-elsewhere");
    let id = elsewhere
        .import(Kind::Printer, &text, Path::new("saturn.toml"), None)
        .expect("the file is the profile");
    let imported = &elsewhere.printer(&id).expect("imported").profile;
    assert_eq!(imported.machines, saturn.machines);
    assert_eq!(imported.connection, crate::Connection::Sdcp);
}

#[test]
fn a_support_profile_comes_back_as_a_copy_beside_itself() {
    let mut catalogue = writable("support");
    let text = catalogue
        .export(Kind::Support, "heavy")
        .expect("heavy ships");
    let id = catalogue
        .import(Kind::Support, &text, Path::new("heavy.toml"), None)
        .expect("the file is the profile");

    assert_ne!(id, "heavy", "an import never writes over what is there");
    let original = &catalogue.support("heavy").expect("still there").profile;
    let copy = &catalogue.support(&id).expect("imported").profile;
    assert_eq!(copy.name, format!("{} (imported)", original.name));
    assert_eq!(copy.tip, original.tip, "every number travels");
}

#[test]
fn a_machine_from_another_directory_keeps_its_name() {
    let mut from = writable("printer-from");
    let mut printer = from.printer(MARS).expect("ships").profile.clone();
    printer.name = "Bench Mars".to_owned();
    from.save_printer("bench-mars", &printer).expect("writable");
    let text = from
        .export(Kind::Printer, "bench-mars")
        .expect("just saved");

    let mut to = writable("printer-to");
    let id = to
        .import(Kind::Printer, &text, Path::new("bench.toml"), None)
        .expect("the file is a printer");
    let entry = to.printer(&id).expect("imported");
    assert_eq!(
        entry.profile.name, "Bench Mars",
        "no machine of that name was there"
    );
    assert!(
        matches!(entry.source, Source::User(_)),
        "an imported machine is one of the user's"
    );
}

#[test]
fn a_resin_is_set_up_on_the_machine_it_was_imported_on() {
    let mut from = writable("resin-from");
    let mut resin = MaterialProfile {
        name: "Bench grey".to_owned(),
        ..MaterialProfile::default()
    };
    let tuning = PrinterTuning {
        exposure_s: Some(3.1),
        ..PrinterTuning::default()
    };
    resin.printers.insert(MARS.to_owned(), tuning);
    resin.last_printer = Some(MARS.to_owned());
    from.save_resin("bench-grey", &resin).expect("writable");
    let text = from.export(Kind::Resin, "bench-grey").expect("just saved");

    let mut to = writable("resin-to");
    let id = to
        .import(Kind::Resin, &text, Path::new("grey.toml"), Some(SATURN))
        .expect("the file is a resin");
    let back = &to.resin(&id).expect("imported").profile;
    assert!(
        back.is_tuned_for(SATURN),
        "it is on the machine it came in on"
    );
    assert!(
        back.is_tuned_for(MARS),
        "and keeps what was measured elsewhere"
    );
    assert!(
        (back.starting_point(SATURN).exposure_s - 3.1).abs() < 1e-6,
        "starting from the numbers of the machine it was last tuned on"
    );
}

#[derive(Serialize, Deserialize)]
struct Hole {
    diameter_mm: f32,
}

#[derive(Deserialize)]
struct Wall {
    #[expect(dead_code, reason = "only the shape is checked")]
    thickness_mm: f32,
}

#[test]
fn a_preset_travels_and_is_refused_as_another_tools() {
    let mut catalogue = writable("presets");
    let preset = Preset::of("Wide", &Hole { diameter_mm: 3.5 }, Path::new("wide"))
        .expect("the settings are a table");
    catalogue
        .save_preset(Kind::Drain, "wide", &preset)
        .expect("writable");
    let text = catalogue.export(Kind::Drain, "wide").expect("just kept");

    let again = catalogue
        .import_preset::<Hole>(Kind::Drain, &text, Path::new("wide.toml"))
        .expect("the file is a drain preset");
    let entry = catalogue
        .presets(Kind::Drain)
        .find(|entry| entry.id == again)
        .expect("imported");
    assert_eq!(entry.profile.name, "Wide (imported)");

    let refused = catalogue.import_preset::<Wall>(Kind::Hollow, &text, Path::new("wide.toml"));
    assert!(
        matches!(refused, Err(ProfileError::Parse { .. })),
        "a hole is not a wall: {refused:?}"
    );
}

#[test]
fn an_import_never_writes_over_a_users_file_that_would_not_read() {
    let dir = std::env::temp_dir().join(format!("encrust-exchange-broken-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let broken = dir.join("supports/heavy-imported.toml");
    std::fs::create_dir_all(broken.parent().expect("a file in a folder")).expect("writable");
    std::fs::write(&broken, "name = ").expect("writable");
    let mut catalogue = Catalogue::with_root(&dir).expect("a broken file cannot fail it");
    let text = catalogue
        .export(Kind::Support, "heavy")
        .expect("heavy ships");

    let id = catalogue
        .import(Kind::Support, &text, Path::new("heavy.toml"), None)
        .expect("the file is the profile");
    assert_ne!(id, "heavy-imported", "the broken file's id is taken");
    assert_eq!(
        std::fs::read_to_string(&broken).expect("still there"),
        "name = ",
        "the user's file is untouched"
    );
}

#[test]
fn a_file_imported_twice_comes_in_under_two_names() {
    let mut catalogue = writable("support-twice");
    let text = catalogue
        .export(Kind::Support, "heavy")
        .expect("heavy ships");
    let name = |catalogue: &Catalogue, id: &str| {
        catalogue
            .support(id)
            .expect("imported")
            .profile
            .name
            .clone()
    };
    let first = catalogue
        .import(Kind::Support, &text, Path::new("heavy.toml"), None)
        .expect("the file is the profile");
    let second = catalogue
        .import(Kind::Support, &text, Path::new("heavy.toml"), None)
        .expect("and still is");
    assert_ne!(
        name(&catalogue, &first),
        name(&catalogue, &second),
        "two copies the list cannot tell apart"
    );
}

#[test]
fn a_preset_is_not_taken_in_as_a_profile() {
    let mut catalogue = writable("preset-as-profile");
    let refused = catalogue.import(Kind::Hollow, "name = 'x'", Path::new("x.toml"), None);
    assert!(matches!(refused, Err(ProfileError::Unknown { .. })));
}

#[test]
fn a_file_that_is_not_a_profile_is_refused() {
    let mut catalogue = writable("garbage");
    let refused = catalogue.import(Kind::Support, "name = ", Path::new("broken.toml"), None);
    assert!(matches!(refused, Err(ProfileError::Parse { .. })));
}

#[test]
fn exporting_what_is_not_there_names_it() {
    let catalogue = writable("missing");
    let missing = catalogue.export(Kind::Hollow, "nothing");
    assert!(matches!(missing, Err(ProfileError::Unknown { .. })));
}

#[test]
fn a_file_naming_its_machine_picks_it_among_the_machines_of_its_panel() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    // Five shipped machines share the Saturn 4 Ultra's 11520 x 5120 panel.
    assert_eq!(catalogue.printer_for_panel(11_520, 5_120, None), None);
    for named in ["Elegoo Saturn 4 Ultra", "ELEGOO  saturn 4 ULTRA"] {
        assert_eq!(
            catalogue.printer_for_panel(11_520, 5_120, Some(named)),
            Some(SATURN),
            "{named} is the Saturn by its label or the name its firmware matches"
        );
    }
    assert_eq!(
        catalogue.printer_for_panel(5_120, 11_520, Some("ELEGOO Saturn 4 Ultra")),
        Some(SATURN),
        "a panel stated portrait is the same panel"
    );
}

#[test]
fn the_one_machine_of_a_panel_is_picked_whatever_the_file_calls_it() {
    let mut catalogue = writable("panel");
    let mut odd = catalogue.printer(MARS).expect("ships").profile.clone();
    odd.display.width_px = 17;
    odd.display.height_px = 9;
    catalogue.save_printer("odd", &odd).expect("writable");
    assert_eq!(
        catalogue.printer_for_panel(17, 9, Some("Unknown machine")),
        Some("odd")
    );
}

#[test]
fn a_resin_taken_from_a_file_is_set_up_on_its_machine_beside_the_rest() {
    let mut catalogue = writable("from-file");
    let resin = MaterialProfile {
        name: "Grey".to_owned(),
        exposure_s: 2.37,
        ..MaterialProfile::default()
    };
    let first = catalogue
        .import_resin(resin.clone(), Some(SATURN))
        .expect("writable");
    let again = catalogue
        .import_resin(resin, Some(SATURN))
        .expect("writable");

    assert_ne!(first, again, "an import never writes over what is there");
    let kept = &catalogue.resin(&again).expect("imported").profile;
    assert_eq!(kept.name, "Grey (imported)");
    assert!(kept.is_tuned_for(SATURN));
    assert_eq!(kept.for_printer(SATURN).exposure_s, 2.37);
}
