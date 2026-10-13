use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::placed::Placed;
use core_geometry::{Ray, Scalar, Vec2, Vec3, raycast_placed};
use printer_profiles::SupportProfile;

use crate::Landing;
use crate::base::braceable;
use crate::clear::{Beam, clear};
use crate::column::{
    Column, LAST_RESORT_SLOPE_DEG, MAX_LANDING_SLOPE_DEG, MAX_LEAN_PER_DROP, MIN_PILLAR_HEIGHT_MM,
    PLATE_Z_MM, SURFACE_EPSILON_MM, landing, landing_within,
};
use crate::group::Profiles;
use crate::mesher::is_small;
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
/// Each group is grown on its own, so the tips of a trunk are all one shape (ADR 0094),
/// and thickest first: what a thicker group has stood is there for a thinner one to run
/// its struts into in place of trunks of its own, see `docs/decisions/0248`. The forest
/// comes back in group order all the same.
pub fn grow(columns: &[Column], placed: &Placed, profiles: Profiles) -> Vec<SupportTree> {
    let mut order: Vec<(u16, &SupportProfile)> = profiles.groups().collect();
    order.sort_by(|a, b| b.1.pillar_radius_mm().total_cmp(&a.1.pillar_radius_mm()));
    let reach_mm = order
        .iter()
        .map(|(_, profile)| profile.branching.max_merge_distance_mm)
        .fold(SAME_PLACE_MM, Scalar::max);

    let mut carriers = Carriers::new(reach_mm);
    let mut forest = Vec::new();
    for (group, profile) in order {
        let mine: Vec<Column> = columns
            .iter()
            .filter(|column| column.group == group)
            .copied()
            .collect();
        let mut trees = grow_one(&mine, placed, profile, group, &carriers);
        // Before anything thinner is leant onto them: a trunk moved afterwards would
        // leave what joined it ending in the air.
        if profile.branching.enabled && profile.bracing.enabled {
            gather(&mut trees, &forest, placed, profile, profiles);
        }
        carriers.take(&trees, profile);
        forest.extend(trees);
    }
    stand_small_pillars_on_supports(&mut forest, &carriers, placed, profiles);
    forest.sort_by_key(SupportTree::group);
    forest
}

/// Runs every tree that stands on the model as a small pillar into a support beside it
/// instead, where one wider than the pillar is in its reach, and builds it that thin
/// all the way up.
///
/// A small pillar is the thinnest thing on the plate whatever its group's pillar is, so
/// it is never what a regular support stands on, and it is asked last, once every group
/// has stood what it could be run into. See `docs/decisions/0251`.
fn stand_small_pillars_on_supports(
    forest: &mut [SupportTree],
    carriers: &Carriers,
    placed: &Placed,
    profiles: Profiles,
) {
    for tree in forest.iter_mut() {
        let profile = profiles.of(tree.group());
        let root = *tree.root();
        if !profile.branching.enabled || root.is_leaf() || !is_small(tree, profile) {
            continue;
        }
        let radius_mm = profile.small_pillar.radius_mm().min(root.radius_mm);
        let met = carriers
            .near(root.position)
            .into_iter()
            .map(|index| &carriers.all[index])
            .filter(|carrier| carrier.radius_mm >= radius_mm + THICKER_BY_MM)
            .filter_map(|carrier| meeting_on(root.position, carrier, profile))
            .filter(|meet| {
                let strut = Beam {
                    from: root.position,
                    to: *meet,
                    from_radius_mm: radius_mm,
                    to_radius_mm: radius_mm,
                }
                .with_clearance(profile.clearance_mm)
                .trimmed(radius_mm + profile.clearance_mm);
                clear(placed, &strut)
            })
            .max_by(|a, b| a.z.total_cmp(&b.z));
        if let Some(meet) = met {
            tree.set_radius_mm(radius_mm);
            tree.stand_on(Landing {
                base: meet,
                on_model: true,
                pad_offset: Vec2::ZERO,
                on_support: true,
            });
        }
    }
}

/// How much wider a carrier has to be than what joins it, millimetres: under this the
/// two are one width, and neither is the other's base.
const THICKER_BY_MM: Scalar = 0.05;

/// A strut or a trunk a thicker group has already stood, which a thinner tree may run
/// into, from its upper end to its lower.
#[derive(Debug, Clone, Copy)]
struct Carrier {
    top: Vec3,
    bottom: Vec3,
    radius_mm: Scalar,
    /// How far above `bottom` it has to be met: the foot under a trunk is not the trunk.
    floor_mm: Scalar,
}

/// Every carrier stood so far, bucketed by the squares of plate it passes over, so a
/// front asks nine buckets rather than every strut of every group before it.
struct Carriers {
    all: Vec<Carrier>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    cell_mm: Scalar,
}

