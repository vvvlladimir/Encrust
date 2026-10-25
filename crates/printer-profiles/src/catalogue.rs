//! The catalogue of shipped printers, resins and support profiles, and the user directory that overrides it
//! by id. See docs/decisions/0049.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{
    AutoProfile, DirStore, MaterialProfile, Preset, PrinterProfile, PrinterTuning, ProfileError,
    ProfileStore, SupportProfile,
};

mod unreadable;

pub use unreadable::Unreadable;

include!(concat!(env!("OUT_DIR"), "/bundled_profiles.rs"));

/// Environment variable holding a profile directory, used in place of the platform one.
pub const PROFILE_DIR_VAR: &str = "ENCRUST_PROFILE_DIR";

/// Which half of the catalogue a profile belongs to, and the directory it lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Printer,
    Resin,
    Support,
    /// Named settings of the Hollow tool, as a [`Preset`].
    Hollow,
    /// Named settings of the Drain tool, as a [`Preset`].
    Drain,
    /// The user's copy of a shipped automatic profile: one can be retuned, none added.
    Auto,
}

impl Kind {
    pub fn dir(self) -> &'static str {
        match self {
            Self::Printer => "printers",
            Self::Resin => "resins",
            Self::Support => "supports",
            Self::Hollow => "hollow",
            Self::Drain => "drain",
            Self::Auto => "auto",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Printer => "printer",
            Self::Resin => "resin",
            Self::Support => "support profile",
            Self::Hollow => "hollow profile",
            Self::Drain => "drain profile",
            Self::Auto => "automatic profile",
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
    /// Automatic profiles. Only those that ship exist; the user may keep a retuned copy
    /// of one, and tunes the shapes they name as any support profile.
    autos: BTreeMap<String, Entry<AutoProfile>>,
    /// Named tool settings, which nothing ships: each is the user's own.
    hollows: BTreeMap<String, Entry<Preset>>,
    drains: BTreeMap<String, Entry<Preset>>,
    /// The user's files that would not read, left out rather than failing the catalogue.
    unreadable: Vec<Unreadable>,
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
            autos: bundled_table(BUNDLED_AUTOS, "auto", AutoProfile::from_toml_str)?,
            hollows: BTreeMap::new(),
            drains: BTreeMap::new(),
            unreadable: Vec::new(),
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
        let printers = overlay(
            &mut self.printers,
            &*store,
            Kind::Printer,
            PrinterProfile::from_toml_str,
            Adds::Yes,
        )?;
        self.unreadable.extend(printers);
        let copies = self.printers.values_mut();
        for entry in copies.filter(|entry| matches!(entry.source, Source::User(_))) {
            entry.profile.locked = shipped_locks(&entry.id, &entry.profile);
        }
        let resins = overlay(
            &mut self.resins,
            &*store,
            Kind::Resin,
            MaterialProfile::from_toml_str,
            Adds::Yes,
        )?;
        let supports = overlay(
            &mut self.supports,
            &*store,
            Kind::Support,
            SupportProfile::from_toml_str,
            Adds::Yes,
        )?;
        let autos = overlay(
            &mut self.autos,
            &*store,
            Kind::Auto,
            AutoProfile::from_toml_str,
            Adds::No,
        )?;
        let hollows = overlay(
            &mut self.hollows,
            &*store,
            Kind::Hollow,
            Preset::from_toml_str,
            Adds::Yes,
        )?;
        let drains = overlay(
            &mut self.drains,
            &*store,
            Kind::Drain,
            Preset::from_toml_str,
            Adds::Yes,
        )?;
        self.unreadable.extend(
            [resins, supports, autos, hollows, drains]
                .into_iter()
                .flatten(),
        );
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

    /// The automatic profiles, each naming support profiles of this catalogue by id.
    pub fn autos(&self) -> impl Iterator<Item = &Entry<AutoProfile>> {
        self.autos.values()
    }

    /// Whether the support profile `id` is one an automatic profile builds to: the lists
    /// offer those apart from the profiles chosen by hand.
    pub fn is_auto_shape(&self, id: &str) -> bool {
        self.autos
            .values()
            .any(|auto| auto.profile.every_shape().any(|shape| shape == id))
    }

    /// The named settings kept for a tool: `Kind::Hollow` or `Kind::Drain`, none else.
    pub fn presets(&self, kind: Kind) -> impl Iterator<Item = &Entry<Preset>> {
        self.preset_table(kind)
            .into_iter()
            .flat_map(BTreeMap::values)
    }

    fn preset_table(&self, kind: Kind) -> Option<&BTreeMap<String, Entry<Preset>>> {
        match kind {
            Kind::Hollow => Some(&self.hollows),
            Kind::Drain => Some(&self.drains),
            Kind::Printer | Kind::Resin | Kind::Support | Kind::Auto => None,
        }
    }

    fn preset_table_mut(&mut self, kind: Kind) -> Option<&mut BTreeMap<String, Entry<Preset>>> {
        match kind {
            Kind::Hollow => Some(&mut self.hollows),
            Kind::Drain => Some(&mut self.drains),
            Kind::Printer | Kind::Resin | Kind::Support | Kind::Auto => None,
        }
    }

    /// Writes `preset` as the named settings of `kind` under `id`. A kind that is not a
    /// tool's is unknown to this call.
    pub fn save_preset(
        &mut self,
        kind: Kind,
        id: &str,
        preset: &Preset,
    ) -> Result<PathBuf, ProfileError> {
        if self.preset_table(kind).is_none() {
            return Err(ProfileError::Unknown {
                kind: kind.label(),
                id: id.to_owned(),
            });
        }
        let path = self.keep(kind, id, |path| preset.to_toml_string(path))?;
        if let Some(table) = self.preset_table_mut(kind) {
            table.insert(
                id.to_owned(),
                Entry {
                    id: id.to_owned(),
                    profile: preset.clone(),
                    source: Source::User(path.clone()),
                },
            );
        }
        Ok(path)
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
            Kind::Auto => BUNDLED_AUTOS,
            Kind::Hollow | Kind::Drain => return false,
        };
        table.iter().any(|(name, _)| *name == id)
    }

