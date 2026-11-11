use super::carriers::{THICKER_BY_MM, meeting_on};
use super::harvest::stands;
use super::speculate::Judged;
use super::{Forest, Front, MOST_BRANCHES, SAME_PLACE_MM};
use crate::Landing;
use crate::clear::{Beam, clear};
use crate::column::{PLATE_Z_MM, stands_clear};
use crate::placed::Placed;
use crate::tree::TreeNode;
use core_geometry::{Scalar, Vec2, Vec3};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// How far both fronts of a refused merge first drop straight down before a retry, mm, each
/// retry twice the last: what blocks a strut is mostly just under the tips (ADR 0338).
pub(super) const DROP_STEP_MM: Scalar = 1.0;

/// The drop a refused merge at `drop_mm` is tried again at: doubling, so a pair standing
/// `h` mm up is retried about log₂ h times before it meets the plate.
fn lower(drop_mm: Scalar) -> Scalar {
    (2.0 * drop_mm).max(DROP_STEP_MM)
}

/// Two fronts that could merge, or a front and the carrier it could run into, and how
/// high up they would meet.
#[derive(Debug, Clone, Copy)]
pub(super) struct Candidate {
    pub(super) meet: Vec3,
    pub(super) a: usize,
    /// The other front, or the carrier when `onto_carrier`.
    pub(super) b: usize,
    pub(super) onto_carrier: bool,
    /// How far both fronts drop straight down before they lean towards `meet`,
    /// millimetres.
    pub(super) drop_mm: Scalar,
}

// Ordered by meeting-point height so the heap yields the biggest saving first; heights are
// finite, so `total_cmp` needs no tie-break.
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.meet
            .z
            .total_cmp(&other.meet.z)
            .then_with(|| other.a.cmp(&self.a))
            .then_with(|| other.b.cmp(&self.b))
            .then_with(|| self.onto_carrier.cmp(&other.onto_carrier))
            .then_with(|| other.drop_mm.total_cmp(&self.drop_mm))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Candidate {}

/// Where `front` leans from once it has dropped `drop_mm` straight down.
pub(super) fn dropped(front: &Front, drop_mm: Scalar) -> Vec3 {
    front.position - Vec3::Z * drop_mm
}

/// The highest point both `a` and `b` reach within `reach_per_drop`, the ground a strut covers
/// per mm of descent, solved in the vertical plane through them.
pub(super) fn meeting_point(a: Vec3, b: Vec3, reach_per_drop: Scalar) -> Vec3 {
    let offset = Vec2::new(b.x - a.x, b.y - a.y);
    let ground = offset.length();
    if ground < SAME_PLACE_MM {
        return Vec3::new(a.x, a.y, a.z.min(b.z));
    }

    // Where along the ground between them the two descents meet: the midpoint, shifted
    // towards the lower of the two by however much head start the higher one has.
    let from_a = Scalar::midpoint(ground, (a.z - b.z) * reach_per_drop).clamp(0.0, ground);
    let z = (a.z - from_a / reach_per_drop).min(b.z - (ground - from_a) / reach_per_drop);

    let fraction = from_a / ground;
    Vec3::new(a.x + offset.x * fraction, a.y + offset.y * fraction, z)
}

