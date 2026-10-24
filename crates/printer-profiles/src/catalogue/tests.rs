use super::*;
use crate::PrinterTuning;

#[test]
fn a_broken_preset_is_left_out_and_the_rest_of_the_catalogue_reads() {
    in_its_own_dir("broken-preset", |dir, _| {
        let drain = dir.join("drain");
        std::fs::create_dir_all(&drain).expect("writable");
        std::fs::write(drain.join("broken.toml"), "name = ").expect("writable");

        let catalogue = Catalogue::with_root(dir).expect("a preset cannot fail the catalogue");
        assert!(catalogue.printer("elegoo-mars-4-ultra").is_ok());
        assert_eq!(catalogue.presets(Kind::Drain).count(), 0);
        assert_eq!(catalogue.unreadable(), [drain.join("broken.toml")]);
    });
}

#[test]
fn a_hollow_preset_is_kept_beside_the_profiles_and_forgotten() {
    in_its_own_dir("hollow-preset", |dir, mut catalogue| {
        let preset = Preset {
            name: "Hive 2 mm".to_owned(),
            settings: toml::Table::from_iter([("thickness_mm".to_owned(), 2.0.into())]),
        };
        let path = catalogue
            .save_preset(Kind::Hollow, "hive-2-mm", &preset)
            .expect("writable");
        assert_eq!(path, dir.join("hollow/hive-2-mm.toml"));
        let reloaded = Catalogue::with_root(dir).expect("the directory reads back");
        let kept: Vec<&str> = reloaded
            .presets(Kind::Hollow)
            .map(|entry| entry.profile.name.as_str())
            .collect();
        assert_eq!(kept, ["Hive 2 mm"]);
        assert_eq!(
            reloaded.presets(Kind::Drain).count(),
            0,
            "kept apart by tool"
        );

        catalogue
            .forget_user_copy(Kind::Hollow, "hive-2-mm")
            .expect("removable");
        assert_eq!(
            catalogue.presets(Kind::Hollow).count(),
            0,
            "nothing shipped to fall back on"
        );
    });
}

#[test]
fn only_a_tools_kind_takes_a_preset() {
    let mut catalogue = Catalogue::bundled().expect("shipped");
    let preset = Preset {
        name: "x".to_owned(),
        settings: toml::Table::new(),
    };
    let refused = catalogue.save_preset(Kind::Printer, "x", &preset);
    assert!(matches!(refused, Err(ProfileError::Unknown { .. })));
}

#[test]
fn every_bundled_profile_parses() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");

    assert!(
        catalogue.printers().count() >= 5,
        "the catalogue ships at least five machines"
    );
    assert!(
        catalogue
            .printers()
            .all(|entry| entry.source == Source::Bundled)
    );
}

#[test]
fn every_shipped_automatic_profile_names_shipped_shapes() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    assert!(catalogue.autos().count() >= 1, "an automatic profile ships");
    for auto in catalogue.autos() {
        for shape in auto.profile.every_shape() {
            assert!(
                catalogue.is_shipped(Kind::Support, shape),
                "{} names {shape}, which does not ship",
                auto.id
            );
            assert!(catalogue.is_auto_shape(shape));
        }
    }
    assert!(
        !catalogue.is_auto_shape("light"),
        "a profile chosen by hand is not one"
    );
}

/// An exposure is measured on the machine in the room, and an invented one costs a
/// tank of resin, so the catalogue ships none; see ADR 0196.
#[test]
fn the_catalogue_ships_no_resin() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    assert_eq!(catalogue.resins().count(), 0);
    assert!(!catalogue.is_shipped(Kind::Resin, "standard-grey"));
}

#[test]
fn the_catalogue_covers_three_manufacturers() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");

    let makers: std::collections::BTreeSet<_> = catalogue
        .printers()
        .map(|entry| entry.profile.manufacturer.clone())
        .collect();
    assert!(makers.len() >= 3, "{makers:?} is not three manufacturers");
}

#[test]
fn a_user_profile_overrides_the_bundled_id() {
    let dir = std::env::temp_dir().join("encrust-catalogue-override");
    let printers = dir.join("printers");
    std::fs::create_dir_all(&printers).expect("a writable temporary directory");
    let path = printers.join("elegoo-mars-4-ultra.toml");
    std::fs::write(
        &path,
        r#"
name = "Mine"
manufacturer = "Elegoo"

[display]
width_px = 100
height_px = 50
width_mm = 10.0
height_mm = 5.0

[build_volume]
x = 10.0
y = 5.0
z = 100.0
"#,
    )
    .expect("a writable file");

    let mut catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    catalogue.overlay(&dir).expect("the directory is readable");

    let entry = catalogue
        .printer("elegoo-mars-4-ultra")
        .expect("still there");
    assert_eq!(entry.profile.name, "Mine");
    assert_eq!(entry.source, Source::User(path.clone()));

    std::fs::remove_dir_all(&dir).expect("the temporary directory goes away");
}

