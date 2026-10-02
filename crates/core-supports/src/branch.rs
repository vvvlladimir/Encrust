use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::placed::Placed;
use core_geometry::{Scalar, Vec2, Vec3};
use printer_profiles::SupportProfile;

use crate::Landing;
use crate::clear::{Beam, clear};
use crate::column::{Column, MIN_PILLAR_HEIGHT_MM, landing};
use crate::group::Profiles;
use crate::tree::{SupportTree, TreeNode};

/// Two tips this close together stand in the same place, and the direction between them
/// carries no information. Well under one pixel of any panel.
const SAME_PLACE_MM: Scalar = 1e-4;

/// How many directions and how many reaches a blocked front leans in before it gives up.
/// Eight directions put a candidate every 45 degrees around it.
const ESCAPE_DIRECTIONS: u32 = 8;
const ESCAPE_REACHES: u32 = 3;

/// Merges the tips of `columns` into shared trunks, and answers with one tree per trunk.
///
/// Tips are merged greedily, highest meeting point first, which is the strategy of
/// *Clever Support: Efficient Support Structure Generation for Digital Fabrication*; see
/// `docs/decisions/0040`. With branching switched off every tip comes back as its own
/// one-node tree, which is the vertical column of step 7a.
///
/// `columns` are in plate coordinates, and `placed` puts the mesh the struts have to keep
/// clear of in the same space.
///
/// Each group is grown on its own, so a trunk is only ever shared by supports of one
/// shape; see `docs/decisions/0094`.
pub fn grow(columns: &[Column], placed: &Placed, profiles: Profiles) -> Vec<SupportTree> {
    let mut forest = Vec::new();
    for (group, profile) in profiles.groups() {
        let mine: Vec<Column> = columns
            .iter()
            .filter(|column| column.group == group)
            .copied()
            .collect();
        forest.extend(grow_one(&mine, placed, profile, group));
    }
    forest
}

/// Grows one group's columns, every one of which is built to `profile`.
fn grow_one(
    columns: &[Column],
    placed: &Placed,
    profile: &SupportProfile,
    group: u16,
) -> Vec<SupportTree> {
    let mut forest = Forest::seeded(columns, profile);
    if profile.branching.enabled {
        forest.merge(placed, profile);
        // Tips the model stands in the way of are bent aside before the second pass, so
        // that what they lean onto can share a trunk rather than stand alone.
        if forest.bend_blocked(placed, profile) {
            forest.merge(placed, profile);
        }
    }
    let mut trees = forest.harvest(placed, profile);
    for tree in &mut trees {
        tree.set_group(group);
    }
    trees
}

/// A tip or a trunk that has not been merged into anything yet.
#[derive(Debug, Clone, Copy)]
struct Front {
    position: Vec3,
    radius_mm: Scalar,
    node: usize,
    alive: bool,
}

/// Two fronts that could merge, and how high up they would.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    meet: Vec3,
    a: usize,
    b: usize,
}

// Ordered by the height of the meeting point, so the heap hands back the merge that
// saves the most first. Heights are finite by construction, which is why `total_cmp`
// needs no tie-break of its own.
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.meet
            .z
            .total_cmp(&other.meet.z)
            .then_with(|| other.a.cmp(&self.a))
            .then_with(|| other.b.cmp(&self.b))
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

/// Every node placed so far, and which of them are still looking for a partner.
struct Forest {
    nodes: Vec<TreeNode>,
    fronts: Vec<Front>,
    /// Fronts bucketed by the square of plate they stand over, so a new one asks nine
    /// buckets rather than every front placed so far. Dead fronts stay in their bucket
    /// and are skipped; there are never more entries than twice the number of tips.
    grid: HashMap<(i32, i32), Vec<usize>>,
    cell_mm: Scalar,
}