impl Forest<'_> {
    /// Merges what can be merged, highest meeting point first.
    pub(super) fn merge(&mut self, placed: &Placed) {
        let mut heap = BinaryHeap::new();
        let mut judged = Judged::default();
        for index in 0..self.fronts.len() {
            for other in self.neighbours(index) {
                if other < index {
                    self.offer(&mut heap, index, other, 0.0);
                }
            }
            self.offer_carriers(&mut heap, index);
        }

        while let Some(candidate) = heap.pop() {
            if candidate.onto_carrier {
                if self.fronts[candidate.a].alive
                    && self.has_room(candidate.a, candidate.b)
                    && self.reaches(&candidate, placed)
                {
                    self.land_on_carrier(&candidate);
                }
                continue;
            }
            if !self.fronts[candidate.a].alive || !self.fronts[candidate.b].alive {
                continue;
            }
            if !self.accepted(&candidate, &mut heap, &mut judged, placed) {
                self.offer(
                    &mut heap,
                    candidate.a,
                    candidate.b,
                    lower(candidate.drop_mm),
                );
                continue;
            }

            let merged = self.join(&candidate);
            for other in self.neighbours(merged) {
                self.offer(&mut heap, merged, other, 0.0);
            }
            self.offer_carriers(&mut heap, merged);
        }
    }

    /// Offers the heap every carrier front `a` could run into, and where.
    pub(super) fn offer_carriers(&self, heap: &mut BinaryHeap<Candidate>, a: usize) {
        let front = &self.fronts[a];
        if !front.alive {
            return;
        }
        let profile = self.profile(a);
        for b in self.carriers.near(front.position) {
            let carrier = &self.carriers.all[b];
            if carrier.radius_mm < front.radius_mm + THICKER_BY_MM || !self.has_room(a, b) {
                continue;
            }
            if let Some(meet) = meeting_on(front.position, carrier, profile) {
                heap.push(Candidate {
                    meet,
                    a,
                    b,
                    onto_carrier: true,
                    drop_mm: 0.0,
                });
            }
        }
    }

    /// Whether the strut from a front to the carrier it would run into clears the model.
    pub(super) fn reaches(&self, candidate: &Candidate, placed: &Placed) -> bool {
        let front = &self.fronts[candidate.a];
        let profile = self.profile(candidate.a);
        let strut = Beam {
            from: front.position,
            to: candidate.meet,
            from_radius_mm: front.radius_mm,
            to_radius_mm: front.radius_mm,
        }
        .with_clearance(profile.clearance_mm);
        clear(placed, &strut)
    }

    /// Whether the trunk under `carrier` can take on the tips of front `a` within both
    /// groups' tips a trunk carries.
    pub(super) fn has_room(&self, a: usize, carrier: usize) -> bool {
        let tree = self.carriers.all[carrier].tree;
        let (own, most) = self.carriers.trunks[tree];
        let mine = self.profile(a).branching.max_tips_per_trunk as usize;
        let landed = self.carried.get(&tree).copied().unwrap_or(0);
        own + landed + self.fronts[a].tips <= most.min(mine)
    }

    /// Ends a front on the carrier of `candidate`: it needs no trunk and no foot.
    pub(super) fn land_on_carrier(&mut self, candidate: &Candidate) {
        let tree = self.carriers.all[candidate.b].tree;
        *self.carried.entry(tree).or_default() += self.fronts[candidate.a].tips;
        self.fronts[candidate.a].alive = false;
        self.landed.push((
            candidate.a,
            Landing {
                base: candidate.meet,
                on_model: true,
                pad_offset: Vec2::ZERO,
                pad_cut: None,
                on_support: true,
                in_plate: false,
            },
        ));
    }

    /// Works out where `a` and `b` meet once both drop `drop_mm` straight down and offers it to the
    /// heap; fronts of two groups meet within both groups' angles, distances, lengths and tips.
    pub(super) fn offer(
        &self,
        heap: &mut BinaryHeap<Candidate>,
        a: usize,
        b: usize,
        drop_mm: Scalar,
    ) {
        let (one, two) = (&self.fronts[a], &self.fronts[b]);
        let (first, second) = (&self.profile(a).branching, &self.profile(b).branching);
        let ground = Vec2::new(
            two.position.x - one.position.x,
            two.position.y - one.position.y,
        );
        let most_tips = first.max_tips_per_trunk.min(second.max_tips_per_trunk);
        if ground.length()
            > first
                .max_merge_distance_mm
                .min(second.max_merge_distance_mm)
            || one.tips + two.tips > most_tips as usize
        {
            return;
        }
        let reach_per_drop = first.reach_per_drop().min(second.reach_per_drop());
        let (from_one, from_two) = (dropped(one, drop_mm), dropped(two, drop_mm));
        let longest_mm = first.max_strut_length_mm.min(second.max_strut_length_mm);
        let meet = meeting_point(from_one, from_two, reach_per_drop);
        if !self.builds(
            meet,
            [(one, from_one), (two, from_two)],
            drop_mm,
            longest_mm,
        ) {
            return;
        }
        heap.push(Candidate {
            meet,
            a,
            b,
            onto_carrier: false,
            drop_mm,
        });
    }

    /// Whether a joint at `meet` can be built from `fronts`, each leaning from where it dropped:
    /// no branch longer than `longest_mm`, nor longer than the joint stands over the plate (0432).
    fn builds(
        &self,
        meet: Vec3,
        fronts: [(&Front, Vec3); 2],
        drop_mm: Scalar,
        longest_mm: Scalar,
    ) -> bool {
        // Two branches of no length are a merge that has already happened. One is the other
        // front coming down onto this one, which is a joint unless it is a contact or full.
        let branch_mm = |from: Vec3| (from - meet).length();
        let onto_full = |(front, from): &(&Front, Vec3)| {
            branch_mm(*from) < SAME_PLACE_MM
                && (self.nodes[front.node].is_leaf()
                    || (drop_mm <= 0.0 && front.branches >= MOST_BRANCHES))
        };
        let longest_branch_mm = branch_mm(fronts[0].1).max(branch_mm(fronts[1].1));
        longest_branch_mm >= SAME_PLACE_MM
            && !fronts.iter().any(onto_full)
            && longest_branch_mm <= longest_mm
            && meet.z - PLATE_Z_MM >= longest_branch_mm
    }

    /// Whether the two struts of `candidate` can be built: clear of the model, and with
    /// room under the meeting point for a trunk to stand.
    pub(super) fn accepts(&self, candidate: &Candidate, placed: &Placed) -> bool {
        let (a, b) = (&self.fronts[candidate.a], &self.fronts[candidate.b]);
        let joint_mm = a.radius_mm.max(b.radius_mm);
        let clearance_mm = self
            .profile(candidate.a)
            .clearance_mm
            .max(self.profile(candidate.b).clearance_mm);
        let reaches = |front: &Front| {
            let from = dropped(front, candidate.drop_mm);
            let drop = Beam {
                from: front.position,
                to: from,
                from_radius_mm: front.radius_mm,
                to_radius_mm: front.radius_mm,
            };
            let beam = Beam {
                from,
                to: candidate.meet,
                from_radius_mm: front.radius_mm,
                to_radius_mm: joint_mm,
            }
            .with_clearance(clearance_mm);
            let leans = from.distance(candidate.meet) >= SAME_PLACE_MM;
            stands_clear(placed, &drop, self.profile(candidate.a))
                && (!leans || clear(placed, &beam))
        };

        let tips = a.tips + b.tips;
        let under = self.profiles.of(self.profiles.heavier(a.group, b.group));
        let branch_mm = |front: &Front| dropped(front, candidate.drop_mm).distance(candidate.meet);
        reaches(a)
            && reaches(b)
            && stands(
                candidate.meet,
                joint_mm,
                tips,
                placed,
                under,
                branch_mm(a).max(branch_mm(b)),
            )
    }

    /// Hangs both fronts of `candidate` off a new node at the meeting point; the joint keeps the
    /// branches' width, only the trunk widens (ADR 0242), built to the heavier head.
    pub(super) fn join(&mut self, candidate: &Candidate) -> usize {
        let (a, b) = (self.fronts[candidate.a], self.fronts[candidate.b]);
        let radius_mm = a.radius_mm.max(b.radius_mm);

        // A front the other comes straight down onto is the joint itself.
        let onto = [a, b].into_iter().find(|front| {
            candidate.drop_mm <= 0.0 && front.position.distance(candidate.meet) < SAME_PLACE_MM
        });
        let node = if let Some(onto) = onto {
            self.nodes[onto.node].radius_mm = radius_mm;
            onto.node
        } else {
            self.nodes.push(TreeNode {
                position: candidate.meet,
                radius_mm,
                parent: None,
                point: None,
                head: None,
            });
            self.nodes.len() - 1
        };
        for front in [a, b].into_iter().filter(|front| front.node != node) {
            let from = self.dropped_node(&front, candidate.drop_mm, node);
            self.nodes[front.node].parent = Some(from);
        }
        self.fronts[candidate.a].alive = false;
        self.fronts[candidate.b].alive = false;

        self.push_front(Front {
            position: candidate.meet,
            radius_mm,
            tips: a.tips + b.tips,
            group: self.profiles.heavier(a.group, b.group),
            node,
            branches: onto.map_or(2, |onto| onto.branches + 1),
            alive: true,
        })
    }

    /// The node `front` leans to `parent` from: a new one `drop_mm` straight under it, or
    /// its own where it does not drop.
    pub(super) fn dropped_node(&mut self, front: &Front, drop_mm: Scalar, parent: usize) -> usize {
        let from = dropped(front, drop_mm);
        if drop_mm <= 0.0 || from.distance(self.nodes[parent].position) < SAME_PLACE_MM {
            return parent;
        }
        let node = self.nodes.len();
        self.nodes.push(TreeNode {
            position: from,
            radius_mm: front.radius_mm,
            parent: Some(parent),
            point: None,
            head: None,
        });
        node
    }
}

