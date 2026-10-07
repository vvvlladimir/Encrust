//! The catalogue of shipped printers, resins and support profiles, and the user directory that overrides it
//! by id. See docs/decisions/0049.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{
    DirStore, MaterialProfile, PrinterProfile, ProfileError, ProfileStore, SupportProfile,
};

include!(concat!(env!("OUT_DIR"), "/bundled_profiles.rs"));

/// Environment variable holding a profile directory, used in place of the platform one.
pub const PROFILE_DIR_VAR: &str = "ENCRUST_PROFILE_DIR";

/// Which half of the catalogue a profile belongs to, and the directory it lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Printer,
    Resin,
    Support,
}

impl Kind {
    pub fn dir(self) -> &'static str {
        match self {
            Self::Printer => "printers",
            Self::Resin => "resins",
            Self::Support => "supports",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Printer => "printer",
            Self::Resin => "resin",
            Self::Support => "support profile",
        }
    }
}

/// Where a catalogue profile came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Shipped inside the binary.
    Bundled,
    /// Read from the user's profile directory, which overrides a bundled id.
    User(PathBuf),
}

/// One catalogue profile under the id it is addressed by, which is its file stem.
#[derive(Debug, Clone)]
pub struct Entry<T> {
    pub id: String,
    pub profile: T,
    pub source: Source,
}

/// Every printer, resin and support profile this build knows about.
#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    printers: BTreeMap<String, Entry<PrinterProfile>>,
    resins: BTreeMap<String, Entry<MaterialProfile>>,
    supports: BTreeMap<String, Entry<SupportProfile>>,
    /// Where an edited profile is written. `None` until a store is laid over the
    /// catalogue, which is what makes a bundled-only catalogue read-only.
    store: Option<Arc<dyn ProfileStore>>,
}

impl Catalogue {
    /// Only what is shipped in the binary.
    pub fn bundled() -> Result<Self, ProfileError> {
        Ok(Self {
            printers: bundled_table(BUNDLED_PRINTERS, "printers", PrinterProfile::from_toml_str)?,
            resins: bundled_table(BUNDLED_RESINS, "resins", MaterialProfile::from_toml_str)?,
            supports: bundled_table(BUNDLED_SUPPORTS, "supports", SupportProfile::from_toml_str)?,
            store: None,
        })
    }

    /// The shipped profiles, with the user's directory laid over them by id. That
    /// directory is also where an edited profile is written.
    pub fn load() -> Result<Self, ProfileError> {
        let mut catalogue = Self::bundled()?;
        if let Some(dir) = user_dir() {
            catalogue.overlay(&dir)?;
        }
        Ok(catalogue)
    }

    /// The shipped profiles with one named directory over them, whatever the platform
    /// and the environment say. This is what a test uses.
    pub fn with_root(dir: &Path) -> Result<Self, ProfileError> {
        let mut catalogue = Self::bundled()?;
        catalogue.overlay(dir)?;
        Ok(catalogue)
    }

    /// Reads `<dir>/printers`, `<dir>/resins` and `<dir>/supports` over what is already
    /// loaded. A missing directory is not an error: most users have never made one.
    pub fn overlay(&mut self, dir: &Path) -> Result<(), ProfileError> {
        self.overlay_store(Arc::new(DirStore::new(dir)))
    }

    /// Lays what `store` keeps over what is already loaded, by id, and writes edits there.
    pub fn overlay_store(&mut self, store: Arc<dyn ProfileStore>) -> Result<(), ProfileError> {
        overlay(
            &mut self.printers,
            &*store,
            Kind::Printer,
            PrinterProfile::from_toml_str,
        )?;
        overlay(
            &mut self.resins,
            &*store,
            Kind::Resin,
            MaterialProfile::from_toml_str,
        )?;
        overlay(
            &mut self.supports,
            &*store,
            Kind::Support,
            SupportProfile::from_toml_str,
        )?;
        self.store = Some(store);
        Ok(())
    }

    /// Whether edits have somewhere to be written.
    pub fn has_store(&self) -> bool {
        self.store.is_some()
    }

    pub fn printers(&self) -> impl Iterator<Item = &Entry<PrinterProfile>> {
        self.printers.values()
    }

    pub fn resins(&self) -> impl Iterator<Item = &Entry<MaterialProfile>> {
        self.resins.values()
    }