impl Forest {
    /// One front per column, each at its contact.
    fn seeded(columns: &[Column], profile: &SupportProfile) -> Self {
        let radius_mm = profile.pillar_radius_mm();
        let mut forest = Self {
            nodes: Vec::with_capacity(columns.len()),
            fronts: Vec::with_capacity(columns.len()),
            grid: HashMap::new(),
            cell_mm: profile.branching.max_merge_distance_mm.max(SAME_PLACE_MM),
        };

        // A tip leaves the surface square on, so the contact and the neck under it are
        // two nodes, and what merges is the neck; see `docs/decisions/0125`.
        for column in columns {
            let contact = forest.nodes.len();
            forest.nodes.push(TreeNode {
                position: column.contact,
                radius_mm,
                parent: Some(contact + 1),
                point: Some(column.point),
            });
            let node = forest.nodes.len();
            forest.nodes.push(TreeNode {
                position: column.neck,
                radius_mm,
                parent: None,
                point: None,
            });
            forest.push_front(Front {
                position: column.neck,
                radius_mm,
                node,
                alive: true,
            });
        }
        forest
    }

    fn push_front(&mut self, front: Front) -> usize {
        let index = self.fronts.len();
        let cell = self.cell_of(front.position);
        self.grid.entry(cell).or_default().push(index);
        self.fronts.push(front);
        index
    }

    fn cell_of(&self, position: Vec3) -> (i32, i32) {
        (
            (position.x / self.cell_mm).floor() as i32,
            (position.y / self.cell_mm).floor() as i32,
        )
    }