    /// The printer this build ships under `id`, whatever the user's copy says; tells an older
    /// release's copy apart and is what *Restore* writes back (`docs/decisions/0196`).
    pub fn shipped_printer(&self, id: &str) -> Option<PrinterProfile> {
        shipped_printer(id)
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
        &mut self,
        kind: Kind,
        id: &str,
        toml: impl FnOnce(&Path) -> Result<String, ProfileError>,
    ) -> Result<PathBuf, ProfileError> {
        let store = self.store_for(id)?;
        let path = store.path_of(kind, id);
        store.write(kind, id, &toml(&path)?)?;
        self.readable_again(kind, id);
        Ok(path)
    }

    /// Writes a printer into the user's directory and takes it into the catalogue, where
    /// it replaces the shipped machine of the same id.
    pub fn save_printer(
        &mut self,
        id: &str,
        profile: &PrinterProfile,
    ) -> Result<PathBuf, ProfileError> {
        let profile = PrinterProfile {
            locked: shipped_locks(id, profile),
            ..profile.clone()
        };
        let path = self.keep(Kind::Printer, id, |path| profile.to_toml_string(path))?;
        self.printers.insert(
            id.to_owned(),
            Entry {
                id: id.to_owned(),
                profile,
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

    /// Writes the user's copy of the shipped automatic profile `id`. There is no other
    /// kind of automatic profile, so an id that does not ship is refused.
    pub fn save_auto(&mut self, id: &str, profile: &AutoProfile) -> Result<PathBuf, ProfileError> {
        if !self.autos.contains_key(id) {
            return Err(ProfileError::Unknown {
                kind: Kind::Auto.label(),
                id: id.to_owned(),
            });
        }
        let path = self.keep(Kind::Auto, id, |path| profile.to_toml_string(path))?;
        self.autos.insert(
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
        self.readable_again(kind, id);
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
            Kind::Auto => restore_bundled(
                &mut self.autos,
                BUNDLED_AUTOS,
                "auto",
                id,
                AutoProfile::from_toml_str,
            ),
            Kind::Hollow | Kind::Drain => {
                if let Some(table) = self.preset_table_mut(kind) {
                    table.remove(id);
                }
                Ok(())
            }
        }
    }
}

/// Whether `id` can be a file stem in the profile directory.
/// An id becomes a path, so it is held to what a catalogue id looks like.
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

/// What a machine locks: the shipped profile's table where one ships under `id`, whatever a
/// copy says, so a copy made before a lock still takes it (ADR 0301).
fn shipped_locks(id: &str, profile: &PrinterProfile) -> PrinterTuning {
    shipped_printer(id).map_or_else(|| profile.locked.clone(), |shipped| shipped.locked)
}

fn shipped_printer(id: &str) -> Option<PrinterProfile> {
    let (_, source) = BUNDLED_PRINTERS.iter().find(|(name, _)| *name == id)?;
    let path = PathBuf::from(format!("<bundled>/printers/{id}.toml"));
    PrinterProfile::from_toml_str(source, &path).ok()
}

pub(crate) fn shipped_support(id: &str) -> Result<SupportProfile, ProfileError> {
    shipped_in(
        BUNDLED_SUPPORTS,
        Kind::Support,
        id,
        SupportProfile::from_toml_str,
    )
}

pub(crate) fn shipped_auto(id: &str) -> Result<AutoProfile, ProfileError> {
    shipped_in(BUNDLED_AUTOS, Kind::Auto, id, AutoProfile::from_toml_str)
}

/// The profile of `kind` that `table` ships under `id`, read by `parse`.
fn shipped_in<T>(
    table: &[(&str, &str)],
    kind: Kind,
    id: &str,
    parse: Parse<T>,
) -> Result<T, ProfileError> {
    let (_, source) =
        table
            .iter()
            .find(|(name, _)| *name == id)
            .ok_or_else(|| ProfileError::Unknown {
                kind: kind.label(),
                id: id.to_owned(),
            })?;
    parse(
        source,
        &PathBuf::from(format!("<bundled>/{}/{id}.toml", kind.dir())),
    )
}

/// Whether the user's directory may hold ids nothing ships, or only copies of shipped ones.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Adds {
    Yes,
    No,
}

/// Lays the user's files of `kind` over `map` by id, and answers those that would not read:
/// each leaves in place whatever was there before it.
fn overlay<T>(
    map: &mut BTreeMap<String, Entry<T>>,
    store: &dyn ProfileStore,
    kind: Kind,
    parse: Parse<T>,
    adds: Adds,
) -> Result<Vec<Unreadable>, ProfileError> {
    let mut unreadable = Vec::new();
    for (id, path, toml) in store.read_all(kind)? {
        if adds == Adds::No && !map.contains_key(&id) {
            continue;
        }
        match parse(&toml, &path) {
            Ok(profile) => {
                let source = Source::User(path);
                map.insert(
                    id.clone(),
                    Entry {
                        id,
                        profile,
                        source,
                    },
                );
            }
            Err(error) => unreadable.push(Unreadable {
                kind,
                id,
                path,
                error: Arc::new(error),
            }),
        }
    }
    Ok(unreadable)
}

#[cfg(test)]
mod tests;