    pub fn supports(&self) -> impl Iterator<Item = &Entry<SupportProfile>> {
        self.supports.values()
    }

    pub fn support(&self, id: &str) -> Result<&Entry<SupportProfile>, ProfileError> {
        self.supports.get(id).ok_or_else(|| ProfileError::Unknown {
            kind: "support profile",
            id: id.to_owned(),
        })
    }

    pub fn printer(&self, id: &str) -> Result<&Entry<PrinterProfile>, ProfileError> {
        self.printers.get(id).ok_or_else(|| ProfileError::Unknown {
            kind: "printer",
            id: id.to_owned(),
        })
    }

    pub fn resin(&self, id: &str) -> Result<&Entry<MaterialProfile>, ProfileError> {
        self.resins.get(id).ok_or_else(|| ProfileError::Unknown {
            kind: "resin",
            id: id.to_owned(),
        })
    }

    /// One resin as one printer needs it: its tuning, or the last tuned printer's.
    pub fn resin_for(
        &self,
        resin_id: &str,
        printer_id: &str,
    ) -> Result<MaterialProfile, ProfileError> {
        Ok(self.resin(resin_id)?.profile.starting_point(printer_id))
    }

    /// Whether a profile of this kind was shipped under `id`, which is what the user's
    /// copy of it falls back to when thrown away.
    pub fn is_shipped(&self, kind: Kind, id: &str) -> bool {
        let table = match kind {
            Kind::Printer => BUNDLED_PRINTERS,
            Kind::Resin => BUNDLED_RESINS,
            Kind::Support => BUNDLED_SUPPORTS,
        };
        table.iter().any(|(name, _)| *name == id)
    }

    /// The printer this build ships under `id`, whatever the user's copy of it says. It
    /// is what tells a copy made by an older release from the profile beside it, and what
    /// *Restore* writes back; see `docs/decisions/0196`.
    pub fn shipped_printer(&self, id: &str) -> Option<PrinterProfile> {
        let (_, source) = BUNDLED_PRINTERS.iter().find(|(name, _)| *name == id)?;
        let path = PathBuf::from(format!("<bundled>/printers/{id}.toml"));
        PrinterProfile::from_toml_str(source, &path).ok()
    }

    /// The store a profile of `id` is written to, once the id is known to be usable.
    fn store_for(&self, id: &str) -> Result<&dyn ProfileStore, ProfileError> {
        if !is_valid_id(id) {
            return Err(ProfileError::BadId { id: id.to_owned() });
        }
        self.store.as_deref().ok_or(ProfileError::NoUserDir {
            variable: PROFILE_DIR_VAR,
        })
    }

    /// Writes `toml` as the profile of `kind` and `id`, and says where it went.
    fn keep(
        &self,
        kind: Kind,
        id: &str,
        toml: impl FnOnce(&Path) -> Result<String, ProfileError>,
    ) -> Result<PathBuf, ProfileError> {
        let store = self.store_for(id)?;
        let path = store.path_of(kind, id);
        store.write(kind, id, &toml(&path)?)?;
        Ok(path)
    }

    /// Writes a printer into the user's directory and takes it into the catalogue, where
    /// it replaces the shipped machine of the same id.
    pub fn save_printer(
        &mut self,
        id: &str,
        profile: &PrinterProfile,
    ) -> Result<PathBuf, ProfileError> {
        let path = self.keep(Kind::Printer, id, |path| profile.to_toml_string(path))?;
        self.printers.insert(
            id.to_owned(),
            Entry {
                id: id.to_owned(),
                profile: profile.clone(),
                source: Source::User(path.clone()),
            },
        );
        Ok(path)
    }

    /// Writes a resin into the user's directory, tuning tables and all.
    pub fn save_resin(
        &mut self,
        id: &str,
        resin: &MaterialProfile,
    ) -> Result<PathBuf, ProfileError> {
        let path = self.keep(Kind::Resin, id, |path| resin.to_toml_string(path))?;
        self.resins.insert(
            id.to_owned(),
            Entry {
                id: id.to_owned(),
                profile: resin.clone(),
                source: Source::User(path.clone()),
            },
        );
        Ok(path)
    }