#[cfg(test)]
mod tests {
    use super::super::carriers::Carriers;
    use super::*;
    use crate::fixtures::{box_mesh, profile};
    use crate::{Profiles, SupportPoint, columns};
    use core_geometry::{Bvh, Mesh, Transform};

    /// The merges `offer` puts up for the first two tips of `at` in clear air, the first one's
    /// front holding `branches`.
    fn offered(at: [Vec3; 2], branches: usize) -> Vec<Candidate> {
        let profile = profile();
        let profiles = Profiles::single(&profile);
        let (model, carriers) = (Mesh::default(), Carriers::new(12.0));
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let points = at.map(SupportPoint::new);
        let mut forest = Forest::seeded(&columns(&points, &placed, profiles), profiles, &carriers);
        forest.fronts[0].branches = branches;
        let mut heap = BinaryHeap::new();
        forest.offer(&mut heap, 0, 1, 0.0);
        heap.into_vec()
    }

    #[test]
    fn a_joint_lower_over_the_plate_than_its_branches_are_long_is_not_offered() {
        // Necks 2 mm under the tips and 8 mm apart meet 4 mm lower, a millimetre over the
        // plate on branches 5.7 mm long.
        let low = [Vec3::new(-4.0, 0.0, 7.0), Vec3::new(4.0, 0.0, 7.0)];
        let high = low.map(|tip| tip + Vec3::Z * 13.0);

        assert!(offered(low, 1).is_empty(), "a joint 1 mm up is no saving");
        assert_eq!(
            offered(high, 1).len(),
            1,
            "the same pair 13 mm higher merges"
        );
    }