    /// Every live front in the nine buckets around `index`, barring `index` itself.
    fn neighbours(&self, index: usize) -> Vec<usize> {
        let (cx, cy) = self.cell_of(self.fronts[index].position);
        let mut found = Vec::new();
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                let Some(bucket) = self.grid.get(&(x, y)) else {
                    continue;
                };
                found.extend(
                    bucket
                        .iter()
                        .copied()
                        .filter(|&other| other != index && self.fronts[other].alive),
                );
            }
        }
        found
    }

    /// Merges what can be merged, highest meeting point first.
    fn merge(&mut self, placed: &Placed, profile: &SupportProfile) {
        let mut heap = BinaryHeap::new();
        for index in 0..self.fronts.len() {
            for other in self.neighbours(index) {
                if other < index {
                    self.offer(&mut heap, index, other, profile);
                }
            }
        }

        while let Some(candidate) = heap.pop() {
            if !self.fronts[candidate.a].alive || !self.fronts[candidate.b].alive {
                continue;
            }
            if !self.accepts(&candidate, placed, profile) {
                continue;
            }

            let merged = self.join(&candidate, profile);
            for other in self.neighbours(merged) {
                self.offer(&mut heap, merged, other, profile);
            }
        }
    }

    /// Works out where `a` and `b` would meet and offers it to the heap.
    fn offer(
        &self,
        heap: &mut BinaryHeap<Candidate>,
        a: usize,
        b: usize,
        profile: &SupportProfile,
    ) {
        let (one, two) = (&self.fronts[a], &self.fronts[b]);
        let ground = Vec2::new(
            two.position.x - one.position.x,
            two.position.y - one.position.y,
        );
        if ground.length() > profile.branching.max_merge_distance_mm {
            return;
        }
        let meet = meeting_point(
            one.position,
            two.position,
            profile.branching.reach_per_drop(),
        );

        // A strut of no length is a merge that has already happened, and a node on top of
        // another node is what the sweep cannot mesh.
        if (one.position - meet).length() < SAME_PLACE_MM
            || (two.position - meet).length() < SAME_PLACE_MM
        {
            return;
        }
        heap.push(Candidate { meet, a, b });
    }

    /// Whether the two struts of `candidate` can be built: clear of the model, and with
    /// room under the meeting point for a trunk to stand.
    fn accepts(&self, candidate: &Candidate, placed: &Placed, profile: &SupportProfile) -> bool {
        let (a, b) = (&self.fronts[candidate.a], &self.fronts[candidate.b]);
        let trunk_mm = profile.branching.merged_radius_mm(a.radius_mm, b.radius_mm);
        let reaches = |front: &Front| {
            let beam = Beam {
                from: front.position,
                to: candidate.meet,
                from_radius_mm: front.radius_mm,
                to_radius_mm: trunk_mm,
            }
            .with_clearance(profile.clearance_mm);
            clear(placed, &beam)
        };

        reaches(a) && reaches(b) && stands(candidate.meet, trunk_mm, placed, profile)
    }

    /// Hangs both fronts of `candidate` off a new node at the meeting point.
    fn join(&mut self, candidate: &Candidate, profile: &SupportProfile) -> usize {
        let (a, b) = (self.fronts[candidate.a], self.fronts[candidate.b]);
        let radius_mm = profile.branching.merged_radius_mm(a.radius_mm, b.radius_mm);

        let node = self.nodes.len();
        self.nodes.push(TreeNode {
            position: candidate.meet,
            radius_mm,
            parent: None,
            point: None,
        });
        self.nodes[a.node].parent = Some(node);
        self.nodes[b.node].parent = Some(node);
        self.fronts[candidate.a].alive = false;
        self.fronts[candidate.b].alive = false;

        self.push_front(Front {
            position: candidate.meet,
            radius_mm,
            node,
            alive: true,
        })
    }

    /// Bends every front the model leaves no room under onto a knee, and carries on from
    /// there. Answers whether any of them moved.
    fn bend_blocked(&mut self, placed: &Placed, profile: &SupportProfile) -> bool {
        let mut bent = false;
        for index in 0..self.fronts.len() {
            let front = self.fronts[index];
            if !front.alive {
                continue;
            }
            if landing(placed, front.position, front.radius_mm, profile).is_some() {
                continue;
            }
            let Some((knee, _)) = escape(front.position, front.radius_mm, placed, profile) else {
                continue;
            };

            let node = self.nodes.len();
            self.nodes.push(TreeNode {
                position: knee,
                radius_mm: front.radius_mm,
                parent: None,
                point: None,
            });
            self.nodes[front.node].parent = Some(node);
            self.fronts[index].alive = false;
            self.push_front(Front {
                position: knee,
                radius_mm: front.radius_mm,
                node,
                alive: true,
            });
            bent = true;
        }
        bent
    }

    /// Drops a trunk from every surviving front and splits the arena into one tree each.
    fn harvest(self, placed: &Placed, profile: &SupportProfile) -> Vec<SupportTree> {
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                children[parent].push(index);
            }
        }

        self.fronts
            .iter()
            .filter(|front| front.alive)
            .filter_map(|front| {
                if let Some(landing) = landing(placed, front.position, front.radius_mm, profile) {
                    return Some(self.cut(front.node, &children, landing));
                }
                let (knee, landing) = escape(front.position, front.radius_mm, placed, profile)?;
                let mut tree = self.cut(front.node, &children, landing);
                tree.stand_on_knee(knee, front.radius_mm);
                Some(tree)
            })
            .collect()
    }

    /// The tree hanging off `root`, with its nodes renumbered from zero.
    fn cut(&self, root: usize, children: &[Vec<usize>], landing: Landing) -> SupportTree {
        let mut order = vec![root];
        let mut index = 0;
        while index < order.len() {
            order.extend(children[order[index]].iter().copied());
            index += 1;
        }

        let mut local = HashMap::with_capacity(order.len());
        for (position, &node) in order.iter().enumerate() {
            local.insert(node, position);
        }
        let nodes = order
            .iter()
            .map(|&node| TreeNode {
                parent: self.nodes[node].parent.and_then(|parent| {
                    // The root's own parent is outside this tree, which is how it became
                    // a root in the first place.
                    local.get(&parent).copied()
                }),
                ..self.nodes[node]
            })
            .collect();

        SupportTree::new(nodes, 0, landing)
    }
}

/// Whether a trunk of `radius_mm` under `at` has anywhere to go: straight down, or
/// leaning aside the way a blocked front does.
fn stands(at: Vec3, radius_mm: Scalar, placed: &Placed, profile: &SupportProfile) -> bool {
    landing(placed, at, radius_mm, profile).is_some()
        || escape(at, radius_mm, placed, profile).is_some()
}

