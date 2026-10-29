use std::path::Path;

use super::*;
use crate::catalogue::tests::{SAMPLE_PRINTER, in_its_own_dir};
use crate::{Catalogue, FrontEdge};

const LD_002H: &str = "creality-ld-002h";

fn file(dir: &Path, id: &str) -> std::path::PathBuf {
    dir.join(format!("printers/{id}.toml"))
}

#[test]
fn an_untouched_install_keeps_no_field_and_follows_the_shipped_printer() {
    in_its_own_dir("edits-untouched", |dir, mut catalogue| {
        let shipped = catalogue.shipped_printer(LD_002H).expect("shipped");
        catalogue.save_printer(LD_002H, &shipped).expect("writable");

        let text = std::fs::read_to_string(file(dir, LD_002H)).expect("written");
        let kept: Table = toml::from_str(&text).expect("TOML");
        assert_eq!(kept.keys().collect::<Vec<_>>(), [MARKER]);
        let reloaded = Catalogue::with_root(dir).expect("reads back");
        let entry = reloaded.printer(LD_002H).expect("installed");
        assert_eq!(entry.profile, shipped);
        assert!(matches!(entry.source, Source::User(_)), "still installed");
    });
}

#[test]
fn an_edit_is_kept_and_every_other_field_follows_the_build() {
    in_its_own_dir("edits-kept", |dir, mut catalogue| {
        let shipped = catalogue.shipped_printer(LD_002H).expect("shipped");
        let edited = PrinterProfile {
            build_volume: crate::BuildVolume {
                z: 150.0,
                ..shipped.build_volume
            },
            ..shipped.clone()
        };
        catalogue.save_printer(LD_002H, &edited).expect("writable");

        let text = std::fs::read_to_string(file(dir, LD_002H)).expect("written");
        let kept: Table = toml::from_str(&text).expect("TOML");
        assert_eq!(kept.keys().collect::<Vec<_>>(), ["build_volume", MARKER]);
        let reloaded = Catalogue::with_root(dir).expect("reads back");
        assert_eq!(
            reloaded.printer(LD_002H).expect("installed").profile,
            edited
        );
    });
}

#[test]
fn a_field_put_back_to_its_default_stays_there() {
    in_its_own_dir("edits-unset", |dir, mut catalogue| {
        let shipped = catalogue.shipped_printer(LD_002H).expect("shipped");
        assert_ne!(
            shipped.front_edge,
            FrontEdge::MinY,
            "the shipped one states it"
        );
        let edited = PrinterProfile {
            front_edge: FrontEdge::MinY,
            ..shipped
        };
        catalogue.save_printer(LD_002H, &edited).expect("writable");

        let reloaded = Catalogue::with_root(dir).expect("reads back");
        let entry = reloaded.printer(LD_002H).expect("installed");
        assert_eq!(entry.profile.front_edge, FrontEdge::MinY);
    });
}

#[test]
fn a_whole_copy_an_older_release_left_is_put_back_to_the_shipped_printer() {
    in_its_own_dir("edits-old-copy", |dir, catalogue| {
        let shipped = catalogue.shipped_printer(LD_002H).expect("shipped");
        let old = PrinterProfile {
            front_edge: FrontEdge::MinX,
            ..shipped.clone()
        };
        let path = file(dir, LD_002H);
        let copy = old.to_toml_string(&path).expect("a valid profile writes");
        std::fs::create_dir_all(path.parent().expect("in a folder")).expect("writable");
        std::fs::write(&path, &copy).expect("writable");

        let catalogue = Catalogue::with_root(dir).expect("reads");
        let entry = catalogue.printer(LD_002H).expect("still installed");
        assert_eq!(entry.profile, shipped);
        assert!(matches!(entry.source, Source::User(_)), "still installed");
        let aside = path.with_extension("toml.bak");
        assert_eq!(std::fs::read_to_string(aside).expect("kept aside"), copy);
        let rewritten: Table =
            toml::from_str(&std::fs::read_to_string(&path).expect("rewritten")).expect("TOML");
        assert_eq!(
            rewritten.keys().collect::<Vec<_>>(),
            [MARKER],
            "no edits kept"
        );
    });
}