    #[test]
    fn a_tip_comes_down_onto_a_neck_only_while_it_holds_under_three_branches() {
        // The higher tip reaches the lower one's column above its neck.
        let at = [Vec3::new(0.0, 0.0, 20.0), Vec3::new(3.0, 0.0, 25.0)];

        assert_eq!(
            offered(at, 1).len(),
            1,
            "a neck holding its tip takes a branch"
        );
        assert!(
            offered(at, 3).is_empty(),
            "a joint holding three takes no more"
        );
    }

    #[test]
    fn a_trunk_shorter_over_the_part_under_it_than_its_branches_does_not_stand() {
        let profile = profile();
        let shelf = box_mesh(Vec3::new(-10.0, -10.0, 8.0), Vec3::new(10.0, 10.0, 10.0));
        let bvh = Bvh::build(&shelf);
        let placed = Placed::new(&shelf, &bvh, Transform::default());
        let stands_at = |z: Scalar| stands(Vec3::new(0.0, 0.0, z), 0.4, 2, &placed, &profile, 5.0);

        assert!(
            !stands_at(12.0),
            "2 mm over the shelf is less than a 5 mm branch"
        );
        assert!(stands_at(16.0), "6 mm over it is more");
    }

    #[test]
    fn a_pair_refused_80_mm_up_is_retried_eight_times_before_the_plate() {
        let drops: Vec<Scalar> = std::iter::successors(Some(0.0), |drop| Some(lower(*drop)))
            .take_while(|drop| *drop < 80.0)
            .collect();
        assert_eq!(drops, [0.0, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]);
    }
}