/// Where a support with nothing under it can lean to and still find a trunk's worth of
/// room, as the knee it bends at and what the trunk under that knee stands on.
///
/// The shortest reach that works is taken, so a support leans no further than it has to;
/// see `docs/decisions/0078`.
fn escape(
    from: Vec3,
    radius_mm: Scalar,
    placed: &Placed,
    profile: &SupportProfile,
) -> Option<(Vec3, Landing)> {
    let reach_per_drop = profile.branching.reach_per_drop();
    // A lean costs height, and what is left underneath is the ceiling on it: a support low
    // over the plate can only step aside as far as it can still descend.
    let room_mm = from.z - profile.base_height_mm() - MIN_PILLAR_HEIGHT_MM;
    let reach_mm = (room_mm * reach_per_drop).min(profile.branching.max_merge_distance_mm);
    if reach_mm <= SAME_PLACE_MM {
        return None;
    }

    for step in 1..=ESCAPE_REACHES {
        let ground = reach_mm * step as Scalar / ESCAPE_REACHES as Scalar;
        for turn in 0..ESCAPE_DIRECTIONS {
            let angle = std::f32::consts::TAU * turn as Scalar / ESCAPE_DIRECTIONS as Scalar;
            let knee = from
                + Vec3::new(
                    ground * angle.cos(),
                    ground * angle.sin(),
                    -ground / reach_per_drop,
                );
            let beam = Beam {
                from,
                to: knee,
                from_radius_mm: radius_mm,
                to_radius_mm: radius_mm,
            }
            .with_clearance(profile.clearance_mm);
            if !clear(placed, &beam) {
                continue;
            }
            if let Some(landing) = landing(placed, knee, radius_mm, profile) {
                return Some((knee, landing));
            }
        }
    }
    None
}

