//! Profiles carried out of the catalogue as files and taken back in, as the TOML kept in the
//! user's directory. See `docs/decisions/0233` and `0235`.

use std::path::Path;

use serde::de::DeserializeOwned;

use crate::{
    Catalogue, Entry, ForeignResin, Kind, MaterialProfile, Preset, PrinterProfile, PrinterTuning,
    ProfileError, SupportProfile, own_id,
};

impl Catalogue {
    /// The profile of `kind` kept under `id`, as the file it is written out to.
    pub fn export(&self, kind: Kind, id: &str) -> Result<String, ProfileError> {
        let path = Path::new(id);
        match kind {
            Kind::Printer => self.printer(id)?.profile.to_toml_string(path),
            Kind::Resin => self.resin(id)?.profile.to_toml_string(path),
            Kind::Support => self.support(id)?.profile.to_toml_string(path),
            Kind::Hollow | Kind::Drain => self.preset(kind, id)?.to_toml_string(path),
            Kind::Auto => match self.autos().find(|entry| entry.id == id) {
                Some(entry) => entry.profile.to_toml_string(path),
                None => Err(ProfileError::Unknown {
                    kind: kind.label(),
                    id: id.to_owned(),
                }),
            },
        }
    }

    /// Takes `text` in as a printer, resin or support profile under a new id, renamed if taken; a
    /// resin is set up on `printer` too. Presets go through [`Catalogue::import_preset`].
    pub fn import(
        &mut self,
        kind: Kind,
        text: &str,
        path: &Path,
        printer: Option<&str>,
    ) -> Result<String, ProfileError> {
        match kind {
            Kind::Printer => self.import_printer(PrinterProfile::from_toml_str(text, path)?),
            Kind::Resin => self.import_resin(MaterialProfile::from_toml_str(text, path)?, printer),
            Kind::Support => self.import_support(SupportProfile::from_toml_str(text, path)?),
            // No automatic profile is ever added, so none is taken in either.
            Kind::Hollow | Kind::Drain | Kind::Auto => Err(ProfileError::Unknown {
                kind: kind.label(),
                id: path.display().to_string(),
            }),
        }
    }

    /// Takes `text` in as a preset of `kind`, held to reading as `T`, that tool's
    /// settings: a drain preset handed in as a hollow one would only be skipped later.
    pub fn import_preset<T: DeserializeOwned>(
        &mut self,
        kind: Kind,
        text: &str,
        path: &Path,
    ) -> Result<String, ProfileError> {
        let mut preset = Preset::from_toml_str(text, path)?;
        preset.read::<T>(path)?;
        preset.name = fresh_name(&preset.name, |name| {
            self.presets(kind).any(|entry| entry.profile.name == name)
        });
        let id = unique_id(&preset.name, |id| self.is_taken(kind, id));
        self.save_preset(kind, &id, &preset)?;
        Ok(id)
    }

    fn import_printer(&mut self, mut profile: PrinterProfile) -> Result<String, ProfileError> {
        let label = |profile: &PrinterProfile| format!("{} {}", profile.manufacturer, profile.name);
        profile.name = fresh_name(&profile.name, |name| {
            let wanted = format!("{} {name}", profile.manufacturer);
            self.printers().any(|entry| label(&entry.profile) == wanted)
        });
        let id = own_id(&label(&profile), |id| self.is_taken(Kind::Printer, id))?;
        self.save_printer(&id, &profile)?;
        Ok(id)
    }

    /// The printer a sliced file was cut for, by its panel and the machine it names: the
    /// one machine of that panel, or the one of them the name picks. `None` where neither.
    pub fn printer_for_panel(
        &self,
        width_px: u32,
        height_px: u32,
        machine: Option<&str>,
    ) -> Option<&str> {
        let fits = |entry: &&Entry<PrinterProfile>| {
            let display = &entry.profile.display;
            (display.width_px, display.height_px) == (width_px, height_px)
                || (display.width_px, display.height_px) == (height_px, width_px)
        };
        let named = |entry: &&Entry<PrinterProfile>| {
            machine.is_some_and(|machine| names(&entry.profile, machine))
        };
        let panel: Vec<_> = self.printers().filter(fits).collect();
        let picked: Vec<_> = panel.iter().copied().filter(named).collect();
        match (picked.as_slice(), panel.as_slice()) {
            ([one], _) | ([], [one]) => Some(one.id.as_str()),
            _ => None,
        }
    }

