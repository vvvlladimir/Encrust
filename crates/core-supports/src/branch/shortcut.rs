use super::{Forest, SAME_PLACE_MM};
use crate::clear::{Beam, clear};
use crate::placed::Placed;
use core_geometry::{Scalar, Vec3};

impl Forest<'_> {
    /// Runs every branch a joint would lead back the way it came straight on to the joint under
    /// that one, where the strut stays within every group's lean and length and clear (ADR 0430).
    pub(super) fn shortcut(&mut self, placed: &Placed) {
        let reach_per_drop = self.strictest(|profile| profile.branching.reach_per_drop());
        let longest_mm = self.strictest(|profile| profile.branching.max_strut_length_mm);
        let clearance_mm = self
            .profiles
            .groups()
            .map(|(_, profile)| profile.clearance_mm)
            .fold(0.0, Scalar::max);

        let mut children = vec![Vec::new(); self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                children[parent].push(index);
            }
        }
        let mut joints: Vec<usize> = (0..self.nodes.len())
            .filter(|&joint| self.nodes[joint].parent.is_some() && children[joint].len() >= 2)
            .collect();
        joints.sort_by(|a, b| {
            let z = |node: &usize| self.nodes[*node].position.z;
            z(b).total_cmp(&z(a))
        });

        for joint in joints {
            let Some(under) = self.nodes[joint].parent else {
                continue;
            };
            let (at, to) = (self.nodes[joint].position, self.nodes[under].position);
            let reaches = |from: &crate::tree::TreeNode| {
                let drop_mm = from.position.z - to.z;
                let strut = Beam {
                    from: from.position,
                    to,
                    from_radius_mm: from.radius_mm,
                    to_radius_mm: from.radius_mm,
                };
                !from.is_leaf()
                    && drop_mm > 0.0
                    && (from.position - to).truncate().length() <= drop_mm * reach_per_drop
                    && from.position.distance(to) <= longest_mm
                    && clear(placed, &strut.with_clearance(clearance_mm))
            };
            let skip = |forest: &mut Self, branch: usize, children: &mut [Vec<usize>]| {
                forest.nodes[branch].parent = Some(under);
                children[joint].retain(|&child| child != branch);
                children[under].push(branch);
            };
            for branch in children[joint].clone() {
                let from = &self.nodes[branch];
                if turns_back(from.position, at, to) && reaches(from) {
                    skip(self, branch, &mut children);
                }
            }
            // A joint left with one branch is a bend in it, straightened where it can be.
            if let [last] = children[joint][..]
                && reaches(&self.nodes[last])
            {
                skip(self, last, &mut children);
            }
            // A joint left holding nothing is no longer part of any support.
            if children[joint].is_empty() {
                self.nodes[joint].parent = None;
                children[under].retain(|&child| child != joint);
            }
        }
    }

    /// The least of `limit` over every group, which a strut shared between them keeps to.
    fn strictest(&self, limit: impl Fn(&printer_profiles::SupportProfile) -> Scalar) -> Scalar {
        self.profiles
            .groups()
            .map(|(_, profile)| limit(profile))
            .fold(Scalar::INFINITY, Scalar::min)
    }
}

/// Whether a branch from `from` through `at` on to `to` turns back past square across the plate.
fn turns_back(from: Vec3, at: Vec3, to: Vec3) -> bool {
    let into = (at - from).truncate();
    let out = (to - at).truncate();
    into.length() >= SAME_PLACE_MM && out.length() >= SAME_PLACE_MM && into.dot(out) < 0.0
}