/// The highest point both `a` and `b` can reach without leaning further than
/// `reach_per_drop`, the ground a strut covers per millimetre it descends.
///
/// The two struts lie in the vertical plane through `a` and `b`, so the solve is on the
/// horizontal distance between them and their difference in height.
fn meeting_point(a: Vec3, b: Vec3, reach_per_drop: Scalar) -> Vec3 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SupportPoint;
    use crate::tests::{box_mesh, profile, ramp};
    use core_geometry::{Bvh, Mesh, Transform};

    /// The trees a set of contacts grows into, over an empty plate unless a model is
    /// given.
    fn forest_of(contacts: &[Vec3], model: &Mesh, profiles: Profiles) -> Vec<SupportTree> {
        let bvh = Bvh::build(model);
        let points: Vec<SupportPoint> = contacts.iter().copied().map(SupportPoint::new).collect();
        let columns = crate::columns(
            &points,
            &Placed::new(model, &bvh, Transform::default()),
            profiles,
        );
        grow(
            &columns,
            &Placed::new(model, &bvh, Transform::default()),
            profiles,
        )
    }

    #[test]
    fn two_tips_of_different_groups_never_share_a_trunk() {
        let mut thin = profile();
        thin.middle.diameter_mm *= 0.5;
        let table = [profile(), thin];
        let points = [
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(14.0, 10.0, 20.0)).in_group(1),
        ];

        let model = Mesh::default();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let profiles = Profiles::new(&table).expect("two groups");
        let forest = grow(
            &crate::columns(&points, &placed, profiles),
            &placed,
            profiles,
        );

        assert_eq!(forest.len(), 2, "two shapes cannot share one trunk");
        assert_eq!(
            forest.iter().map(SupportTree::group).collect::<Vec<_>>(),
            vec![0, 1],
            "each tree is stamped with the group it was built to"
        );
    }

    #[test]
    fn two_tips_side_by_side_share_one_trunk() {
        let forest = forest_of(
            &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
            &Mesh::default(),
            Profiles::single(&profile()),
        );

        assert_eq!(forest.len(), 1, "both tips hang off one trunk");
        assert_eq!(forest[0].tip_count(), 2);
    }

    #[test]
    fn two_tips_meet_halfway_between_them() {
        let forest = forest_of(
            &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
            &Mesh::default(),
            Profiles::single(&profile()),
        );
        let root = forest[0].root();

        assert!(
            (root.position.x - 12.0).abs() < 1e-4,
            "two tips at the same height meet over the middle, got {}",
            root.position.x
        );
        // Four millimetres apart at 45 degrees is two millimetres of drop, taken from the
        // necks the struts leave rather than from the contacts above them.
        let from_z = 20.0 - profile().top_length_mm();
        assert!(
            (root.position.z - (from_z - 2.0)).abs() < 1e-4,
            "a 45 degree lean drops as far as it reaches sideways, got {}",
            root.position.z
        );
    }

    #[test]
    fn two_tips_whose_struts_would_graze_the_model_do_not_merge() {
        // A ledge beside where the two struts run down to their meeting point: they pass
        // it by 0.6 mm, which is less than the body and the air it keeps.
        let ledge = box_mesh(Vec3::new(11.5, 10.6, 17.0), Vec3::new(12.5, 12.0, 19.0));
        let forest = forest_of(
            &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
            &ledge,
            Profiles::single(&profile()),
        );

        assert_eq!(
            forest.len(),
            2,
            "the same two tips share a trunk over an empty plate"
        );
        assert!(forest.iter().all(|tree| tree.tip_count() == 1));

        // The axis of that strut runs past the ledge in clear air: nothing but the body's
        // own width is what rejects the merge.
        assert!(clear(
            &Placed::new(&ledge, &Bvh::build(&ledge), Transform::default()),
            &Beam {
                from: Vec3::new(10.0, 10.0, 20.0),
                to: Vec3::new(12.0, 10.0, 18.0),
                from_radius_mm: 0.0,
                to_radius_mm: 0.0,
            }
        ));
    }

    #[test]
    fn a_tip_with_nothing_under_it_leans_until_it_finds_room() {
        // A face too steep to stand on, with the tip right over it. Straight down there
        // is nowhere to go, so the support bends a knee and steps off the ramp.
        let steep = ramp(4.0, 10.0);
        let contact = Vec3::new(2.0, 0.0, 20.0);
        let forest = forest_of(&[contact], &steep, Profiles::single(&profile()));

        assert_eq!(forest.len(), 1, "the tip is held");
        let tree = &forest[0];
        assert_eq!(tree.tip_count(), 1);
        assert!(!tree.landing().on_model, "it stepped off onto the plate");

        assert!(
            tree.root().point.is_none(),
            "the knee holds no tip of its own"
        );
        let knee = tree.root().position;
        assert!(
            (knee.truncate() - contact.truncate()).length() > 1.0,
            "the knee has to be somewhere else than under the contact, got {knee}"
        );
        assert!(knee.z < contact.z, "a lean descends");
    }

    #[test]
    fn tips_further_apart_than_the_merge_distance_stay_apart() {
        let profile = profile();
        let far = profile.branching.max_merge_distance_mm + 1.0;
        let forest = forest_of(
            &[
                Vec3::new(10.0, 10.0, 20.0),
                Vec3::new(10.0 + far, 10.0, 20.0),
            ],
            &Mesh::default(),
            Profiles::single(&profile),
        );

        assert_eq!(forest.len(), 2);
        assert!(forest.iter().all(|tree| tree.tip_count() == 1));
    }

    #[test]
    fn a_pair_that_would_meet_below_the_plate_stays_apart() {
        // Ten millimetres apart at 45 degrees needs five millimetres of drop, and these
        // tips stand three above the plate.
        let forest = forest_of(
            &[Vec3::new(10.0, 10.0, 3.0), Vec3::new(20.0, 10.0, 3.0)],
            &Mesh::default(),
            Profiles::single(&profile()),
        );
        assert_eq!(forest.len(), 2);
    }

    #[test]
    fn branching_switched_off_leaves_every_tip_its_own_column() {
        let mut profile = profile();
        profile.branching.enabled = false;
        let forest = forest_of(
            &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
            &Mesh::default(),
            Profiles::single(&profile),
        );

        assert_eq!(forest.len(), 2);
        assert!(
            forest.iter().all(|tree| tree.nodes().len() == 2),
            "a contact and the neck under it, and nothing merged onto either"
        );
    }

    #[test]
    fn a_wall_between_two_tips_keeps_them_apart() {
        // A slab standing between the two tips, so neither strut can reach the middle.
        let wall = box_mesh(Vec3::new(11.5, 0.0, 0.0), Vec3::new(12.5, 20.0, 30.0));
        let forest = forest_of(
            &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
            &wall,
            Profiles::single(&profile()),
        );
        assert_eq!(forest.len(), 2, "the slab is in the way of the merge");
    }

    #[test]
    fn a_shallower_angle_merges_lower_down() {
        let steep = SupportProfile {
            branching: printer_profiles::Branching {
                max_angle_deg: 60.0,
                ..profile().branching
            },
            ..profile()
        };
        let shallow = SupportProfile {
            branching: printer_profiles::Branching {
                max_angle_deg: 20.0,
                ..profile().branching
            },
            ..profile()
        };
        let contacts = [Vec3::new(10.0, 10.0, 40.0), Vec3::new(14.0, 10.0, 40.0)];

        let steep = forest_of(&contacts, &Mesh::default(), Profiles::single(&steep));
        let shallow = forest_of(&contacts, &Mesh::default(), Profiles::single(&shallow));
        assert!(
            shallow[0].root().position.z < steep[0].root().position.z,
            "a strut that may not lean far has to drop further to meet"
        );
    }

    #[test]
    fn a_trunk_is_thicker_than_the_branches_it_carries() {
        let profile = profile();
        let forest = forest_of(
            &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
            &Mesh::default(),
            Profiles::single(&profile),
        );
        assert!(forest[0].root().radius_mm > profile.top_lower_radius_mm());
    }

    #[test]
    fn four_tips_in_a_square_end_up_on_one_trunk() {
        let forest = forest_of(
            &[
                Vec3::new(10.0, 10.0, 30.0),
                Vec3::new(14.0, 10.0, 30.0),
                Vec3::new(10.0, 14.0, 30.0),
                Vec3::new(14.0, 14.0, 30.0),
            ],
            &Mesh::default(),
            Profiles::single(&profile()),
        );

        assert_eq!(forest.len(), 1);
        assert_eq!(forest[0].tip_count(), 4);
        assert_eq!(
            forest[0].nodes().len(),
            11,
            "four tips, a neck under each and three joints"
        );
    }

    #[test]
    fn every_point_comes_back_exactly_once() {
        let contacts: Vec<Vec3> = (0..16u8)
            .map(|index| {
                Vec3::new(
                    10.0 + f32::from(index % 4) * 3.0,
                    10.0 + f32::from(index / 4) * 3.0,
                    30.0,
                )
            })
            .collect();
        let forest = forest_of(&contacts, &Mesh::default(), Profiles::single(&profile()));

        let mut held: Vec<usize> = forest.iter().flat_map(SupportTree::points).collect();
        held.sort_unstable();
        assert_eq!(held, (0..16).collect::<Vec<_>>());
    }

    #[test]
    fn no_strut_leans_further_than_the_profile_allows() {
        let profile = profile();
        let contacts: Vec<Vec3> = (0..16u8)
            .map(|index| {
                Vec3::new(
                    10.0 + f32::from(index % 4) * 3.0,
                    10.0 + f32::from(index / 4) * 3.0,
                    30.0,
                )
            })
            .collect();

        for tree in forest_of(&contacts, &Mesh::default(), Profiles::single(&profile)) {
            for (child, parent) in tree.struts() {
                let offset = tree.nodes()[parent].position - tree.nodes()[child].position;
                let ground = Vec2::new(offset.x, offset.y).length();
                let drop = -offset.z;
                assert!(drop > 0.0, "a strut always descends, got {drop}");
                assert!(
                    ground <= drop * profile.branching.reach_per_drop() + 1e-3,
                    "a strut covering {ground} mm of ground must drop at least \
                     {} mm, got {drop}",
                    ground / profile.branching.reach_per_drop()
                );
            }
        }
    }

    #[test]
    fn no_contacts_grow_no_trees() {
        assert!(forest_of(&[], &Mesh::default(), Profiles::single(&profile())).is_empty());
    }
}
