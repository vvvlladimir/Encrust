//! Profiles carried out of the catalogue as files and taken back in: each the TOML it is
//! kept as in the user's directory, so a file written anywhere reads anywhere the
//! catalogue does. See `docs/decisions/0233` and `0235`.

use std::path::Path;

use serde::de::DeserializeOwned;

use crate::{
    Catalogue, Kind, MaterialProfile, Preset, PrinterProfile, PrinterTuning, ProfileError,
    SupportProfile,
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
        }
    }

    /// Takes `text` in as a printer, resin or support profile under an id of its own,
    /// renamed when the name is already taken: an import never writes over what the user
    /// has. A resin is set up on `printer` as well. Answers the new id. A tool's preset
    /// goes through [`Catalogue::import_preset`], which knows the tool's settings.
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
            Kind::Hollow | Kind::Drain => Err(ProfileError::Unknown {
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
        if self
            .presets(kind)
            .any(|entry| entry.profile.name == preset.name)
        {
            preset.name = imported(&preset.name);
        }
        let id = unique_id(&preset.name, |id| {
            self.presets(kind).any(|entry| entry.id == id)
        });
        self.save_preset(kind, &id, &preset)?;
        Ok(id)
    }

    fn import_printer(&mut self, mut profile: PrinterProfile) -> Result<String, ProfileError> {
        let label = |profile: &PrinterProfile| format!("{} {}", profile.manufacturer, profile.name);
        if self
            .printers()
            .any(|entry| label(&entry.profile) == label(&profile))
        {
            profile.name = imported(&profile.name);
        }
        let id = unique_id(&label(&profile), |id| self.printer(id).is_ok());
        self.save_printer(&id, &profile)?;
        Ok(id)
    }

    fn import_resin(
        &mut self,
        mut resin: MaterialProfile,
        printer: Option<&str>,
    ) -> Result<String, ProfileError> {
        if self.resins().any(|entry| entry.profile.name == resin.name) {
            resin.name = imported(&resin.name);
        }
        if let Some(printer) = printer {
            set_up_on(&mut resin, printer);
        }
        let id = unique_id(&resin.name, |id| self.resin(id).is_ok());
        self.save_resin(&id, &resin)?;
        Ok(id)
    }

    fn import_support(&mut self, mut profile: SupportProfile) -> Result<String, ProfileError> {
        if self
            .supports()
            .any(|entry| entry.profile.name == profile.name)
        {
            profile.name = imported(&profile.name);
        }
        let id = unique_id(&profile.name, |id| self.support(id).is_ok());
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
fn slug(name: &str) -> String {
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

fn imported(name: &str) -> String {
    format!("{name} (imported)")
}

/// Sets `resin` up on `printer`, starting from the printer it was last tuned on, unless
/// the file already carries numbers for it.
fn set_up_on(resin: &mut MaterialProfile, printer: &str) {
    if !resin.is_tuned_for(printer) {
        let start = resin.starting_point(printer);
        let tuning = PrinterTuning::of_changes(resin, &start);
        resin.printers.insert(printer.to_owned(), tuning);
    }
    resin.last_printer = Some(printer.to_owned());
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::Source;

    const MARS: &str = "elegoo-mars-3-pro";
    const SATURN: &str = "elegoo-saturn-4-ultra";

    /// The shipped catalogue over a directory of its own, which every write lands in.
    fn writable(name: &str) -> Catalogue {
        let dir =
            std::env::temp_dir().join(format!("encrust-exchange-{name}-{}", std::process::id()));
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
}
