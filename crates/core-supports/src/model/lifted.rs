use super::ModelSupports;
use crate::SupportTree;

impl ModelSupports {
    /// Lifts frozen trees `frozen` out of the meshes while a hand carries them, so a step
    /// meshes only what moved (ADR 0438); none sets them down again, meshed with the rest.
    pub fn lift(&mut self, frozen: Vec<usize>) {
        if self.lifted != frozen {
            self.lifted = frozen;
            self.built = None;
        }
    }

    /// The frozen trees a hand is carrying, left out of [`ModelSupports::meshes`].
    pub fn lifted(&self) -> &[usize] {
        &self.lifted
    }

    /// Puts frozen tree `frozen` where a hand carried it. A lifted one is drawn by the hand, so
    /// nothing is built again for it; any other drops what was built.
    pub fn carry_to(&mut self, frozen: usize, tree: SupportTree) {
        if !self.lifted.contains(&frozen) {
            if let Some(slot) = self.frozen_mut(frozen) {
                *slot = tree;
            }
            return;
        }
        let Some(slot) = self.frozen.get_mut(frozen) else {
            return;
        };
        if let Some(built) = &mut self.built
            && let Some(standing) = built.trees.get_mut(built.grown + frozen)
        {
            *standing = tree.moved(built.transform.to_matrix());
        }
        *slot = tree;
    }

    /// The trees of `trees`, the first `grown` grown and the rest frozen, a rebuild meshes:
    /// all but the lifted ones.
    pub(super) fn meshed_of(&self, trees: &[SupportTree], grown: usize) -> Vec<SupportTree> {
        trees
            .iter()
            .enumerate()
            .filter(|(at, _)| {
                at.checked_sub(grown)
                    .is_none_or(|frozen| !self.lifted.contains(&frozen))
            })
            .map(|(_, tree)| tree.clone())
            .collect()
    }
}
