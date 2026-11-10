//! Which edits the history holds, for what is said about one of them after it is made.

use super::{History, fingerprint};
use crate::scene::Scene;

impl History {
    /// Whether the plate differs from the last edit recorded: what a press just did will be
    /// recorded under [`Self::next_edit`].
    pub fn has_unrecorded(&self, scene: &Scene) -> bool {
        fingerprint(scene) != self.fingerprint
    }

    /// The last edit recorded that an undo would take back.
    pub fn last_edit(&self) -> Option<u64> {
        self.past.back().map(|(id, _)| *id)
    }

    /// Whether `edit` is still to be recorded, is recorded, or waits for its redo.
    pub fn knows(&self, edit: u64) -> bool {
        edit == self.next
            || self.past.iter().any(|(id, _)| *id == edit)
            || self.future.iter().any(|(id, _)| *id == edit)
    }

    /// `edit` and every edit recorded after it, or `None` if it has been undone or lost; an
    /// edit still to be recorded has none yet.
    pub fn edits_since(&self, edit: u64) -> Option<impl Iterator<Item = u64> + '_> {
        let recorded = self.past.iter().any(|(id, _)| *id == edit);
        (recorded || edit == self.next).then(|| {
            self.past
                .iter()
                .map(|(id, _)| *id)
                .filter(move |id| *id >= edit)
        })
    }
}