#[test]
fn a_missing_user_directory_leaves_the_catalogue_alone() {
    let mut catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let before = catalogue.printers().count();
    catalogue
        .overlay(Path::new("/nonexistent/encrust/profiles"))
        .expect("a missing directory is not an error");
    assert_eq!(catalogue.printers().count(), before);
}

#[test]
fn an_unknown_id_is_reported_as_unknown() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    let error = catalogue.printer("no-such-machine").unwrap_err();
    assert!(matches!(
        error,
        ProfileError::Unknown {
            kind: "printer",
            ..
        }
    ));
}

#[test]
fn a_resin_comes_out_carrying_the_printers_numbers() {
    in_its_own_dir("resolve-resin", |_, mut catalogue| {
        let mut resin = MaterialProfile {
            exposure_s: 2.6,
            ..MaterialProfile::default()
        };
        resin.printers.insert(
            MARS.to_owned(),
            PrinterTuning {
                exposure_s: Some(3.2),
                ..PrinterTuning::default()
            },
        );
        catalogue.save_resin("mine", &resin).expect("writable");

        let tuned = catalogue.resin_for("mine", MARS).expect("saved");
        assert!(
            (tuned.exposure_s - 3.2).abs() < 1e-6,
            "the machine's own measured exposure"
        );
        assert!(tuned.printers.is_empty(), "a resolved resin carries no map");
    });
}

const MARS: &str = "elegoo-mars-3-pro";

