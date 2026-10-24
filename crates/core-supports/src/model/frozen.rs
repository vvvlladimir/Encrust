use super::{ModelSupports, next_revision};
use crate::{Part, SupportPoint, SupportTree};
use core_geometry::Transform;

impl ModelSupports {
    /// Which frozen tree a tree of [`ModelSupports::trees`] is, or `None` when it was
    /// grown from a point and is nobody's to edit until it has been frozen.
    pub fn frozen_of(&self, tree: usize) -> Option<usize> {
        let built = self.built.as_ref()?;
        tree.checked_sub(built.grown)
            .filter(|frozen| *frozen < self.frozen.len())
    }

    /// The trees the hand has taken hold of, in the model's own space.
    pub fn frozen(&self) -> &[SupportTree] {
        &self.frozen
    }

    /// Takes a grown tree out of the automatic run so a rebuild cannot undo an edit; returns its
    /// index in [`ModelSupports::frozen`] or `None`; its points go with it.
    pub fn freeze(&mut self, tree: usize, transform: Transform) -> Option<usize> {
        if let Some(frozen) = self.frozen_of(tree) {
            return Some(frozen);
        }
        let inverse = transform.inverse_matrix()?;
        let built = self.built.as_ref()?;
        let standing = built.trees.get(tree)?.clone();
        let grown = built.grown;

        let mut taken: Vec<usize> = standing.points().collect();
        taken.sort_unstable();
        for point in taken.iter().rev() {
            if *point < self.points.len() {
                self.points.remove(*point);
            }
        }
        self.lift_out_of_forest(tree, grown, &taken);
        self.revision = next_revision();

        self.frozen.push(standing.moved(inverse));
        self.built = None;
        Some(self.frozen.len() - 1)
    }

    /// Takes grown tree `tree` out of the forest and renumbers the points the others hold
    /// past the `taken` ones, so freezing one support leaves every other where it stood.
    pub(super) fn lift_out_of_forest(&mut self, tree: usize, grown: usize, taken: &[usize]) {
        let Some(forest) = self
            .forest
            .as_mut()
            .filter(|forest| forest.trees.len() == grown)
        else {
            self.forest = None;
            return;
        };
        forest.trees.remove(tree);
        for kept in &mut forest.trees {
            kept.close_up_points(taken);
        }
    }

    /// The frozen tree `frozen`, to be edited in the model's own space.
    pub fn frozen_mut(&mut self, frozen: usize) -> Option<&mut SupportTree> {
        self.built = None;
        self.frozen.get_mut(frozen)
    }

    /// Hands a frozen tree back to the automatic run: its tips become points again and
    /// the next rebuild grows them afresh.
    pub fn thaw(&mut self, frozen: usize, transform: Transform) {
        if frozen >= self.frozen.len() {
            return;
        }
        if transform.inverse_matrix().is_none() {
            return;
        }
        let tree = self.frozen.remove(frozen);
        for (contact, group) in tree.contacts().zip(tree.tip_groups()) {
            self.points.push(SupportPoint::new(contact).in_group(group));
        }
        self.points_changed();
    }

    /// Takes a frozen support away for good.
    pub fn remove_frozen(&mut self, frozen: usize) {
        if frozen < self.frozen.len() {
            self.frozen.remove(frozen);
            self.built = None;
        }
    }

    /// Takes `parts` of a frozen support away with what they hold up, all of it if nothing is left
    /// to hold or its trunk or foot was named; returns whether it is still there.
    pub fn remove_parts(&mut self, frozen: usize, parts: &[Part]) -> bool {
        let Some(tree) = self.frozen.get(frozen) else {
            return false;
        };
        let whole = parts
            .iter()
            .any(|part| matches!(part, Part::Trunk | Part::Foot));
        let cut: Vec<usize> = parts
            .iter()
            .filter_map(|part| match part {
                Part::Node(node) | Part::Strut(node) => Some(*node),
                Part::Trunk | Part::Foot => None,
            })
            .collect();
        let left = if whole { None } else { tree.without(&cut) };

        self.built = None;
        if let Some(left) = left {
            self.frozen[frozen] = left;
            return true;
        }
        self.frozen.remove(frozen);
        false
    }
}
