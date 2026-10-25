use super::{SAME_PLACE_MM, lean_per_drop};
use crate::column::MIN_PILLAR_HEIGHT_MM;
use crate::group::Profiles;
use crate::mesher::is_small;
use crate::tree::SupportTree;
use core_geometry::{Buckets, Scalar, Vec3};
use printer_profiles::SupportProfile;

/// How much wider a carrier has to be than what joins it, millimetres: under this the
/// two are one width, and neither is the other's base.
pub(super) const THICKER_BY_MM: Scalar = 0.05;

/// A strut or a trunk a thicker group has already stood, which a thinner tree may run
/// into, from its upper end to its lower.
#[derive(Debug, Clone, Copy)]
pub(super) struct Carrier {
    pub(super) top: Vec3,
    pub(super) bottom: Vec3,
    pub(super) radius_mm: Scalar,
    /// How far above `bottom` it has to be met: the foot under a trunk is not the trunk.
    pub(super) floor_mm: Scalar,
    /// The tree it is part of, an index into [`Carriers::trunks`].
    pub(super) tree: usize,
}

/// Every carrier stood so far, bucketed by the squares of plate it passes over, so a
/// front asks nine buckets rather than every strut of every group before it.
pub(super) struct Carriers {
    pub(super) all: Vec<Carrier>,
    /// The tips each tree stood so far carries, and the most its group lets it carry.
    pub(super) trunks: Vec<(usize, usize)>,
    pub(super) grid: Buckets<usize>,
}

impl Carriers {
    /// `cell_mm` is the furthest any group reaches across to merge.
    pub(super) fn new(cell_mm: Scalar) -> Self {
        Self {
            all: Vec::new(),
            trunks: Vec::new(),
            grid: Buckets::new(cell_mm),
        }
    }

    /// Takes on the struts and trunks of `trees`. A tip's strut carries only its tip; a
    /// small-pillar tree's trunk is left out, as it may yet be run into a support beside it.
    pub(super) fn take(&mut self, trees: &[SupportTree], profiles: Profiles) {
        for tree in trees {
            let profile = profiles.of(tree.group());
            let small = is_small(tree, profile);
            let index = self.trunks.len();
            self.trunks.push((
                tree.tip_count(),
                profile.branching.max_tips_per_trunk as usize,
            ));
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
                    tree: index,
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
                    tree: index,
                });
            }
        }
    }

    pub(super) fn push(&mut self, carrier: Carrier) {
        if carrier.top.z - carrier.bottom.z < MIN_PILLAR_HEIGHT_MM {
            return;
        }
        let index = self.all.len();
        self.grid
            .insert_over(carrier.top.truncate(), carrier.bottom.truncate(), &index);
        self.all.push(carrier);
    }

    /// Every carrier passing over the nine buckets around `at`, each once.
    pub(super) fn near(&self, at: Vec3) -> Vec<usize> {
        let mut found: Vec<usize> = self.grid.around(at.truncate()).copied().collect();
        found.sort_unstable();
        found.dedup();
        found
    }
}

/// How many times the stretch of a carrier a strut may meet is halved to find where: 16
/// puts the point within a thousandth of a millimetre on any strut that is printed.
pub(super) const MEETING_HALVINGS: u32 = 16;

/// The highest point on `carrier` a strut from `from` reaches within `profile`'s branch lean, or
/// `None` when out of reach or it would come in under what the carrier stands on.
pub(super) fn meeting_on(from: Vec3, carrier: &Carrier, profile: &SupportProfile) -> Option<Vec3> {
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
    let strut_mm = (from - meet).length();
    let long_enough = strut_mm >= MIN_PILLAR_HEIGHT_MM;
    let short_enough = strut_mm <= profile.branching.max_strut_length_mm;
    (leans_within && in_reach && on_it && long_enough && short_enough).then_some(meet)
}
