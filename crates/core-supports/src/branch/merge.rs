use super::carriers::{THICKER_BY_MM, meeting_on};
use super::ground::escape;
use super::harvest::stands;
use super::speculate::Judged;
use super::{Forest, Front, SAME_PLACE_MM};
use crate::Landing;
use crate::clear::{Beam, clear};
use crate::column::{MAX_LANDING_SLOPE_DEG, PLATE_Z_MM, landing, stands_clear};
use crate::placed::Placed;
use crate::tree::TreeNode;
use core_geometry::{Scalar, Vec2, Vec3};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// How far both fronts of a refused merge drop straight down before a retry, mm: what blocks a
/// strut is mostly just under the tips (ADR 0347).
pub(super) const DROP_STEP_MM: Scalar = 1.0;

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
                let lower = candidate.drop_mm + DROP_STEP_MM;
                self.offer(&mut heap, candidate.a, candidate.b, lower);
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
        let meet = meeting_point(from_one, from_two, reach_per_drop);
        let longest_mm = first.max_strut_length_mm.min(second.max_strut_length_mm);

        // Two struts of no length are a merge that has already happened. One is the other
        // front coming down onto this one, which is a joint unless this one is a contact.
        let strut_mm = |from: Vec3| (from - meet).length();
        let onto_contact = |front: &Front, from: Vec3| {
            strut_mm(from) < SAME_PLACE_MM && self.nodes[front.node].is_leaf()
        };
        if strut_mm(from_one).max(strut_mm(from_two)) < SAME_PLACE_MM
            || onto_contact(one, from_one)
            || onto_contact(two, from_two)
            || strut_mm(from_one).max(strut_mm(from_two)) > longest_mm
            || meet.z <= PLATE_Z_MM
        {
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
        reaches(a) && reaches(b) && stands(candidate.meet, joint_mm, tips, placed, under)
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

    /// Bends every front the model leaves no room under onto a knee, and carries on from
    /// there. Answers whether any of them moved.
    pub(super) fn bend_blocked(&mut self, placed: &Placed) -> bool {
        // Where each front bends is its own question, so they are asked at once.
        let knees: Vec<(usize, Vec3)> = (0..self.fronts.len())
            .into_par_iter()
            .filter_map(|index| Some((index, self.knee(index, placed)?)))
            .collect();
        let bent = !knees.is_empty();
        for (index, knee) in knees {
            let front = self.fronts[index];
            let node = self.nodes.len();
            self.nodes.push(TreeNode {
                position: knee,
                radius_mm: front.radius_mm,
                parent: None,
                point: None,
                head: None,
            });
            self.nodes[front.node].parent = Some(node);
            self.fronts[index].alive = false;
            self.push_front(Front {
                position: knee,
                node,
                alive: true,
                ..front
            });
        }
        bent
    }

    /// Where front `index` bends to with no room under it, or `None` if it has room or nowhere.
    fn knee(&self, index: usize, placed: &Placed) -> Option<Vec3> {
        let front = self.fronts[index];
        // A contact with no head stands as a small pillar onto the model under it.
        if !front.alive || self.nodes[front.node].is_leaf() {
            return None;
        }
        let profile = self.profile(index);
        if landing(placed, front.position, front.radius_mm, front.tips, profile).is_some() {
            return None;
        }
        escape(&front, placed, profile, MAX_LANDING_SLOPE_DEG).map(|(knee, _)| knee)
    }
}
