use super::{ModelSupports, next_revision};
use crate::{Part, Profiles, SupportPoint, SupportTree, standing_on};
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
        self.freeze_all(&[tree], transform)
            .first()
            .copied()
            .flatten()
    }

    /// [`Self::freeze`] for `tree` and every support standing on it, which a hand carries
    /// with it (ADR 0433); answers `tree`'s index in [`ModelSupports::frozen`].
    pub fn freeze_with_riders(
        &mut self,
        tree: usize,
        transform: Transform,
        profiles: Profiles,
    ) -> Option<usize> {
        let mut trees = vec![tree];
        trees.extend(standing_on(self.trees(), tree, profiles));
        self.freeze_all(&trees, transform)
            .first()
            .copied()
            .flatten()
    }

    /// Freezes each of `trees`, indices into [`ModelSupports::trees`], at once: freezing one
    /// renumbers the rest. Answers each one's frozen index, in the order asked.
    fn freeze_all(&mut self, trees: &[usize], transform: Transform) -> Vec<Option<usize>> {
        let (Some(inverse), Some(built)) = (transform.inverse_matrix(), self.built.as_ref()) else {
            return vec![None; trees.len()];
        };
        let grown = built.grown;
        let mut lifted: Vec<usize> = trees.iter().copied().filter(|tree| *tree < grown).collect();
        lifted.sort_unstable();
        lifted.dedup();
        if lifted.is_empty() {
            return trees.iter().map(|tree| self.frozen_of(*tree)).collect();
        }

        let first = self.frozen.len();
        let standing: Vec<SupportTree> = lifted
            .iter()
            .map(|tree| built.trees[*tree].moved(inverse))
            .collect();
        let answer = trees
            .iter()
            .map(|tree| match lifted.binary_search(tree) {
                Ok(at) => Some(first + at),
                Err(_) => self.frozen_of(*tree),
            })
            .collect();

        let mut taken: Vec<usize> = standing.iter().flat_map(SupportTree::points).collect();
        taken.sort_unstable();
        taken.dedup();
        for point in taken.iter().rev() {
            if *point < self.points.len() {
                self.points.remove(*point);
            }
        }
        self.lift_out_of_forest(&lifted, grown, &taken);
        self.revision = next_revision();
        self.frozen.extend(standing);
        self.built = None;
        answer
    }

    /// Takes grown trees `lifted`, sorted, out of the forest and renumbers the points the others
    /// hold past the `taken` ones, so freezing some supports leaves every other where it stood.
    pub(super) fn lift_out_of_forest(&mut self, lifted: &[usize], grown: usize, taken: &[usize]) {
        let Some(forest) = self
            .forest
            .as_mut()
            .filter(|forest| forest.trees.len() == grown)
        else {
            self.forest = None;
            return;
        };
        for tree in lifted.iter().rev() {
            forest.trees.remove(*tree);
        }
        for kept in &mut forest.trees {
            kept.close_up_points(taken);
        }
        for lost in &mut forest.lost {
            lost.point -= taken.partition_point(|taken| *taken < lost.point);
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