    /// Writes a support profile into the user's directory.
    pub fn save_support(
        &mut self,
        id: &str,
        profile: &SupportProfile,
    ) -> Result<PathBuf, ProfileError> {
        let path = self.keep(Kind::Support, id, |path| profile.to_toml_string(path))?;
        self.supports.insert(
            id.to_owned(),
            Entry {
                id: id.to_owned(),
                profile: profile.clone(),
                source: Source::User(path.clone()),
            },
        );
        Ok(path)
    }

    /// Throws away the user's copy of a profile. What was shipped under that id comes
    /// back; an id that was only ever theirs leaves the catalogue.
    pub fn forget_user_copy(&mut self, kind: Kind, id: &str) -> Result<(), ProfileError> {
        self.store_for(id)?.remove(kind, id)?;
        match kind {
            Kind::Printer => restore_bundled(
                &mut self.printers,
                BUNDLED_PRINTERS,
                "printers",
                id,
                PrinterProfile::from_toml_str,
            ),
            Kind::Resin => restore_bundled(
                &mut self.resins,
                BUNDLED_RESINS,
                "resins",
                id,
                MaterialProfile::from_toml_str,
            ),
            Kind::Support => restore_bundled(
                &mut self.supports,
                BUNDLED_SUPPORTS,
                "supports",
                id,
                SupportProfile::from_toml_str,
            ),
        }
    }
}

/// Whether `id` can be a file stem in the profile directory.
///
/// An id ends up as a path, so it is held to what a catalogue id looks like rather than
/// to what a file system would merely tolerate.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Where the user's own profiles live: the environment variable when it is set, otherwise
/// the platform's configuration directory.
pub fn user_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(PROFILE_DIR_VAR) {
        return Some(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "Encrust").map(|dirs| dirs.config_dir().join("profiles"))
}

type Parse<T> = fn(&str, &Path) -> Result<T, ProfileError>;

/// Puts the shipped profile of `id` back, or drops the id when nothing was shipped
/// under it.
fn restore_bundled<T>(
    map: &mut BTreeMap<String, Entry<T>>,
    table: &[(&str, &str)],
    kind: &str,
    id: &str,
    parse: Parse<T>,
) -> Result<(), ProfileError> {
    let Some((_, source)) = table.iter().find(|(name, _)| *name == id) else {
        map.remove(id);
        return Ok(());
    };
    let path = PathBuf::from(format!("<bundled>/{kind}/{id}.toml"));
    let profile = parse(source, &path)?;
    map.insert(
        id.to_owned(),
        Entry {
            id: id.to_owned(),
            profile,
            source: Source::Bundled,
        },
    );
    Ok(())
}

fn bundled_table<T>(
    table: &[(&str, &str)],
    kind: &str,
    parse: Parse<T>,
) -> Result<BTreeMap<String, Entry<T>>, ProfileError> {
    table
        .iter()
        .map(|(id, source)| {
            // Nothing on disk to point at, so an error in a shipped profile names where
            // in the repository it came from.
            let path = PathBuf::from(format!("<bundled>/{kind}/{id}.toml"));
            let profile = parse(source, &path)?;
            Ok((
                (*id).to_owned(),
                Entry {
                    id: (*id).to_owned(),
                    profile,
                    source: Source::Bundled,
                },
            ))
        })
        .collect()
}

fn overlay<T>(
    map: &mut BTreeMap<String, Entry<T>>,
    store: &dyn ProfileStore,
    kind: Kind,
    parse: Parse<T>,
) -> Result<(), ProfileError> {
    for (id, path, toml) in store.read_all(kind)? {
        let profile = parse(&toml, &path)?;
        map.insert(
            id.clone(),
            Entry {
                id,
                profile,
                source: Source::User(path),
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrinterTuning;

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
            assert_eq!(shipped, SupportProfile::medium(), "the asset is the preset");
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
    fn a_name_that_would_not_be_a_file_stem_is_refused() {
        in_its_own_dir("bad-id", |_, mut catalogue| {
            let profile = PrinterProfile::from_toml_str(SAMPLE_PRINTER, Path::new("inline.toml"))
                .expect("valid");
            let error = catalogue.save_printer("../escape", &profile).unwrap_err();
            assert!(matches!(error, ProfileError::BadId { .. }));
        });
    }

    #[test]
    fn a_profile_that_would_not_load_is_never_written() {
        in_its_own_dir("invalid-save", |dir, mut catalogue| {
            let mut broken =
                PrinterProfile::from_toml_str(SAMPLE_PRINTER, Path::new("inline.toml"))
                    .expect("valid");
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
}
