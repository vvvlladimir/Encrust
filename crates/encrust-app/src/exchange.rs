//! Profiles carried out of the catalogue as files and taken back in: each the TOML it is
//! kept as in the user's directory, so a file written here reads anywhere the catalogue
//! does. See `docs/decisions/0233`.

use std::path::Path;

use core_engine::project::{DrainState, HollowState};
use printer_profiles::{
    Catalogue, Kind, MaterialProfile, Preset, PrinterProfile, PrinterTuning, ProfileError,
    SupportProfile,
};

use crate::files::Handed;
use crate::settings::unique_id;
use crate::state::Machine;

/// The profile of `kind` kept under `id`, as the file it is written out to.
pub fn export(catalogue: &Catalogue, kind: Kind, id: &str) -> Result<String, ProfileError> {
    let path = Path::new(id);
    match kind {
        Kind::Printer => catalogue.printer(id)?.profile.to_toml_string(path),
        Kind::Resin => catalogue.resin(id)?.profile.to_toml_string(path),
        Kind::Support => catalogue.support(id)?.profile.to_toml_string(path),
        Kind::Hollow | Kind::Drain => preset(catalogue, kind, id)?.to_toml_string(path),
    }
}

/// What a profile's file is called when it is written out.
pub fn file_name(id: &str) -> String {
    format!("{id}.toml")
}

/// Reads `file` and takes it in as [`import`] does, saying in the status bar how it went.
/// Answers the new id.
pub fn import_file(
    machine: &mut Machine,
    kind: Kind,
    file: &Handed,
    printer: Option<&str>,
) -> Option<String> {
    use anyhow::Context as _;

    let path = file.path();
    let imported = file
        .text()
        .map_err(anyhow::Error::new)
        .and_then(|text| {
            let catalogue = &mut machine.slicing.catalogue;
            Ok(import(catalogue, kind, &text, path, printer)?)
        })
        .with_context(|| format!("cannot import {}", path.display()));
    let done = format!("Imported {}", path.display());
    machine.status.report(&done, imported)
}

/// Takes `text` in as a profile of `kind` under an id of its own, renamed when the name is
/// already taken: an import never writes over what the user has. A resin is set up on
/// `printer` as well, which is where it was imported from. Answers the new id.
pub fn import(
    catalogue: &mut Catalogue,
    kind: Kind,
    text: &str,
    path: &Path,
    printer: Option<&str>,
) -> Result<String, ProfileError> {
    match kind {
        Kind::Printer => {
            let mut profile = PrinterProfile::from_toml_str(text, path)?;
            let label =
                |profile: &PrinterProfile| format!("{} {}", profile.manufacturer, profile.name);
            if catalogue
                .printers()
                .any(|entry| label(&entry.profile) == label(&profile))
            {
                profile.name = imported(&profile.name);
            }
            let id = unique_id(&label(&profile), |id| catalogue.printer(id).is_ok());
            catalogue.save_printer(&id, &profile)?;
            Ok(id)
        }
        Kind::Resin => {
            let mut resin = MaterialProfile::from_toml_str(text, path)?;
            if catalogue
                .resins()
                .any(|entry| entry.profile.name == resin.name)
            {
                resin.name = imported(&resin.name);
            }
            if let Some(printer) = printer {
                set_up_on(&mut resin, printer);
            }
            let id = unique_id(&resin.name, |id| catalogue.resin(id).is_ok());
            catalogue.save_resin(&id, &resin)?;
            Ok(id)
        }
        Kind::Support => {
            let mut profile = SupportProfile::from_toml_str(text, path)?;
            if catalogue
                .supports()
                .any(|entry| entry.profile.name == profile.name)
            {
                profile.name = imported(&profile.name);
            }
            let id = unique_id(&profile.name, |id| catalogue.support(id).is_ok());
            catalogue.save_support(&id, &profile)?;
            Ok(id)
        }
        Kind::Hollow | Kind::Drain => import_preset(catalogue, kind, text, path),
    }
}