impl Carriers {
    /// `cell_mm` is the furthest any group reaches across to merge.
    fn new(cell_mm: Scalar) -> Self {
        Self {
            all: Vec::new(),
            grid: HashMap::new(),
            cell_mm,
        }
    }

    fn cell_of(&self, at: Vec3) -> (i32, i32) {
        (
            (at.x / self.cell_mm).floor() as i32,
            (at.y / self.cell_mm).floor() as i32,
        )
    }

    /// Takes on the struts and the trunks of `trees`. The strut under a tip is its head,
    /// narrowing to the contact, and carries nothing but its own tip. A tree built as a
    /// small pillar is as wide as that, whatever its nodes say, and its trunk is left
    /// out: it may yet be run into a support beside it, and would leave what stood on
    /// it ending in the air.
    fn take(&mut self, trees: &[SupportTree], profile: &SupportProfile) {
        for tree in trees {
            let small = is_small(tree, profile);
            let widest_mm = if small {
                profile.small_pillar.radius_mm()
            } else {
                Scalar::INFINITY
            };
            let nodes = tree.nodes();
            for (child, parent) in tree.struts().filter(|(child, _)| !nodes[*child].is_leaf()) {
                self.push(Carrier {
                    top: nodes[child].position,
                    bottom: nodes[parent].position,
                    radius_mm: nodes[child].radius_mm.min(widest_mm),
                    floor_mm: 0.0,
                });
            }
            let (root, landing) = (tree.root(), tree.landing());
            if !root.is_leaf() && !small {
                let bottom_z = landing.pillar_bottom_z(profile);
                self.push(Carrier {
                    top: root.position,
                    bottom: Vec3::new(landing.base.x, landing.base.y, bottom_z),
                    radius_mm: root.radius_mm.min(widest_mm),
                    floor_mm: MIN_PILLAR_HEIGHT_MM,
                });
            }
        }
    }

    fn push(&mut self, carrier: Carrier) {
        if carrier.top.z - carrier.bottom.z < MIN_PILLAR_HEIGHT_MM {
            return;
        }
        let index = self.all.len();
        let (one, two) = (self.cell_of(carrier.top), self.cell_of(carrier.bottom));
        for x in one.0.min(two.0)..=one.0.max(two.0) {
            for y in one.1.min(two.1)..=one.1.max(two.1) {
                self.grid.entry((x, y)).or_default().push(index);
            }
        }
        self.all.push(carrier);
    }

    /// Every carrier passing over the nine buckets around `at`, each once.
    fn near(&self, at: Vec3) -> Vec<usize> {
        let (cx, cy) = self.cell_of(at);
        let mut found: Vec<usize> = (cx - 1..=cx + 1)
            .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
            .filter_map(|cell| self.grid.get(&cell))
            .flatten()
            .copied()
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    }
}

/// How many times the stretch of a carrier a strut may meet is halved to find where: 16
/// puts the point within a thousandth of a millimetre on any strut that is printed.
const MEETING_HALVINGS: u32 = 16;

/// The highest point on `carrier` that a strut from `from` reaches without leaning
/// further than `profile` lets a branch lean, or `None` when it is out of reach or would
/// come in under what the carrier stands on.
fn meeting_on(from: Vec3, carrier: &Carrier, profile: &SupportProfile) -> Option<Vec3> {
    let (top, bottom) = (carrier.top, carrier.bottom);

    // How much steeper than it has to be the strut to a point of the carrier comes in:
    // under zero it would lean too far. A carrier leaning away never lets it catch up.
    let reach = lean_per_drop(profile);
    let at = |share: Scalar| top + (bottom - top) * share;
    let spare = |share: Scalar| {
        let point = at(share);
        (from.z - point.z) * reach - (point - from).truncate().length()
    };
    if spare(1.0) < 0.0 {
        return None;
    }
    // The highest such point, halved down to well under a layer.
    let (mut too_high, mut low_enough) = (0.0, 1.0);
    if spare(0.0) >= 0.0 {
        low_enough = 0.0;
    }
    for _ in 0..MEETING_HALVINGS {
        let middle = Scalar::midpoint(too_high, low_enough);
        if spare(middle) >= 0.0 {
            low_enough = middle;
        } else {
            too_high = middle;
        }
    }
    let meet = at(low_enough);

    // Half the merge distance: two fronts that far apart meet in the middle, so this is
    // as long as a strut ever is, and a strut twice that across a cavity is a lever.
    let across = (meet - from).truncate().length();
    let leans_within = from.z - meet.z >= across / reach - SAME_PLACE_MM;
    let in_reach = across <= profile.branching.max_merge_distance_mm / 2.0;
    let on_it = meet.z - bottom.z >= carrier.floor_mm;
    let long_enough = (from - meet).length() >= MIN_PILLAR_HEIGHT_MM;
    (leans_within && in_reach && on_it && long_enough).then_some(meet)
}

