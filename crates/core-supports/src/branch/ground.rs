use super::{Forest, Front, SAME_PLACE_MM, lean_per_drop};
use crate::Landing;
use crate::clear::{Beam, clear};
use crate::column::{MAX_LANDING_SLOPE_DEG, MIN_PILLAR_HEIGHT_MM, landing, landing_within};
use crate::placed::Placed;
use crate::tree::TreeNode;
use core_geometry::{Scalar, Vec3};
use printer_profiles::SupportProfile;
use rayon::prelude::*;

/// How many directions a blocked front leans in and how far apart the reaches, mm, shortest first:
/// eight is every 45 degrees, a millimetre the least step leaving the model a body's width.
pub(super) const ESCAPE_DIRECTIONS: u32 = 8;

pub(super) const ESCAPE_STEP_MM: Scalar = 1.0;

/// The longest a support runs straight down to the model under it, mm, with nowhere else to stand
/// (crevice, ledge); further, whatever blocked the column blocks this too.
pub(super) const BRIDGE_MAX_MM: Scalar = 6.0;

/// Where a support with nothing under it can lean to find a trunk's worth of room: the knee and
/// what the trunk stands on; the shortest working reach is taken (ADR 0078).
pub(super) fn escape(
    front: &Front,
    placed: &Placed,
    profile: &SupportProfile,
    steepest_deg: Scalar,
) -> Option<(Vec3, Landing)> {
    let search = Search {
        from_mm: 0.0,
        to_mm: profile.branching.max_merge_distance_mm,
        reaches: (profile.branching.max_merge_distance_mm / ESCAPE_STEP_MM).ceil() as u32,
        directions: ESCAPE_DIRECTIONS,
    };
    lean_for_ground(front, placed, profile, steepest_deg, search)
}

/// The longest strut a support with nowhere nearer to stand leans down to find ground,
/// millimetres along it; see ADR 0338.
pub(super) const GROUND_SEARCH_MM: Scalar = 25.0;

/// How far apart the reaches of the long search for ground are, millimetres across the
/// plate, and how many ways round it looks at each.
pub(super) const GROUND_SEARCH_STEP_MM: Scalar = 1.5;

pub(super) const GROUND_SEARCH_DIRECTIONS: u32 = 16;

/// [`escape`] carried past the merge reach to a strut [`GROUND_SEARCH_MM`] long, finer round:
/// the last lean offered before running onto the model or being left unstood (ADR 0338).
pub(super) fn search_for_ground(
    front: &Front,
    placed: &Placed,
    profile: &SupportProfile,
    steepest_deg: Scalar,
) -> Option<(Vec3, Landing)> {
    let reach_per_drop = lean_per_drop(profile);
    let to_mm = GROUND_SEARCH_MM * reach_per_drop / reach_per_drop.hypot(1.0);
    let from_mm = profile.branching.max_merge_distance_mm.min(to_mm);
    let reaches = ((to_mm - from_mm) / GROUND_SEARCH_STEP_MM).ceil();
    if reaches < 1.0 {
        return None;
    }
    let search = Search {
        from_mm,
        to_mm,
        reaches: reaches as u32,
        directions: GROUND_SEARCH_DIRECTIONS,
    };
    lean_for_ground(front, placed, profile, steepest_deg, search)
}

/// Where a lean for ground looks: `reaches` evenly out past `from_mm` to `to_mm` across
/// the plate, each tried `directions` ways round.
#[derive(Debug, Clone, Copy)]
pub(super) struct Search {
    pub(super) from_mm: Scalar,
    pub(super) to_mm: Scalar,
    pub(super) reaches: u32,
    pub(super) directions: u32,
}

/// The nearest knee `search` finds a front can lean to and stand under, and what it
/// stands on.
pub(super) fn lean_for_ground(
    front: &Front,
    placed: &Placed,
    profile: &SupportProfile,
    steepest_deg: Scalar,
    search: Search,
) -> Option<(Vec3, Landing)> {
    let (from, radius_mm) = (front.position, front.radius_mm);
    let reach_per_drop = lean_per_drop(profile);
    // A lean costs height, and what is left underneath is the ceiling on it: a support low
    // over the plate can only step aside as far as it can still descend.
    let room_mm = from.z - profile.footing_height_mm() - MIN_PILLAR_HEIGHT_MM;
    let reach_mm = (room_mm * reach_per_drop).min(search.to_mm);
    if reach_mm <= search.from_mm.max(SAME_PLACE_MM) {
        return None;
    }

    for step in 1..=search.reaches {
        let share = step as Scalar / search.reaches as Scalar;
        let ground = search.from_mm + (reach_mm - search.from_mm) * share;
        for turn in 0..search.directions {
            let angle = std::f32::consts::TAU * turn as Scalar / search.directions as Scalar;
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

impl Forest<'_> {
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
                branches: 1,
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