/// A tool's preset, held to reading as that tool's settings: a drain preset handed in as
/// a hollow one would only be skipped by the list later, without a word.
fn import_preset(
    catalogue: &mut Catalogue,
    kind: Kind,
    text: &str,
    path: &Path,
) -> Result<String, ProfileError> {
    let mut preset = Preset::from_toml_str(text, path)?;
    match kind {
        Kind::Hollow => preset.read::<HollowState>(path).map(drop)?,
        _ => preset.read::<DrainState>(path).map(drop)?,
    }
    if catalogue
        .presets(kind)
        .any(|entry| entry.profile.name == preset.name)
    {
        preset.name = imported(&preset.name);
    }
    let id = unique_id(&preset.name, |id| {
        catalogue.presets(kind).any(|entry| entry.id == id)
    });
    catalogue.save_preset(kind, &id, &preset)?;
    Ok(id)
}

fn preset<'a>(catalogue: &'a Catalogue, kind: Kind, id: &str) -> Result<&'a Preset, ProfileError> {
    catalogue
        .presets(kind)
        .find(|entry| entry.id == id)
        .map(|entry| &entry.profile)
        .ok_or_else(|| ProfileError::Unknown {
            kind: kind.label(),
            id: id.to_owned(),
        })
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
    use super::*;

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
    fn a_support_profile_comes_back_as_a_copy_beside_itself() {
        let mut catalogue = writable("support");
        let text = export(&catalogue, Kind::Support, "heavy").expect("heavy ships");
        let id = import(
            &mut catalogue,
            Kind::Support,
            &text,
            Path::new("heavy.toml"),
            None,
        )
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
        let text = export(&from, Kind::Printer, "bench-mars").expect("just saved");

        let mut to = writable("printer-to");
        let id = import(&mut to, Kind::Printer, &text, Path::new("bench.toml"), None)
            .expect("the file is a printer");
        let entry = to.printer(&id).expect("imported");
        assert_eq!(
            entry.profile.name, "Bench Mars",
            "no machine of that name was there"
        );
        assert!(
            crate::settings::installed(&entry.source),
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
        let text = export(&from, Kind::Resin, "bench-grey").expect("just saved");

        let mut to = writable("resin-to");
        let id = import(
            &mut to,
            Kind::Resin,
            &text,
            Path::new("grey.toml"),
            Some(SATURN),
        )
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

    #[test]
    fn a_drain_preset_travels_and_a_hollow_one_is_refused_in_its_place() {
        let mut catalogue = writable("presets");
        let state = DrainState {
            diameter_mm: 3.5,
            depth_mm: 6.0,
            taper: 0.0,
        };
        let preset = Preset::of("Wide", &state, Path::new("wide")).expect("the state is a table");
        catalogue
            .save_preset(Kind::Drain, "wide", &preset)
            .expect("writable");
        let text = export(&catalogue, Kind::Drain, "wide").expect("just kept");

        let again = import(
            &mut catalogue,
            Kind::Drain,
            &text,
            Path::new("wide.toml"),
            None,
        )
        .expect("the file is a drain preset");
        let entry = catalogue
            .presets(Kind::Drain)
            .find(|entry| entry.id == again)
            .expect("imported");
        assert_eq!(entry.profile.name, "Wide (imported)");

        let refused = import(
            &mut catalogue,
            Kind::Hollow,
            &text,
            Path::new("wide.toml"),
            None,
        );
        assert!(
            matches!(refused, Err(ProfileError::Parse { .. })),
            "drain numbers are not a hollow preset: {refused:?}"
        );
    }

    #[test]
    fn a_file_that_is_not_a_profile_is_refused() {
        let mut catalogue = writable("garbage");
        let refused = import(
            &mut catalogue,
            Kind::Support,
            "name = ",
            Path::new("broken.toml"),
            None,
        );
        assert!(matches!(refused, Err(ProfileError::Parse { .. })));
    }

    #[test]
    fn exporting_what_is_not_there_names_it() {
        let catalogue = writable("missing");
        let missing = export(&catalogue, Kind::Hollow, "nothing");
        assert!(matches!(missing, Err(ProfileError::Unknown { .. })));
    }
}