/// Grows one group's columns, every one of which is built to `profile`.
fn grow_one(
    columns: &[Column],
    placed: &Placed,
    profile: &SupportProfile,
    group: u16,
    carriers: &Carriers,
) -> Vec<SupportTree> {
    let mut forest = Forest::seeded(columns, profile, carriers);
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

/// How near a trunk that stood alone is brought to its neighbour, as a share of the
/// distance a brace spans: inside it with room to spare, and not on top of the other.
const GATHERED: Scalar = 0.75;

/// Leans every tall trunk that stands out of a brace's reach towards its nearest
/// neighbour, until one reaches.
///
/// A long thin trunk on its own is the support that sways on the peel, and bracing
/// cannot help what it cannot span. The lean is a knee within 45 degrees, taken only
/// where the trunk under it is still tall enough to brace, clears the model and lands on
/// the plate, and only as far as a branch reaches. The neighbour may be one of `settled`,
/// the trunks thicker groups have already stood: a brace ties any two trunks, whatever
/// their groups, where both groups have bracing switched on. See `docs/decisions/0240`,
/// `docs/decisions/0250` and `docs/decisions/0252`.
fn gather(
    trees: &mut [SupportTree],
    settled: &[SupportTree],
    placed: &Placed,
    profile: &SupportProfile,
    profiles: Profiles,
) {
    let reach = profile.bracing.max_spacing_mm;
    // No further than a branch reaches: a trunk carried across a cavity to meet a brace
    // is a longer lever than the one the brace was to steady.
    let furthest = profile.branching.max_merge_distance_mm / 2.0;
    let at = |tree: &SupportTree| tree.root().position.truncate();
    for index in 0..trees.len() {
        let tree = &trees[index];
        if tree.landing().on_model || !braceable(tree, profile) {
            continue;
        }
        let here = at(tree);
        let nearest = trees
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, tree)| tree)
            .chain(settled)
            .filter(|tree| braceable(tree, profiles.of(tree.group())))
            .map(at)
            .min_by(|a, b| (*a - here).length().total_cmp(&(*b - here).length()));
        let Some(there) = nearest.filter(|there| (*there - here).length() > reach) else {
            continue;
        };

        let step = (there - here) * (1.0 - reach * GATHERED / (there - here).length());
        if step.length() > furthest {
            continue;
        }
        let root = *tree.root();
        let knee = root.position + step.extend(-step.length() / lean_per_drop(profile));
        let lean = Beam {
            from: root.position,
            to: knee,
            from_radius_mm: root.radius_mm,
            to_radius_mm: root.radius_mm,
        }
        .with_clearance(profile.clearance_mm);
        let stands = landing(placed, knee, root.radius_mm, tree.tip_count(), profile)
            .filter(|landing| !landing.on_model && clear(placed, &lean));
        let Some(landing) = stands else {
            continue;
        };

        let mut leaning = tree.clone();
        leaning.lean_to(knee, root.radius_mm, landing);
        if braceable(&leaning, profile) {
            trees[index] = leaning;
        }
    }
}

/// A tip or a trunk that has not been merged into anything yet.
#[derive(Debug, Clone, Copy)]
struct Front {
    position: Vec3,
    radius_mm: Scalar,
    /// How many tips hang off it, which is what its trunk's base is sized for.
    tips: usize,
    node: usize,
    alive: bool,
}

/// Two fronts that could merge, or a front and the carrier it could run into, and how
/// high up they would meet.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    meet: Vec3,
    a: usize,
    /// The other front, or the carrier when `onto_carrier`.
    b: usize,
    onto_carrier: bool,
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
            .then_with(|| self.onto_carrier.cmp(&other.onto_carrier))
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
struct Forest<'a> {
    nodes: Vec<TreeNode>,
    fronts: Vec<Front>,
    /// What thicker groups have stood, for a front to run into.
    carriers: &'a Carriers,
    /// The fronts that ran into a carrier, each with where: trees with no trunk of their
    /// own.
    landed: Vec<(usize, Landing)>,
    /// Fronts bucketed by the square of plate they stand over, so a new one asks nine
    /// buckets rather than every front placed so far. Dead fronts stay in their bucket
    /// and are skipped; there are never more entries than twice the number of tips.
    grid: HashMap<(i32, i32), Vec<usize>>,
    cell_mm: Scalar,
}

