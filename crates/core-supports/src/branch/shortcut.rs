use super::{Forest, MOST_BRANCHES, SAME_PLACE_MM};
use crate::clear::{Beam, clear};
use crate::column::PLATE_Z_MM;
use crate::placed::Placed;
use core_geometry::{Scalar, Vec3};

impl Forest<'_> {
    /// Runs every branch a joint would lead back straight on to the joint under, within every
    /// group's lean, length and clearance, onto room and height for it (ADR 0428, 0430).
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
                let strut_mm = from.position.distance(to);
                !from.is_leaf()
                    && drop_mm > 0.0
                    && (from.position - to).truncate().length() <= drop_mm * reach_per_drop
                    && strut_mm <= longest_mm
                    && to.z - PLATE_Z_MM >= strut_mm
                    && clear(placed, &strut.with_clearance(clearance_mm))
            };
            let skip = |forest: &mut Self, branch: usize, children: &mut [Vec<usize>]| {
                forest.nodes[branch].parent = Some(under);
                children[joint].retain(|&child| child != branch);
                children[under].push(branch);
            };
            for branch in children[joint].clone() {
                let from = &self.nodes[branch];
                let room = children[under].len() < MOST_BRANCHES;
                if room && turns_back(from.position, at, to) && reaches(from) {
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

#[cfg(test)]
mod tests {
    use super::super::carriers::Carriers;
    use super::*;
    use crate::fixtures::profile;
    use crate::group::Profiles;
    use crate::tree::TreeNode;
    use core_geometry::{Bvh, Mesh, Transform};

    /// Where the branch over a joint at `under_z` + 3 mm hangs once shortcut: a joint at
    /// `under_z` holds `others` branches beside it, and the branch would lead back 1 mm.
    fn shortcut_from(under_z: Scalar, others: usize) -> Option<usize> {
        let profile = profile();
        let (model, carriers) = (Mesh::default(), Carriers::new(12.0));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let mut forest = Forest::seeded(&[], Profiles::single(&profile), &carriers);
        let node = |x: Scalar, y: Scalar, up_mm: Scalar, parent: Option<usize>| TreeNode {
            position: Vec3::new(x, y, under_z + up_mm),
            radius_mm: 0.4,
            parent,
            point: None,
            head: None,
        };
        forest.nodes.push(node(0.0, 0.0, 0.0, None));
        for other in 0..others {
            forest.nodes.push(node(-3.0, other as Scalar, 3.0, Some(0)));
        }
        let joint = forest.nodes.len();
        forest.nodes.push(node(2.0, 0.0, 3.0, Some(0)));
        let back = forest.nodes.len();
        forest.nodes.push(node(1.0, 0.0, 5.0, Some(joint)));
        forest.nodes.push(node(4.0, 0.0, 5.0, Some(joint)));

        forest.shortcut(&placed);
        forest.nodes[back].parent
    }

    #[test]
    fn a_branch_led_back_runs_on_to_the_joint_under_only_while_that_has_room() {
        assert_eq!(
            shortcut_from(10.0, 1),
            Some(0),
            "a joint of two takes a third"
        );
        assert_eq!(
            shortcut_from(10.0, 2),
            Some(3),
            "a joint of three takes no fourth"
        );
    }

    #[test]
    fn a_branch_led_back_is_not_run_onto_a_joint_lower_over_the_plate_than_it_is_long() {
        assert_eq!(
            shortcut_from(3.0, 1),
            Some(2),
            "5.1 mm of branch onto a joint 3 mm up"
        );
    }
}