/// A catalogue writing into a directory of this test's own, so nothing reads or
/// writes the profile directory the user actually keeps.
fn in_its_own_dir(name: &str, body: impl FnOnce(&Path, Catalogue)) {
    let dir = std::env::temp_dir().join(format!("encrust-catalogue-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    let catalogue = Catalogue::with_root(&dir).expect("the shipped catalogue is valid");
    body(&dir, catalogue);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edited_printer_is_saved_over_the_shipped_one_and_can_be_thrown_away() {
    in_its_own_dir("edit-printer", |dir, mut catalogue| {
        let mut edited = catalogue
            .printer("elegoo-mars-4-ultra")
            .expect("shipped")
            .profile
            .clone();
        edited.build_volume.z = 199.0;

        let path = catalogue
            .save_printer("elegoo-mars-4-ultra", &edited)
            .expect("the profile directory is writable");
        assert_eq!(path, dir.join("printers/elegoo-mars-4-ultra.toml"));

        let entry = catalogue
            .printer("elegoo-mars-4-ultra")
            .expect("still there");
        assert!((entry.profile.build_volume.z - 199.0).abs() < f32::EPSILON);
        assert_eq!(entry.source, Source::User(path.clone()));

        // A catalogue built fresh over the same directory reads the file back.
        let reloaded = Catalogue::with_root(dir).expect("the directory is readable");
        let entry = reloaded
            .printer("elegoo-mars-4-ultra")
            .expect("still there");
        assert!((entry.profile.build_volume.z - 199.0).abs() < f32::EPSILON);

        catalogue
            .forget_user_copy(Kind::Printer, "elegoo-mars-4-ultra")
            .expect("the file goes away");
        let entry = catalogue
            .printer("elegoo-mars-4-ultra")
            .expect("shipped again");
        assert_eq!(entry.source, Source::Bundled);
        assert!(!path.exists());
    });
}

#[test]
fn a_resin_the_user_invented_leaves_the_catalogue_when_it_is_thrown_away() {
    in_its_own_dir("invented-resin", |_, mut catalogue| {
        let resin = MaterialProfile {
            name: "Mine".to_owned(),
            ..MaterialProfile::default()
        };

        catalogue.save_resin("my-resin", &resin).expect("writable");
        assert!(catalogue.resin("my-resin").is_ok());

        catalogue
            .forget_user_copy(Kind::Resin, "my-resin")
            .expect("the file goes away");
        assert!(
            catalogue.resin("my-resin").is_err(),
            "nothing was shipped under that id, so it is gone"
        );
    });
}

#[test]
fn a_support_profile_is_saved_over_the_shipped_one_and_can_be_thrown_away() {
    in_its_own_dir("edit-support", |dir, mut catalogue| {
        let shipped = catalogue
            .support("medium")
            .expect("shipped")
            .profile
            .clone();
        assert_eq!(
            shipped,
            SupportProfile::shipped("medium").expect("the medium preset ships"),
            "the asset is the preset"
        );
        let edited = SupportProfile {
            density: 3.0,
            ..shipped.clone()
        };

        let path = catalogue.save_support("medium", &edited).expect("writable");
        assert_eq!(path, dir.join("supports/medium.toml"));
        let reloaded = Catalogue::with_root(dir).expect("the directory is readable");
        assert_eq!(reloaded.support("medium").expect("there").profile, edited);

        catalogue
            .forget_user_copy(Kind::Support, "medium")
            .expect("the file goes away");
        assert_eq!(
            catalogue.support("medium").expect("shipped").profile,
            shipped
        );
    });
}

#[test]
fn an_automatic_profile_is_saved_over_the_shipped_one_and_can_be_thrown_away() {
    in_its_own_dir("edit-auto", |dir, mut catalogue| {
        let entry = catalogue
            .autos()
            .next()
            .expect("an automatic profile ships");
        let (id, shipped) = (entry.id.clone(), entry.profile.clone());
        let edited = AutoProfile {
            max_overhang_deg: shipped.max_overhang_deg + 10.0,
            ..shipped.clone()
        };

        let path = catalogue.save_auto(&id, &edited).expect("writable");
        assert_eq!(path, dir.join(format!("auto/{id}.toml")));
        let reloaded = Catalogue::with_root(dir).expect("the directory is readable");
        let kept = reloaded
            .autos()
            .find(|entry| entry.id == id)
            .expect("there");
        assert_eq!(
            kept.profile, edited,
            "the copy is read back over the shipped one"
        );

        catalogue
            .forget_user_copy(Kind::Auto, &id)
            .expect("the file goes away");
        let back = catalogue
            .autos()
            .find(|entry| entry.id == id)
            .expect("shipped");
        assert_eq!(back.profile, shipped);
    });
}

#[test]
fn no_automatic_profile_can_be_added() {
    in_its_own_dir("new-auto", |dir, mut catalogue| {
        let shipped = catalogue.autos().next().expect("ships").profile.clone();
        let refused = catalogue.save_auto("mine", &shipped);
        assert!(
            matches!(refused, Err(ProfileError::Unknown { .. })),
            "an id that does not ship is no automatic profile"
        );

        let stray = dir.join("auto/mine.toml");
        std::fs::create_dir_all(dir.join("auto")).expect("writable");
        let text = shipped.to_toml_string(&stray).expect("serialises");
        std::fs::write(&stray, text).expect("writable");
        let reloaded = Catalogue::with_root(dir).expect("the directory is readable");
        assert!(
            reloaded.autos().all(|entry| entry.id != "mine"),
            "a file under an id that does not ship is not taken up"
        );
    });
}

#[test]
fn only_a_shipped_id_is_shipped() {
    let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    assert!(catalogue.is_shipped(Kind::Printer, "elegoo-mars-4-ultra"));
    assert!(!catalogue.is_shipped(Kind::Printer, "my-printer"));
    assert!(catalogue.is_shipped(Kind::Support, "medium"));
}

/// What P-11 costs the user: a copy made by an older release stands over the shipped
/// profile, and nothing in the file says so.
#[test]
fn the_shipped_printer_is_still_reachable_under_an_edited_id() {
    in_its_own_dir("shipped-printer", |_, mut catalogue| {
        let shipped = catalogue
            .shipped_printer("elegoo-saturn-4-ultra")
            .expect("a shipped machine");
        // The one thing ADR 0142 forbids on a tilting vat, which is what a copy made
        // before that decision keeps.
        let edited = PrinterProfile {
            firmware: crate::Firmware {
                per_layer_settings: true,
                ..shipped.firmware
            },
            ..shipped.clone()
        };
        catalogue
            .save_printer("elegoo-saturn-4-ultra", &edited)
            .expect("writable");

        assert_eq!(
            catalogue.shipped_printer("elegoo-saturn-4-ultra"),
            Some(shipped),
            "the user's copy does not hide what the build ships"
        );
        assert!(catalogue.shipped_printer("my-printer").is_none());
    });
}

#[test]
fn a_copy_saved_before_a_lock_still_takes_the_shipped_one() {
    in_its_own_dir("copy-before-lock", |dir, mut catalogue| {
        let shipped = catalogue
            .shipped_printer("elegoo-saturn-4-ultra")
            .expect("a shipped machine");
        let old = PrinterProfile {
            locked: PrinterTuning::default(),
            ..shipped.clone()
        };
        catalogue
            .save_printer("elegoo-saturn-4-ultra", &old)
            .expect("writable");
        let written = catalogue
            .printer("elegoo-saturn-4-ultra")
            .expect("saved")
            .profile
            .locked
            .clone();
        assert_eq!(written, shipped.locked, "a save keeps the machine's locks");
        let file = dir.join("printers/elegoo-saturn-4-ultra.toml");
        let on_disk = std::fs::read_to_string(&file).expect("the save wrote it");
        let on_disk = PrinterProfile::from_toml_str(&on_disk, &file).expect("and it reads");
        assert_eq!(on_disk.locked, shipped.locked, "and so does the file");

        std::fs::write(
            dir.join("printers/elegoo-saturn-4-ultra.toml"),
            old.to_toml_string(dir).expect("a valid profile writes"),
        )
        .expect("writable");
        let reloaded = Catalogue::with_root(dir).expect("the directory is readable");
        let entry = reloaded.printer("elegoo-saturn-4-ultra").expect("the copy");
        assert!(matches!(entry.source, Source::User(_)), "the copy stands");
        assert_eq!(
            entry.profile.locked, shipped.locked,
            "with the shipped locks"
        );
    });
}

#[test]
fn a_name_that_would_not_be_a_file_stem_is_refused() {
    in_its_own_dir("bad-id", |_, mut catalogue| {
        let profile =
            PrinterProfile::from_toml_str(SAMPLE_PRINTER, Path::new("inline.toml")).expect("valid");
        let error = catalogue.save_printer("../escape", &profile).unwrap_err();
        assert!(matches!(error, ProfileError::BadId { .. }));
    });
}

#[test]
fn a_profile_that_would_not_load_is_never_written() {
    in_its_own_dir("invalid-save", |dir, mut catalogue| {
        let mut broken =
            PrinterProfile::from_toml_str(SAMPLE_PRINTER, Path::new("inline.toml")).expect("valid");
        broken.display.width_mm = 0.0;

        let error = catalogue.save_printer("my-printer", &broken).unwrap_err();
        assert!(matches!(error, ProfileError::NonPositive { .. }));
        assert!(!dir.join("printers/my-printer.toml").exists());
    });
}

#[test]
fn a_bundled_catalogue_has_nowhere_to_save() {
    let mut catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    assert!(!catalogue.has_store());
    let error = catalogue
        .save_resin("my-resin", &MaterialProfile::default())
        .unwrap_err();
    assert!(matches!(error, ProfileError::NoUserDir { .. }));
}

/// A store with no directory behind it, as a browser's is.
#[derive(Debug, Default)]
struct Kept(std::sync::Mutex<BTreeMap<(&'static str, String), String>>);

impl ProfileStore for Kept {
    fn path_of(&self, kind: Kind, id: &str) -> PathBuf {
        PathBuf::from(format!("kept/{}/{id}.toml", kind.dir()))
    }

    fn read_all(&self, kind: Kind) -> Result<Vec<(String, PathBuf, String)>, ProfileError> {
        let kept = self.0.lock().expect("no test panics holding it");
        Ok(kept
            .iter()
            .filter(|((dir, _), _)| *dir == kind.dir())
            .map(|((_, id), toml)| (id.clone(), self.path_of(kind, id), toml.clone()))
            .collect())
    }

    fn write(&self, kind: Kind, id: &str, toml: &str) -> Result<(), ProfileError> {
        let mut kept = self.0.lock().expect("no test panics holding it");
        kept.insert((kind.dir(), id.to_owned()), toml.to_owned());
        Ok(())
    }

    fn remove(&self, kind: Kind, id: &str) -> Result<(), ProfileError> {
        let mut kept = self.0.lock().expect("no test panics holding it");
        kept.remove(&(kind.dir(), id.to_owned()));
        Ok(())
    }
}

#[test]
fn a_store_with_no_directory_keeps_what_is_saved_into_it() {
    let store = Arc::new(Kept::default());
    let mut catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");
    catalogue
        .overlay_store(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .expect("an empty store reads");
    let resin = MaterialProfile {
        name: "Mine".to_owned(),
        ..MaterialProfile::default()
    };
    let path = catalogue
        .save_resin("my-resin", &resin)
        .expect("the store takes it");
    assert_eq!(path, PathBuf::from("kept/resins/my-resin.toml"));

    let mut reloaded = Catalogue::bundled().expect("the shipped catalogue is valid");
    reloaded
        .overlay_store(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .expect("the store reads back");
    let entry = reloaded.resin("my-resin").expect("the saved resin is back");
    assert_eq!(entry.profile.name, "Mine");
    assert_eq!(entry.source, Source::User(path));

    reloaded
        .forget_user_copy(Kind::Resin, "my-resin")
        .expect("the store lets it go");
    assert!(reloaded.resin("my-resin").is_err(), "only the user had it");
}

const SAMPLE_PRINTER: &str = r#"
name = "Mine"
manufacturer = "Me"

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