impl<'a> Forest<'a> {
    /// One front per column, each at its contact.
    fn seeded(columns: &[Column], profile: &SupportProfile, carriers: &'a Carriers) -> Self {
        let radius_mm = profile.pillar_radius_mm();
        let mut forest = Self {
            nodes: Vec::with_capacity(columns.len()),
            fronts: Vec::with_capacity(columns.len()),
            carriers,
            landed: Vec::new(),
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
                tips: 1,
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
            self.offer_carriers(&mut heap, index, profile);
        }

        while let Some(candidate) = heap.pop() {
            if candidate.onto_carrier {
                if self.fronts[candidate.a].alive && self.reaches(&candidate, placed, profile) {
                    self.land_on_carrier(&candidate);
                }
                continue;
            }
            if !self.fronts[candidate.a].alive || !self.fronts[candidate.b].alive {
                continue;
            }
            if !self.accepts(&candidate, placed, profile) {
                continue;
            }

            let merged = self.join(&candidate);
            for other in self.neighbours(merged) {
                self.offer(&mut heap, merged, other, profile);
            }
            self.offer_carriers(&mut heap, merged, profile);
        }
    }

    /// Offers the heap every carrier front `a` could run into, and where.
    fn offer_carriers(&self, heap: &mut BinaryHeap<Candidate>, a: usize, profile: &SupportProfile) {
        let front = &self.fronts[a];
        if !front.alive {
            return;
        }
        for b in self.carriers.near(front.position) {
            let carrier = &self.carriers.all[b];
            if carrier.radius_mm < front.radius_mm + THICKER_BY_MM {
                continue;
            }
            if let Some(meet) = meeting_on(front.position, carrier, profile) {
                heap.push(Candidate {
                    meet,
                    a,
                    b,
                    onto_carrier: true,
                });
            }
        }
    }

    /// Whether the strut from a front to the carrier it would run into clears the model.
    fn reaches(&self, candidate: &Candidate, placed: &Placed, profile: &SupportProfile) -> bool {
        let front = &self.fronts[candidate.a];
        let strut = Beam {
            from: front.position,
            to: candidate.meet,
            from_radius_mm: front.radius_mm,
            to_radius_mm: front.radius_mm,
        }
        .with_clearance(profile.clearance_mm);
        clear(placed, &strut)
    }

    /// Ends a front on the carrier of `candidate`: it needs no trunk and no foot.
    fn land_on_carrier(&mut self, candidate: &Candidate) {
        self.fronts[candidate.a].alive = false;
        self.landed.push((
            candidate.a,
            Landing {
                base: candidate.meet,
                on_model: true,
                pad_offset: Vec2::ZERO,
                on_support: true,
            },
        ));
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
        heap.push(Candidate {
            meet,
            a,
            b,
            onto_carrier: false,
        });
    }

    /// Whether the two struts of `candidate` can be built: clear of the model, and with
    /// room under the meeting point for a trunk to stand.
    fn accepts(&self, candidate: &Candidate, placed: &Placed, profile: &SupportProfile) -> bool {
        let (a, b) = (&self.fronts[candidate.a], &self.fronts[candidate.b]);
        let joint_mm = a.radius_mm.max(b.radius_mm);
        let reaches = |front: &Front| {
            let beam = Beam {
                from: front.position,
                to: candidate.meet,
                from_radius_mm: front.radius_mm,
                to_radius_mm: joint_mm,
            }
            .with_clearance(profile.clearance_mm);
            clear(placed, &beam)
        };

        let tips = a.tips + b.tips;
        reaches(a) && reaches(b) && stands(candidate.meet, joint_mm, tips, placed, profile)
    }

