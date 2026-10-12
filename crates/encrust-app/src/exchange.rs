//! Profiles carried out of the catalogue as files and taken back in, through the
//! catalogue's own calls. See `docs/decisions/0233` and `0235`.

use std::path::Path;

use core_engine::project::{DrainState, HollowState};
use printer_profiles::{Catalogue, Kind, ProfileError};

use crate::files::Handed;
use crate::state::Machine;

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

/// Takes `text` in as a profile of `kind` under an id of its own, a preset held to
/// reading as its tool's settings. Answers the new id.
pub fn import(
    catalogue: &mut Catalogue,
    kind: Kind,
    text: &str,
    path: &Path,
    printer: Option<&str>,
) -> Result<String, ProfileError> {
    match kind {
        Kind::Hollow => catalogue.import_preset::<HollowState>(kind, text, path),
        Kind::Drain => catalogue.import_preset::<DrainState>(kind, text, path),
        Kind::Printer | Kind::Resin | Kind::Support => catalogue.import(kind, text, path, printer),
    }
}

#[cfg(test)]
mod tests {
    use printer_profiles::Preset;

    use super::*;

    /// The shipped catalogue over a directory of its own, which every write lands in.
    fn writable(name: &str) -> Catalogue {
        let dir =
            std::env::temp_dir().join(format!("encrust-exchange-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Catalogue::with_root(&dir).expect("a missing directory reads as empty")
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
        let text = catalogue.export(Kind::Drain, "wide").expect("just kept");

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
}
