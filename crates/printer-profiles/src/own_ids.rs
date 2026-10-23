//! Ids of the user's own printers and resins: a name with a random tail, so that a project
//! naming one never lands on somebody else's profile of the same name (ADR 0261).

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::exchange::slug;
use crate::{Catalogue, Kind, MaterialProfile, ProfileError, Source};

/// Hex digits in the tail: 32 bits, so two people's profiles of one name never meet.
const TAIL_LEN: usize = 8;

/// A profile [`Catalogue::give_own_ids`] moved, and the ids it moved between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renamed {
    pub kind: Kind,
    pub from: String,
    pub to: String,
}

/// An id for the user's own printer or resin: the file stem `name` makes and a random
/// tail, drawn again in the rare case `taken` already claims it.
pub fn own_id(name: &str, taken: impl Fn(&str) -> bool) -> Result<String, ProfileError> {
    let stem = match slug(name) {
        stem if stem.is_empty() => "profile".to_owned(),
        stem => stem,
    };
    loop {
        let id = format!("{stem}-{}", tail()?);
        if !taken(&id) {
            return Ok(id);
        }
    }
}

fn tail() -> Result<String, ProfileError> {
    let mut bytes = [0u8; TAIL_LEN / 2];
    getrandom::fill(&mut bytes).map_err(|source| ProfileError::NoRandomness { source })?;
    Ok(bytes.iter().fold(String::new(), |mut tail, byte| {
        let _ = write!(tail, "{byte:02x}");
        tail
    }))
}

