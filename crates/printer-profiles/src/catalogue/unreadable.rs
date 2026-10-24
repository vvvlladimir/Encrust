//! The user's files that would not read: left out so the rest of the catalogue loads, the
//! shipped profile of the same id standing in. See docs/decisions/0353.

use std::path::PathBuf;
use std::sync::Arc;

use super::{Catalogue, Kind};
use crate::ProfileError;

/// A file in the user's profile directory left out of the catalogue because it would not read.
#[derive(Debug, Clone)]
pub struct Unreadable {
    pub kind: Kind,
    pub id: String,
    pub path: PathBuf,
    /// Why it would not read.
    pub error: Arc<ProfileError>,
}

impl Catalogue {
    /// The user's files left out because they would not read, the shipped profile of each
    /// id in use instead where one ships.
    pub fn unreadable(&self) -> &[Unreadable] {
        &self.unreadable
    }

    /// Whether `id` is in use for `kind`: by a profile, or by a file of the user's that would
    /// not read, which a new profile must not be written over.
    pub fn is_taken(&self, kind: Kind, id: &str) -> bool {
        let listed = match kind {
            Kind::Printer => self.printer(id).is_ok(),
            Kind::Resin => self.resin(id).is_ok(),
            Kind::Support => self.support(id).is_ok(),
            Kind::Auto => self.autos().any(|entry| entry.id == id),
            Kind::Hollow | Kind::Drain => self.presets(kind).any(|entry| entry.id == id),
        };
        listed
            || self
                .unreadable
                .iter()
                .any(|file| file.kind == kind && file.id == id)
    }

    /// Whether throwing `file` away gives back a shipped profile, rather than losing a
    /// profile of the user's own.
    pub fn resets_to_shipped(&self, file: &Unreadable) -> bool {
        self.is_shipped(file.kind, &file.id)
    }

    /// Throws away every unreadable copy of a shipped profile, leaving the shipped one in
    /// use; a file under an id of the user's own stays. Answers how many went.
    pub fn reset_unreadable(&mut self) -> Result<usize, ProfileError> {
        let copies: Vec<(Kind, String)> = self
            .unreadable
            .iter()
            .filter(|file| self.resets_to_shipped(file))
            .map(|file| (file.kind, file.id.clone()))
            .collect();
        for (kind, id) in &copies {
            self.forget_user_copy(*kind, id)?;
        }
        Ok(copies.len())
    }

    /// Drops `id` from the unreadable files once it is written anew or thrown away.
    pub(super) fn readable_again(&mut self, kind: Kind, id: &str) {
        self.unreadable
            .retain(|file| file.kind != kind || file.id != id);
    }
}