    /// Hangs both fronts of `candidate` off a new node at the meeting point. The joint
    /// keeps the branches' own width: only the trunk under the last one widens, see
    /// `docs/decisions/0242`.
    fn join(&mut self, candidate: &Candidate) -> usize {
        let (a, b) = (self.fronts[candidate.a], self.fronts[candidate.b]);
        let radius_mm = a.radius_mm.max(b.radius_mm);

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
            tips: a.tips + b.tips,
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
            if landing(placed, front.position, front.radius_mm, front.tips, profile).is_some() {
                continue;
            }
            let Some((knee, _)) = escape(&front, placed, profile, MAX_LANDING_SLOPE_DEG) else {
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
                node,
                alive: true,
                ..front
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
            .flat_map(|front| {
                // Down, then aside; then both again onto a steeper face before the tips go
                // to a strut each: a support cured onto a slope beats none under a contact.
                for steepest_deg in [MAX_LANDING_SLOPE_DEG, LAST_RESORT_SLOPE_DEG] {
                    let (at, radius_mm, tips) = (front.position, front.radius_mm, front.tips);
                    let down = landing_within(placed, at, radius_mm, tips, profile, steepest_deg);
                    if let Some(landing) = down {
                        return vec![self.cut(front.node, &children, landing)];
                    }
                    if let Some((knee, landing)) = escape(front, placed, profile, steepest_deg) {
                        let mut tree = self.cut(front.node, &children, landing);
                        tree.stand_on_knee(knee, front.radius_mm);
                        return vec![tree];
                    }
                }
                self.bridges(front, &children, placed, profile)
            })
            .chain(
                self.landed.iter().map(|(front, landing)| {
                    self.cut(self.fronts[*front].node, &children, *landing)
                }),
            )
            .collect()
    }

    /// The last resort of the tips under a front with nowhere to stand: each on one strut
    /// from its contact straight down onto the model under it, as a contact in a crevice
    /// or under a low ledge needs. Each is a tree of the one node, which the mesher
    /// builds as a small pillar where the profile has one.
    fn bridges(
        &self,
        front: &Front,
        children: &[Vec<usize>],
        placed: &Placed,
        profile: &SupportProfile,
    ) -> Vec<SupportTree> {
        if !profile.land_on_model {
            return Vec::new();
        }
        let mut under_it = vec![front.node];
        let mut index = 0;
        while index < under_it.len() {
            under_it.extend(children[under_it[index]].iter().copied());
            index += 1;
        }
        under_it
            .into_iter()
            .map(|node| self.nodes[node])
            .filter(TreeNode::is_leaf)
            .filter_map(|tip| {
                // From just under the contact: one placed by hand sits on the face itself.
                let ray = Ray::new(tip.position - Vec3::Z * SURFACE_EPSILON_MM, -Vec3::Z);
                let under = raycast_placed(placed.model, placed.bvh, placed.transform, &ray)
                    // A face looking up at the contact: one looking away is the inside of
                    // the model, and a strut to it is built in the solid and pokes out.
                    .filter(|hit| hit.normal.z > 0.0)
                    .filter(|hit| hit.point.z > PLATE_Z_MM && !placed.blocks_face(hit.face))
                    .filter(|hit| (MIN_PILLAR_HEIGHT_MM..=BRIDGE_MAX_MM).contains(&hit.distance))?;
                let node = TreeNode {
                    parent: None,
                    ..tip
                };
                let landing = Landing {
                    base: under.point,
                    on_model: true,
                    pad_offset: Vec2::ZERO,
                    on_support: false,
                };
                Some(SupportTree::new(vec![node], 0, landing))
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

/// Whether a trunk carrying `tips` under `at`, leaving it `radius_mm` wide, has anywhere
/// to go: straight down, or leaning aside the way a blocked front does.
fn stands(
    at: Vec3,
    radius_mm: Scalar,
    tips: usize,
    placed: &Placed,
    profile: &SupportProfile,
) -> bool {
    let front = Front {
        position: at,
        radius_mm,
        tips,
        node: 0,
        alive: true,
    };
    landing(placed, at, radius_mm, tips, profile).is_some()
        || escape(&front, placed, profile, MAX_LANDING_SLOPE_DEG).is_some()
}

/// How much ground a leaning trunk — a knee, or the strut into a thicker support — covers
/// per millimetre it descends: the profile's branch angle, and never past 45 degrees,
/// whatever a branch is let lean. See `docs/decisions/0249`.
fn lean_per_drop(profile: &SupportProfile) -> Scalar {
    profile.branching.reach_per_drop().min(MAX_LEAN_PER_DROP)
}

/// The longest a support is run straight from its contact down to the model under it,
/// millimetres, when it has nowhere else to stand: a crevice or the gap under a ledge.
/// Further than this and what kept a column from standing is in the way of this too.
const BRIDGE_MAX_MM: Scalar = 6.0;

/// Where a support with nothing under it can lean to and still find a trunk's worth of
/// room, as the knee it bends at and what the trunk under that knee stands on.
///
/// The shortest reach that works is taken, so a support leans no further than it has to;
/// see `docs/decisions/0078`.
fn escape(
    front: &Front,
    placed: &Placed,
    profile: &SupportProfile,
    steepest_deg: Scalar,
) -> Option<(Vec3, Landing)> {
    let (from, radius_mm) = (front.position, front.radius_mm);
    let reach_per_drop = lean_per_drop(profile);
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
            // Swept from its own width and clearance down, as a trunk is: see
            // `column::stands_clear`.
            let beam = Beam {
                from,
                to: knee,
                from_radius_mm: radius_mm,
                to_radius_mm: radius_mm,
            }
            .with_clearance(profile.clearance_mm)
            .trimmed(radius_mm + profile.clearance_mm);
            if !clear(placed, &beam) {
                continue;
            }
            let stood = landing_within(placed, knee, radius_mm, front.tips, profile, steepest_deg);
            if let Some(landing) = stood {
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

    /// A thick support of group 0 at x = 10 and a thin one of group 1 `apart` along x,
    /// both 20 mm over an empty plate.
    fn thick_and_thin(apart: Scalar) -> Vec<SupportTree> {
        let mut thin = profile();
        thin.middle.diameter_mm *= 0.5;
        let table = [profile(), thin];
        let points = [
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(10.0 + apart, 10.0, 20.0)).in_group(1),
        ];

        let model = Mesh::default();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let profiles = Profiles::new(&table).expect("two groups");
        grow(
            &crate::columns(&points, &placed, profiles),
            &placed,
            profiles,
        )
    }

    #[test]
    fn two_tips_of_different_groups_stay_two_trees_of_their_own_shape() {
        let forest = thick_and_thin(4.0);
        assert_eq!(forest.len(), 2, "a tree's tips are all one shape");
        assert_eq!(
            forest.iter().map(SupportTree::group).collect::<Vec<_>>(),
            vec![0, 1],
            "each tree is stamped with the group it was built to"
        );
    }

    #[test]
    fn a_thinner_support_runs_into_the_trunk_of_a_thicker_one_beside_it() {
        let forest = thick_and_thin(4.0);
        let (thick, thin) = (&forest[0], &forest[1]);

        assert!(
            !thick.landing().on_model,
            "the thicker one carries, and stands on the plate"
        );
        let landing = thin.landing();
        assert!(
            landing.on_support && landing.on_model,
            "the thinner one has no foot"
        );
        assert!(
            (landing.base.truncate() - Vec2::new(10.0, 10.0)).length() < 1e-3,
            "it ends on the axis of the thick trunk, got {}",
            landing.base
        );
        let root = thin.root().position;
        let across = (root - landing.base).truncate().length();
        let lean = across / (root.z - landing.base.z);
        assert!(
            lean <= profile().branching.reach_per_drop() + 1e-3,
            "and leans no further than a branch may: {lean} across per unit of drop"
        );
        assert!(
            landing.base.z > thick.landing().base.z + 1.0 && landing.base.z < root.z,
            "it meets the trunk between its foot and its top, at z = {}",
            landing.base.z
        );
    }

    #[test]
    fn a_thinner_support_runs_into_a_branch_of_a_thicker_tree_and_not_only_its_trunk() {
        let mut thin = profile();
        thin.middle.diameter_mm *= 0.5;
        let table = [profile(), thin];
        // Two thick tips that share a trunk, and a thin one beside the middle of them:
        // the branches lean towards it, so it meets one before it could meet the trunk.
        let points = [
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(16.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(13.0, 12.0, 20.0)).in_group(1),
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

        assert_eq!(forest.len(), 2, "the two thick tips share a trunk");
        let (thick, thin) = (&forest[0], &forest[1]);
        assert_eq!(thick.tip_count(), 2);
        let joint_z = thick.root().position.z;
        let landing = thin.landing();
        assert!(landing.on_support, "the thin one has something to run into");
        assert!(
            landing.base.z > joint_z,
            "the branch beside it is met at z = {}, above the joint at {joint_z} the trunk \
             begins under",
            landing.base.z
        );
    }

    #[test]
    fn a_support_run_into_another_is_meshed_closed_and_without_a_foot() {
        let mut thin = profile();
        thin.middle.diameter_mm *= 0.5;
        let table = [profile(), thin];
        let profiles = Profiles::new(&table).expect("two groups");
        let forest = thick_and_thin(4.0);

        let model = Mesh::default();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let mesh = crate::mesh_trees(&forest, &placed, profiles);

        assert_eq!(core_geometry::diagnose(&mesh).boundary_edges, 0);
        let join_z = forest[1].landing().base.z;
        let under = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.z < join_z - 1.0)
            // Past the rim of the thick support's own pad, which reaches x = 14.
            .filter(|vertex| vertex.x > 14.2)
            .count();
        assert_eq!(
            under, 0,
            "nothing of the thin support is built under where it joins the thick one"
        );
    }

    #[test]
    fn a_contact_inside_the_model_is_not_run_out_through_its_skin() {
        // In the middle of a solid block hanging over the plate: straight down is the
        // inside of its underside, 5 mm away.
        let model = box_mesh(Vec3::new(0.0, 0.0, 5.0), Vec3::new(10.0, 10.0, 15.0));
        let forest = forest_of(
            &[Vec3::new(5.0, 5.0, 10.0)],
            &model,
            Profiles::single(&profile()),
        );
        assert!(
            forest.is_empty(),
            "a strut built in the solid holds nothing up and pokes out of the part"
        );
    }

    #[test]
    fn a_small_pillar_stands_on_the_support_beside_it_and_never_the_other_way_round() {
        // A block with a contact 10 mm over it, which alone is a small pillar standing
        // on the block, and a contact beside the block with the plate under it. Two
        // groups of one profile, so that the two do not simply merge; the pillar's first.
        let block = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let table = [profile(), profile()];
        let points = [
            SupportPoint::new(Vec3::new(9.0, 5.0, 20.0)),
            // Far enough out for its pad to clear the block, so it stands straight.
            SupportPoint::new(Vec3::new(14.5, 5.0, 20.0)).in_group(1),
        ];
        let bvh = Bvh::build(&block);
        let placed = Placed::new(&block, &bvh, Transform::default());
        let profiles = Profiles::new(&table).expect("two groups");
        let forest = grow(
            &crate::columns(&points, &placed, profiles),
            &placed,
            profiles,
        );

        let (pillar, column) = (&forest[0], &forest[1]);
        assert!(
            !column.landing().on_model,
            "the regular support stands on the plate, not on the pillar"
        );
        let landing = pillar.landing();
        assert!(
            landing.on_support,
            "the pillar runs into it instead of the block"
        );
        assert!(
            (landing.base.truncate() - Vec2::new(14.5, 5.0)).length() < 1e-3,
            "it ends on the column's axis, got {}",
            landing.base
        );
        let thin = profile().small_pillar.radius_mm();
        assert!(
            pillar
                .nodes()
                .iter()
                .all(|node| (node.radius_mm - thin).abs() < 1e-6),
            "and it is a small pillar's width all the way up"
        );
    }

    #[test]
    fn a_small_pillar_never_stands_on_another_that_has_moved_away() {
        // The block and the two supports of the test above, and a third group whose small
        // pillar is thinner still, beside the first: its nearest carrier would be the
        // trunk the first pillar left behind when it ran into the column.
        let block = box_mesh(Vec3::ZERO, Vec3::splat(10.0));
        let mut thinner = profile();
        thinner.small_pillar.diameter_mm -= 0.2;
        let table = [profile(), profile(), thinner];
        let points = [
            SupportPoint::new(Vec3::new(9.0, 5.0, 20.0)),
            SupportPoint::new(Vec3::new(14.5, 5.0, 20.0)).in_group(1),
            SupportPoint::new(Vec3::new(9.0, 3.0, 20.0)).in_group(2),
        ];
        let bvh = Bvh::build(&block);
        let placed = Placed::new(&block, &bvh, Transform::default());
        let profiles = Profiles::new(&table).expect("three groups");
        let forest = grow(
            &crate::columns(&points, &placed, profiles),
            &placed,
            profiles,
        );

        let (moved, thin) = (&forest[0], &forest[2]);
        assert!(moved.landing().on_support, "the first ran into the column");
        let landing = thin.landing();
        assert!(
            !landing.on_support || (landing.base.truncate() - Vec2::new(14.5, 5.0)).length() < 1e-3,
            "nothing stands at {} any more for it to end on",
            landing.base
        );
    }

    #[test]
    fn a_trunk_leans_no_further_than_45_degrees_whatever_a_branch_may() {
        let mut wide = profile();
        wide.branching.max_angle_deg = 70.0;
        assert!(
            (lean_per_drop(&wide) - 1.0).abs() < 1e-6,
            "70 degrees is held to 45"
        );

        let mut narrow = profile();
        narrow.branching.max_angle_deg = 30.0;
        assert!(
            (lean_per_drop(&narrow) - 30.0_f32.to_radians().tan()).abs() < 1e-6,
            "a profile that asks for less keeps what it asked for"
        );
    }

    #[test]
    fn a_tip_with_no_room_for_a_trunk_is_run_straight_onto_the_model_under_it() {
        // A ledge a millimetre over a block: too little for a head and a column.
        let mut model = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 9.0));
        let ledge = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 12.0));
        let offset = model.vertices.len() as u32;
        model.vertices.extend_from_slice(&ledge.vertices);
        model
            .faces
            .extend(ledge.faces.iter().map(|face| face.map(|at| at + offset)));

        let contact = Vec3::new(5.0, 5.0, 10.0);
        let forest = forest_of(&[contact], &model, Profiles::single(&profile()));

        assert_eq!(forest.len(), 1, "the contact is held, not left bare");
        let tree = &forest[0];
        assert_eq!(tree.nodes().len(), 1, "one strut, from the contact itself");
        assert!(tree.root().is_leaf());
        let landing = tree.landing();
        assert!(landing.on_model);
        assert!(
            landing.base.abs_diff_eq(Vec3::new(5.0, 5.0, 9.0), 1e-3),
            "it comes down on the block straight under it, got {}",
            landing.base
        );

        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let mesh = crate::mesh_trees(&forest, &placed, Profiles::single(&profile()));
        assert!(!mesh.is_empty(), "and it is built");
        assert_eq!(core_geometry::diagnose(&mesh).boundary_edges, 0);
    }

    #[test]
    fn a_strut_into_a_thicker_support_is_no_longer_than_a_strut_of_a_merge() {
        // Inside the merge distance, and further than the half of it each of two merging
        // fronts would have to cover.
        let far = profile().branching.max_merge_distance_mm * 0.75;
        let forest = thick_and_thin(far);
        assert!(
            forest.iter().all(|tree| !tree.landing().on_model),
            "the thin one stands on the plate rather than reach {far} mm across"
        );
    }

    #[test]
    fn a_thinner_support_out_of_reach_of_a_thicker_one_stands_on_the_plate() {
        let far = profile().branching.max_merge_distance_mm + 5.0;
        let forest = thick_and_thin(far);
        assert!(forest.iter().all(|tree| !tree.landing().on_model));
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
    fn a_joint_keeps_the_width_of_the_branches_it_joins() {
        let profile = profile();
        let forest = forest_of(
            &[Vec3::new(10.0, 10.0, 20.0), Vec3::new(14.0, 10.0, 20.0)],
            &Mesh::default(),
            Profiles::single(&profile),
        );
        assert!(
            (forest[0].root().radius_mm - profile.pillar_radius_mm()).abs() < 1e-6,
            "only the trunk under the joint widens, and it does so towards its base"
        );
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

    /// The fixture with braces spanning 8 mm and branches merging no further than 10.
    fn braced_at_8_mm() -> SupportProfile {
        let mut profile = profile();
        profile.branching.max_merge_distance_mm = 10.0;
        profile.bracing = printer_profiles::Bracing {
            enabled: true,
            max_spacing_mm: 8.0,
            start_height_mm: 5.0,
            rise_mm: 5.0,
            ..printer_profiles::Bracing::default()
        };
        profile
    }

    #[test]
    fn a_trunk_is_not_carried_across_a_gap_wider_than_a_branch_reaches_to_meet_a_brace() {
        let profile = braced_at_8_mm();
        let forest = forest_of(
            &[Vec3::new(0.0, 0.0, 40.0), Vec3::new(30.0, 0.0, 40.0)],
            &Mesh::default(),
            Profiles::single(&profile),
        );
        let apart = (forest[0].landing().base - forest[1].landing().base).length();
        assert!(
            (apart - 30.0).abs() < 1e-3,
            "24 mm of lean is a lever, so each stands under its own tip: {apart} mm apart"
        );
    }

    #[test]
    fn a_trunk_leans_towards_one_of_another_group_as_it_would_towards_its_own() {
        let mut thin = braced_at_8_mm();
        thin.middle.diameter_mm *= 0.5;
        let table = [braced_at_8_mm(), thin];
        let points = [
            SupportPoint::new(Vec3::new(0.0, 0.0, 40.0)),
            SupportPoint::new(Vec3::new(11.0, 0.0, 40.0)).in_group(1),
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

        let (thick, thin) = (&forest[0], &forest[1]);
        assert!(
            (thick.landing().base.truncate()).length() < 1e-3,
            "the thicker one was stood first and stays where it stood"
        );
        let apart = (thick.landing().base - thin.landing().base).length();
        assert!(
            apart <= table[0].bracing.max_spacing_mm,
            "the thinner one leant into a brace's reach of it, {apart} mm apart"
        );
    }

    #[test]
    fn a_tall_trunk_standing_alone_leans_into_reach_of_a_brace() {
        // Two columns 40 mm tall and 11 mm apart: past merging, and past the 8 mm a brace
        // spans, with far more height than the lean costs.
        let profile = braced_at_8_mm();
        let forest = forest_of(
            &[Vec3::new(0.0, 0.0, 40.0), Vec3::new(11.0, 0.0, 40.0)],
            &Mesh::default(),
            Profiles::single(&profile),
        );
        assert_eq!(forest.len(), 2, "11 mm is too far to merge");

        let apart = (forest[0].landing().base - forest[1].landing().base).length();
        assert!(
            apart <= profile.bracing.max_spacing_mm,
            "their trunks stand {apart} mm apart, out of a brace's reach"
        );
        for tree in &forest {
            assert!(
                braceable(tree, &profile),
                "and each is still tall enough to tie"
            );
        }
    }

    #[test]
    fn no_contacts_grow_no_trees() {
        assert!(forest_of(&[], &Mesh::default(), Profiles::single(&profile())).is_empty());
    }
}