#[test]
fn a_whole_copy_an_older_release_left_keeps_its_printers_on_the_network() {
    in_its_own_dir("edits-old-machines", |dir, catalogue| {
        let shipped = catalogue.shipped_printer(LD_002H).expect("shipped");
        let machines = vec![crate::NetworkMachine {
            name: "Shelf".to_owned(),
            address: "192.168.1.40".to_owned(),
            board: None,
        }];
        let old = PrinterProfile {
            front_edge: FrontEdge::MinX,
            machines: machines.clone(),
            ..shipped.clone()
        };
        let path = file(dir, LD_002H);
        std::fs::create_dir_all(path.parent().expect("in a folder")).expect("writable");
        std::fs::write(&path, old.to_toml_string(&path).expect("valid")).expect("writable");

        let entry = Catalogue::with_root(dir).expect("reads");
        let entry = entry.printer(LD_002H).expect("still installed");
        assert_eq!(entry.profile.machines, machines);
        assert_eq!(entry.profile.front_edge, shipped.front_edge);
        let reloaded = Catalogue::with_root(dir).expect("reads back");
        assert_eq!(
            reloaded
                .printer(LD_002H)
                .expect("installed")
                .profile
                .machines,
            machines,
            "kept as an edit"
        );
    });
}

#[test]
fn a_whole_copy_no_longer_valid_still_keeps_its_printers_on_the_network() {
    in_its_own_dir("edits-old-stale", |dir, catalogue| {
        let shipped = catalogue.shipped_printer(LD_002H).expect("shipped");
        let old = PrinterProfile {
            machines: vec![crate::NetworkMachine {
                name: "Shelf".to_owned(),
                address: "192.168.1.40".to_owned(),
                board: None,
            }],
            ..shipped.clone()
        };
        let path = file(dir, LD_002H);
        let mut stale: Table =
            toml::from_str(&old.to_toml_string(&path).expect("valid")).expect("TOML");
        stale.remove("display");
        std::fs::create_dir_all(path.parent().expect("in a folder")).expect("writable");
        std::fs::write(&path, toml::to_string(&stale).expect("TOML")).expect("writable");

        let catalogue = Catalogue::with_root(dir).expect("reads");
        let entry = catalogue.printer(LD_002H).expect("still installed");
        assert_eq!(entry.profile.machines, old.machines);
    });
}

#[test]
fn a_whole_copy_whose_printers_do_not_read_is_left_as_it_is() {
    in_its_own_dir("edits-old-bad-machines", |dir, _| {
        let path = file(dir, LD_002H);
        let copy = "machines = [{ address = 40 }]\n";
        std::fs::create_dir_all(path.parent().expect("in a folder")).expect("writable");
        std::fs::write(&path, copy).expect("writable");

        let catalogue = Catalogue::with_root(dir).expect("reads");
        assert!(
            catalogue
                .unreadable()
                .iter()
                .any(|bad| bad.id == LD_002H && matches!(*bad.error, ProfileError::Parse { .. })),
            "listed as unreadable"
        );
        assert_eq!(std::fs::read_to_string(&path).expect("kept"), copy);
    });
}

#[test]
fn a_printer_of_their_own_is_read_as_it_stands() {
    in_its_own_dir("edits-own", |dir, _| {
        let path = file(dir, "my-printer");
        std::fs::create_dir_all(path.parent().expect("in a folder")).expect("writable");
        std::fs::write(&path, SAMPLE_PRINTER).expect("writable");

        let catalogue = Catalogue::with_root(dir).expect("reads");
        assert_eq!(
            catalogue
                .printer("my-printer")
                .expect("theirs")
                .profile
                .name,
            "Mine"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("kept"),
            SAMPLE_PRINTER
        );
        assert!(!path.with_extension("toml.bak").exists());
    });
}

#[test]
fn edits_over_a_printer_no_longer_shipped_are_left_out() {
    in_its_own_dir("edits-orphan", |dir, _| {
        let path = file(dir, "gone-printer");
        std::fs::create_dir_all(path.parent().expect("in a folder")).expect("writable");
        std::fs::write(&path, format!("{MARKER} = true\n")).expect("writable");

        let catalogue = Catalogue::with_root(dir).expect("cannot fail the catalogue");
        let [left_out] = catalogue.unreadable() else {
            panic!("one file left out, got {:?}", catalogue.unreadable());
        };
        assert!(matches!(
            *left_out.error,
            ProfileError::EditsOverNothing { .. }
        ));
    });
}