/// Whether `id` already ends in a tail [`own_id`] gives.
fn has_own_tail(id: &str) -> bool {
    id.rsplit_once('-').is_some_and(|(stem, tail)| {
        !stem.is_empty()
            && tail.len() == TAIL_LEN
            && tail
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

impl Catalogue {
    /// Moves every printer and resin the user keeps under a bare name to an id of its own,
    /// and points the resins' tuning at the printers' new ids. Answers what moved, so
    /// whatever else holds one of those ids can follow it.
    pub fn give_own_ids(&mut self) -> Result<Vec<Renamed>, ProfileError> {
        let mut renamed = Vec::new();
        for from in self.bare(Kind::Printer) {
            let to = own_id(&from, |id| self.printer(id).is_ok())?;
            let profile = self.printer(&from)?.profile.clone();
            self.save_printer(&to, &profile)?;
            self.forget_user_copy(Kind::Printer, &from)?;
            renamed.push(Renamed {
                kind: Kind::Printer,
                from,
                to,
            });
        }
        self.retarget_tuning(&renamed)?;
        for from in self.bare(Kind::Resin) {
            let to = own_id(&from, |id| self.resin(id).is_ok())?;
            let resin = self.resin(&from)?.profile.clone();
            self.save_resin(&to, &resin)?;
            self.forget_user_copy(Kind::Resin, &from)?;
            renamed.push(Renamed {
                kind: Kind::Resin,
                from,
                to,
            });
        }
        Ok(renamed)
    }

    /// The user's own profiles of `kind` with no tail: not a shipped id they overrode.
    fn bare(&self, kind: Kind) -> Vec<String> {
        let ids: Vec<(&String, &Source)> = match kind {
            Kind::Printer => self.printers().map(|e| (&e.id, &e.source)).collect(),
            Kind::Resin => self.resins().map(|e| (&e.id, &e.source)).collect(),
            Kind::Support | Kind::Hollow | Kind::Drain | Kind::Auto => Vec::new(),
        };
        ids.into_iter()
            .filter(|(id, source)| {
                matches!(source, Source::User(_)) && !self.is_shipped(kind, id) && !has_own_tail(id)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Rewrites every resin of the user's whose tuning names a printer that moved.
    fn retarget_tuning(&mut self, renamed: &[Renamed]) -> Result<(), ProfileError> {
        let changed: Vec<(String, MaterialProfile)> = self
            .resins()
            .filter(|entry| matches!(entry.source, Source::User(_)))
            .filter_map(|entry| {
                retargeted(&entry.profile, renamed).map(|resin| (entry.id.clone(), resin))
            })
            .collect();
        for (id, resin) in changed {
            self.save_resin(&id, &resin)?;
        }
        Ok(())
    }
}

/// `resin` with its tuning keyed by the printers' new ids, or `None` when it names none
/// that moved.
fn retargeted(resin: &MaterialProfile, renamed: &[Renamed]) -> Option<MaterialProfile> {
    let moved = |id: &str| {
        renamed
            .iter()
            .find(|r| r.kind == Kind::Printer && r.from == id)
            .map(|r| r.to.clone())
    };
    let touches = resin.printers.keys().any(|id| moved(id).is_some())
        || resin.last_printer.as_deref().and_then(moved).is_some();
    if !touches {
        return None;
    }
    let printers: BTreeMap<_, _> = resin
        .printers
        .iter()
        .map(|(id, tuning)| (moved(id).unwrap_or_else(|| id.clone()), tuning.clone()))
        .collect();
    let last_printer = resin
        .last_printer
        .as_deref()
        .map(|id| moved(id).unwrap_or_else(|| id.to_owned()));
    Some(MaterialProfile {
        last_printer,
        printers,
        ..resin.clone()
    })
}

#[cfg(test)]
mod tests {
    use crate::{PrinterProfile, PrinterTuning, is_valid_id};

    use super::*;

    const SATURN: &str = "elegoo-saturn-4-ultra";

    fn writable(name: &str) -> Catalogue {
        let dir =
            std::env::temp_dir().join(format!("encrust-own-ids-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Catalogue::with_root(&dir).expect("a missing directory reads as empty")
    }

    #[test]
    fn an_own_id_is_the_name_with_a_random_tail() {
        let first = own_id("Saturn at home", |_| false).expect("the platform has randomness");
        let second = own_id("Saturn at home", |_| false).expect("the platform has randomness");

        assert!(first.starts_with("saturn-at-home-"), "{first}");
        assert!(has_own_tail(&first) && is_valid_id(&first), "{first}");
        assert_ne!(
            first, second,
            "two people naming a printer alike get two ids"
        );
        let script = own_id("Смола", |_| false).expect("the platform has randomness");
        assert!(script.starts_with("profile-"), "{script}");
    }

    #[test]
    fn a_tail_is_told_from_a_word() {
        assert!(has_own_tail("grey-0a1b2c3d"));
        assert!(!has_own_tail("grey"));
        assert!(!has_own_tail("saturn-4-ultra"));
        assert!(
            !has_own_tail("grey-0A1B2C3D"),
            "own_id writes lower case only"
        );
        assert!(!has_own_tail("-0a1b2c3d"));
    }

    #[test]
    fn bare_profiles_move_and_resins_follow_their_printer() {
        let mut catalogue = writable("move");
        let printer = PrinterProfile {
            name: "Bench".to_owned(),
            ..PrinterProfile::default()
        };
        catalogue.save_printer("bench", &printer).expect("writable");
        let mut resin = MaterialProfile::default();
        resin
            .printers
            .insert("bench".to_owned(), PrinterTuning::default());
        resin
            .printers
            .insert(SATURN.to_owned(), PrinterTuning::default());
        resin.last_printer = Some("bench".to_owned());
        catalogue.save_resin("grey", &resin).expect("writable");
        let shipped = catalogue.printer(SATURN).expect("ships").profile.clone();
        catalogue.save_printer(SATURN, &shipped).expect("writable");

        let renamed = catalogue.give_own_ids().expect("writable");

        let printer_to = &renamed[0].to;
        assert_eq!(
            renamed.len(),
            2,
            "the bare printer and resin, not the shipped override"
        );
        assert!(catalogue.printer("bench").is_err(), "the bare id is gone");
        assert!(catalogue.printer(printer_to).is_ok());
        assert!(
            catalogue.printer(SATURN).is_ok(),
            "a shipped id stays everyone's"
        );
        let back = &catalogue.resin(&renamed[1].to).expect("moved").profile;
        assert!(
            back.is_tuned_for(printer_to),
            "the tuning follows its printer"
        );
        assert!(back.is_tuned_for(SATURN));
        assert_eq!(back.last_printer.as_deref(), Some(printer_to.as_str()));
        assert!(
            catalogue.give_own_ids().expect("writable").is_empty(),
            "a second run finds nothing bare"
        );
    }

    #[test]
    fn what_moved_is_read_back_from_the_directory() {
        let dir = std::env::temp_dir().join(format!("encrust-own-ids-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut catalogue = Catalogue::with_root(&dir).expect("a missing directory reads as empty");
        catalogue
            .save_resin("grey", &MaterialProfile::default())
            .expect("writable");

        let renamed = catalogue.give_own_ids().expect("writable");

        let again = Catalogue::with_root(&dir).expect("the directory reads");
        assert!(again.resin("grey").is_err(), "the bare file was removed");
        assert!(
            again.resin(&renamed[0].to).is_ok(),
            "and written under its own id"
        );
    }
}