    /// The printer a resin another program exported was set up for: by its panel as
    /// [`Catalogue::printer_for_panel`] picks it, or by name alone where it states no panel.
    pub fn printer_for_foreign(&self, foreign: &ForeignResin) -> Option<&str> {
        let machine = foreign.machine.as_deref();
        if let Some([width_px, height_px]) = foreign.panel_px {
            return self.printer_for_panel(width_px, height_px, machine);
        }
        let mut named = self
            .printers()
            .filter(|entry| machine.is_some_and(|machine| names(&entry.profile, machine)));
        match (named.next(), named.next()) {
            (Some(one), None) => Some(one.id.as_str()),
            _ => None,
        }
    }

    /// Takes `resin` in under an id of its own, renamed when the name is already taken, and
    /// sets it up on `printer`. Answers the new id.
    pub fn import_resin(
        &mut self,
        mut resin: MaterialProfile,
        printer: Option<&str>,
    ) -> Result<String, ProfileError> {
        resin.name = fresh_name(&resin.name, |name| {
            self.resins().any(|entry| entry.profile.name == name)
        });
        if let Some(printer) = printer {
            let locked = self
                .printer(printer)
                .map(|entry| entry.profile.locked.clone());
            set_up_on(&mut resin, printer, &locked.unwrap_or_default());
        }
        let id = own_id(&resin.name, |id| self.is_taken(Kind::Resin, id))?;
        self.save_resin(&id, &resin)?;
        Ok(id)
    }

    fn import_support(&mut self, mut profile: SupportProfile) -> Result<String, ProfileError> {
        profile.name = fresh_name(&profile.name, |name| {
            self.supports().any(|entry| entry.profile.name == name)
        });
        let id = unique_id(&profile.name, |id| self.is_taken(Kind::Support, id));
        self.save_support(&id, &profile)?;
        Ok(id)
    }

    fn preset(&self, kind: Kind, id: &str) -> Result<&Preset, ProfileError> {
        self.presets(kind)
            .find(|entry| entry.id == id)
            .map(|entry| &entry.profile)
            .ok_or_else(|| ProfileError::Unknown {
                kind: kind.label(),
                id: id.to_owned(),
            })
    }
}

/// The file stem `name` makes, numbered past the ones `taken` already claims.
pub fn unique_id(name: &str, taken: impl Fn(&str) -> bool) -> String {
    // A name in a script a file stem cannot hold still names a profile.
    let stem = match slug(name) {
        stem if stem.is_empty() => "profile".to_owned(),
        stem => stem,
    };
    let mut id = stem.clone();
    let mut next = 2;
    while taken(&id) {
        id = format!("{stem}-{next}");
        next += 1;
    }
    id
}

/// The file stem a name makes: lower-case letters and digits, one dash between words.
pub(crate) fn slug(name: &str) -> String {
    let mut stem = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            stem.push(c.to_ascii_lowercase());
        } else if !stem.is_empty() && !stem.ends_with('-') {
            stem.push('-');
        }
    }
    stem.trim_end_matches('-').to_owned()
}

/// Whether `machine` is what `profile`'s firmware or label calls it.
fn names(profile: &PrinterProfile, machine: &str) -> bool {
    let label = format!("{} {}", profile.manufacturer, profile.name);
    [profile.machine_name(), label.as_str()]
        .iter()
        .any(|name| same_name(name, machine))
}

/// Whether two machine names are one, whatever their case and spacing.
fn same_name(ours: &str, theirs: &str) -> bool {
    ours.split_whitespace()
        .map(str::to_lowercase)
        .eq(theirs.split_whitespace().map(str::to_lowercase))
}

/// `name`, or marked as imported where it is `taken`, numbered past the marks taken too.
fn fresh_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    let mut fresh = format!("{name} (imported)");
    let mut next = 2;
    while taken(&fresh) {
        fresh = format!("{name} (imported {next})");
        next += 1;
    }
    fresh
}

/// Sets `resin` up on `printer`, starting from the printer it was last tuned on, unless
/// the file already carries numbers for it. What the machine locks, the file's own numbers
/// state, as the program that wrote it would print them.
fn set_up_on(resin: &mut MaterialProfile, printer: &str, locked: &PrinterTuning) {
    if !resin.is_tuned_for(printer) {
        let start = resin.starting_point(printer);
        let tuning = PrinterTuning::of_changes(resin, &start).and(&locked.read_from(&start));
        resin.printers.insert(printer.to_owned(), tuning);
    }
    resin.last_printer = Some(printer.to_owned());
}

#[cfg(test)]
mod tests;
