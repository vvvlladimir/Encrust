use std::f32::consts::FRAC_1_SQRT_2;

use core_geometry::{Quat, Scalar, Vec2, Vec3};
use printer_profiles::SupportProfile;

use crate::base::{SAME_PLACE_MM, ties, top_z};
use crate::clear::{Beam, clear};
use crate::column::landing;
use crate::group::Profiles;
use crate::mesher::trunk_axis_at;
use crate::placed::Placed;
use crate::tree::{SupportTree, TreeNode};
use rayon::prelude::*;

/// How far either buttress of a pair turns off the way out from the part, degrees: apart
/// enough to hold the trunk two ways, near enough to be braced to each other.
const PAIR_TURN_DEG: Scalar = 20.0;

/// The buttresses `trees` want, each a tree of its own: a trunk standing over `pair_above_mm`
/// above its highest brace gets two, over `one_above_mm` above its second gets one (ADR 0328).
pub(crate) fn buttresses(
    trees: &[SupportTree],
    placed: &Placed,
    profiles: Profiles,
) -> Vec<SupportTree> {
    let wanted = |tree: &SupportTree| {
        let bracing = &profiles.of(tree.group()).bracing;
        bracing.enabled && bracing.buttress.enabled
    };
    if !trees.iter().any(wanted) {
        return Vec::new();
    }
    let held = held_at(trees, placed, profiles);
    let middle = placed
        .model
        .aabb()
        .map(|aabb| {
            let centre = aabb.center();
            placed.transform.to_matrix().transform_point3(centre)
        })
        .unwrap_or_default()
        .truncate();
    trees
        .par_iter()
        .zip(held)
        .flat_map_iter(|(tree, held)| {
            let profile = profiles.of(tree.group());
            let out = (tree.root().position.truncate() - middle)
                .try_normalize()
                .unwrap_or(Vec2::X);
            turns(tree, profile, held)
                .iter()
                .filter_map(move |turn| buttress(tree, profile, placed, out, *turn))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// How high each of `trees` is braced: the top of its ties to the neighbour it is tied
/// highest to, and to the one after.
fn held_at(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Vec<[Option<Scalar>; 2]> {
    let mut held = vec![[None; 2]; trees.len()];
    for (a, b, struts) in ties(trees, placed, profiles) {
        let top = struts
            .iter()
            .map(|(from, to, _, _)| from.z.max(to.z))
            .fold(Scalar::MIN, Scalar::max);
        for tree in [a, b] {
            let [first, second] = &mut held[tree];
            if first.is_none_or(|first| top > first) {
                *second = *first;
                *first = Some(top);
            } else if second.is_none_or(|second| top > second) {
                *second = Some(top);
            }
        }
    }
    held
}

/// Which way each buttress `tree` wants turns off the way out, degrees.
fn turns(
    tree: &SupportTree,
    profile: &SupportProfile,
    held: [Option<Scalar>; 2],
) -> &'static [Scalar] {
    let (bracing, landing) = (&profile.bracing, tree.landing());
    if !bracing.enabled || !bracing.buttress.enabled || landing.on_model || landing.on_support {
        return &[];
    }
    let (top, bottom) = (top_z(tree, profile), landing.bottom_z(profile));
    let free = |held: Option<Scalar>| top - held.unwrap_or(bottom);
    if free(held[0]) > bracing.buttress.pair_above_mm {
        &[PAIR_TURN_DEG, -PAIR_TURN_DEG]
    } else if free(held[1]) > bracing.buttress.one_above_mm {
        &[0.0]
    } else {
        &[]
    }
}

/// One buttress off the top of `tree`, `turn_deg` off `out`: a strut one rise long
/// leaning 45 degrees out and down, and a column from its foot to the plate.
fn buttress(
    tree: &SupportTree,
    profile: &SupportProfile,
    placed: &Placed,
    out: Vec2,
    turn_deg: Scalar,
) -> Option<SupportTree> {
    let reach = profile.bracing.rise_mm * FRAC_1_SQRT_2;
    let way = Quat::from_rotation_z(turn_deg.to_radians()) * out.extend(0.0);
    let top = trunk_axis_at(tree, profile, top_z(tree, profile));
    let knee = top + way * reach - Vec3::Z * reach;
    let radius_mm = profile.pillar_radius_mm();
    let strut = Beam {
        from: top,
        to: knee,
        from_radius_mm: radius_mm,
        to_radius_mm: radius_mm,
    }
    .with_clearance(profile.clearance_mm);
    if reach <= SAME_PLACE_MM || !clear(placed, &strut) {
        return None;
    }
    let stands = landing(placed, knee, radius_mm, 1, profile)
        .filter(|landing| !landing.on_model && !landing.on_support)?;
    let node = |position, parent| TreeNode {
        position,
        radius_mm,
        parent,
        point: None,
        head: None,
    };
    let mut leg = SupportTree::new(vec![node(top, Some(1)), node(knee, None)], 1, stands);
    leg.set_group(tree.group());
    Some(leg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SupportPoint;
    use crate::fixtures::{profile, trees_on};
    use core_geometry::{Bvh, Mesh, Transform};
    use printer_profiles::{BracingPreset, Branching};

    /// `preset`'s bracing, buttresses and all, with branching off so trunks stay apart.
    fn braced(preset: BracingPreset) -> SupportProfile {
        SupportProfile {
            branching: Branching {
                enabled: false,
                ..profile().branching
            },
            bracing: preset.bracing(),
            ..profile()
        }
    }

    /// The trees standing `height` millimetres tall at each of `places`, and the
    /// buttresses they want, in clear air.
    fn propped(
        places: &[(Scalar, Scalar)],
        height: Scalar,
        profile: &SupportProfile,
    ) -> (Vec<SupportTree>, Vec<SupportTree>) {
        let model = Mesh::default();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let points: Vec<SupportPoint> = places
            .iter()
            .map(|&(x, y)| SupportPoint::new(Vec3::new(x, y, height)))
            .collect();
        let profiles = Profiles::single(profile);
        let trees = trees_on(&points, &model, Transform::default(), profiles);
        let legs = buttresses(&trees, &placed, profiles);
        (trees, legs)
    }

    #[test]
    fn a_tall_lone_trunk_gets_a_pair_leaning_out_at_45_degrees() {
        let profile = braced(BracingPreset::Standard);
        let (trees, legs) = propped(&[(10.0, 10.0)], 50.0, &profile);
        assert_eq!(
            legs.len(),
            2,
            "50 mm with no brace is past the pair's 30 mm"
        );
        let top = trunk_axis_at(&trees[0], &profile, top_z(&trees[0], &profile));
        for leg in &legs {
            let (from, knee) = (leg.nodes()[0].position, leg.root().position);
            assert!(
                (from - top).length() < 1e-4,
                "a leg starts where the trunk ends"
            );
            let across = (knee - from).truncate().length();
            let drop = from.z - knee.z;
            assert!(
                (across - drop).abs() < 1e-4 && (across - 5.0 * FRAC_1_SQRT_2).abs() < 1e-4,
                "one 5 mm rise at 45 degrees goes as far out as down, got {across} and {drop}"
            );
            assert!(!leg.landing().on_model);
        }
    }

    #[test]
    fn the_pair_is_braced_to_its_trunk_and_to_each_other() {
        let profile = braced(BracingPreset::Standard);
        let (trees, legs) = propped(&[(10.0, 10.0)], 50.0, &profile);
        let all: Vec<SupportTree> = trees.into_iter().chain(legs).collect();
        let model = Mesh::default();
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let pairs: Vec<(usize, usize)> = ties(&all, &placed, Profiles::single(&profile))
            .into_iter()
            .map(|(a, b, _)| (a, b))
            .collect();
        assert_eq!(pairs, [(0, 1), (0, 2), (1, 2)]);
    }

    #[test]
    fn a_trunk_short_of_both_heights_gets_none() {
        let profile = braced(BracingPreset::Standard);
        let (_, legs) = propped(&[(10.0, 10.0)], 20.0, &profile);
        assert!(
            legs.is_empty(),
            "20 mm is short of the pair's 30 mm and the one's 60"
        );
    }

    #[test]
    fn a_trunk_tied_to_one_neighbour_alone_gets_one_each() {
        // Small: a pair past 10 mm above the highest brace, one past 30 above the second.
        let profile = braced(BracingPreset::Small);
        let (_, legs) = propped(&[(10.0, 10.0), (18.0, 10.0)], 50.0, &profile);
        assert_eq!(legs.len(), 2, "tied to the top, but by one neighbour each");
    }

    #[test]
    fn buttresses_switched_off_prop_nothing() {
        let mut profile = braced(BracingPreset::Standard);
        profile.bracing.buttress.enabled = false;
        let (_, legs) = propped(&[(10.0, 10.0)], 50.0, &profile);
        assert!(legs.is_empty());
    }
}
