//! The catalogue of shipped printers, resins and support profiles, and the user directory that overrides it
//! by id. See docs/decisions/0049.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{MaterialProfile, PrinterProfile, ProfileError, SupportProfile};

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
    /// Where an edited profile is written. `None` until a directory is laid over the
    /// catalogue, which is what makes a bundled-only catalogue read-only.
    root: Option<PathBuf>,
}

impl Catalogue {
    /// Only what is shipped in the binary.
    pub fn bundled() -> Result<Self, ProfileError> {
        Ok(Self {
            printers: bundled_table(BUNDLED_PRINTERS, "printers", PrinterProfile::from_toml_str)?,
            resins: bundled_table(BUNDLED_RESINS, "resins", MaterialProfile::from_toml_str)?,
            supports: bundled_table(BUNDLED_SUPPORTS, "supports", SupportProfile::from_toml_str)?,
            root: None,
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

    /// The directory edits are written to, if there is one.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Reads `<dir>/printers`, `<dir>/resins` and `<dir>/supports` over what is already
    /// loaded. A missing directory is not an error: most users have never made one.
    pub fn overlay(&mut self, dir: &Path) -> Result<(), ProfileError> {
        self.root = Some(dir.to_owned());
        overlay_dir(
            &mut self.printers,
            &dir.join("printers"),
            PrinterProfile::from_toml_str,
        )?;
        overlay_dir(
            &mut self.resins,
            &dir.join("resins"),
            MaterialProfile::from_toml_str,
        )?;
        overlay_dir(
            &mut self.supports,
            &dir.join("supports"),
            SupportProfile::from_toml_str,
        )
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

    /// Where a profile of this kind and id belongs in the directory being edited.
    pub fn user_path(&self, kind: Kind, id: &str) -> Result<PathBuf, ProfileError> {
        if !is_valid_id(id) {
            return Err(ProfileError::BadId { id: id.to_owned() });
        }
        let dir = self.root.as_ref().ok_or(ProfileError::NoUserDir {
            variable: PROFILE_DIR_VAR,
        })?;
        Ok(dir.join(kind.dir()).join(format!("{id}.toml")))
    }

    /// Writes a printer into the user's directory and takes it into the catalogue, where
    /// it replaces the shipped machine of the same id.
    pub fn save_printer(
        &mut self,
        id: &str,
        profile: &PrinterProfile,
    ) -> Result<PathBuf, ProfileError> {
        let path = self.user_path(Kind::Printer, id)?;
        write_profile(&path, |path| profile.save(path))?;
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
        let path = self.user_path(Kind::Resin, id)?;
        write_profile(&path, |path| resin.save(path))?;
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
        let path = self.user_path(Kind::Support, id)?;
        write_profile(&path, |path| profile.save(path))?;
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
        let path = self.user_path(kind, id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(ProfileError::Io { path, source }),
        }
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

    /// The resin a printer starts on: the first one carrying numbers for that machine,
    /// or the first in the catalogue when none does.
    pub fn default_resin_for(&self, printer_id: &str) -> Option<&Entry<MaterialProfile>> {
        self.resins()
            .find(|entry| entry.profile.is_tuned_for(printer_id))
            .or_else(|| self.resins().next())
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

/// Makes the directory before writing, because the user's profile directory does not
/// exist until the first profile they save.
fn write_profile(
    path: &Path,
    save: impl FnOnce(&Path) -> Result<(), ProfileError>,
) -> Result<(), ProfileError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|source| ProfileError::Io {
            path: dir.to_owned(),
            source,
        })?;
    }
    save(path)
}

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

fn overlay_dir<T>(
    map: &mut BTreeMap<String, Entry<T>>,
    dir: &Path,
    parse: Parse<T>,
) -> Result<(), ProfileError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ProfileError::Io {
                path: dir.to_owned(),
                source,
            });
        }
    };

    for entry in entries {
        let path = entry
            .map_err(|source| ProfileError::Io {
                path: dir.to_owned(),
                source,
            })?
            .path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let source = std::fs::read_to_string(&path).map_err(|source| ProfileError::Io {
            path: path.clone(),
            source,
        })?;
        let profile = parse(&source, &path)?;
        map.insert(
            id.to_owned(),
            Entry {
                id: id.to_owned(),
                profile,
                source: Source::User(path.clone()),
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_profile_parses() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");

        assert!(
            catalogue.printers().count() >= 5,
            "the catalogue ships at least five machines"
        );
        assert!(catalogue.resins().count() >= 4);
        assert!(
            catalogue
                .printers()
                .all(|entry| entry.source == Source::Bundled)
        );
    }

    #[test]
    fn every_bundled_resin_is_tuned_for_a_machine_and_resolves_for_the_rest() {
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");

        // Nobody has measured these resins on 150 machines, and an invented exposure
        // costs a tank of resin, so a resin is tuned where it was measured and every
        // other machine resolves to the numbers it was last measured with.
        for resin in catalogue.resins() {
            assert!(
                catalogue
                    .printers()
                    .any(|printer| resin.profile.is_tuned_for(&printer.id)),
                "{} is tuned for no shipped machine",
                resin.id
            );
            for printer in catalogue.printers() {
                let resolved = catalogue
                    .resin_for(&resin.id, &printer.id)
                    .expect("a shipped resin and a shipped machine");
                assert!(
                    resolved.exposure_s > 0.0 && resolved.bottom_exposure_s > 0.0,
                    "{} resolves to no exposure on {}",
                    resin.id,
                    printer.id
                );
            }
        }
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
        let catalogue = Catalogue::bundled().expect("the shipped catalogue is valid");

        let base = &catalogue.resin("generic-resin").expect("shipped").profile;
        let tuned = catalogue
            .resin_for("generic-resin", "elegoo-mars-3-pro")
            .expect("shipped");

        assert!(
            tuned.exposure_s > base.exposure_s,
            "an older LED matrix needs longer than the untuned starting point"
        );
        assert!(tuned.printers.is_empty(), "a resolved resin carries no map");
    }

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
        assert!(catalogue.is_shipped(Kind::Resin, "standard-grey"));
        assert!(!catalogue.is_shipped(Kind::Resin, "my-resin"));
        assert!(catalogue.is_shipped(Kind::Support, "medium"));
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
        assert!(catalogue.root().is_none());
        let error = catalogue
            .save_resin("my-resin", &MaterialProfile::default())
            .unwrap_err();
        assert!(matches!(error, ProfileError::NoUserDir { .. }));
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
