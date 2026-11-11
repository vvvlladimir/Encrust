//! How the supports' resin splits between their parts, read off the bodies the mesher sweeps.

use core_geometry::{Mesh, Scalar, signed_volume};
use rayon::prelude::*;

use super::{brace, propped, raft, sound::bodies, stand_on, sweep};
use crate::base::braces;
use crate::group::Profiles;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// The resin each part of the supports takes, millilitres, each body whole: where bodies
/// overlap, as at a joint, both count it, so the parts add up to a little over the union.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResinByPart {
    /// Struts that start at a contact: the head and the run to where it merges.
    pub heads_ml: Scalar,
    /// Struts between two joints of a tree.
    pub branches_ml: Scalar,
    /// The column each tree stands on, a lone support's head included.
    pub trunks_ml: Scalar,
    /// Flares and pads.
    pub feet_ml: Scalar,
    pub braces_ml: Scalar,
    pub raft_ml: Scalar,
}

impl ResinByPart {
    pub fn total_ml(&self) -> Scalar {
        self.heads_ml
            + self.branches_ml
            + self.trunks_ml
            + self.feet_ml
            + self.braces_ml
            + self.raft_ml
    }

    fn add(self, other: Self) -> Self {
        Self {
            heads_ml: self.heads_ml + other.heads_ml,
            branches_ml: self.branches_ml + other.branches_ml,
            trunks_ml: self.trunks_ml + other.trunks_ml,
            feet_ml: self.feet_ml + other.feet_ml,
            braces_ml: self.braces_ml + other.braces_ml,
            raft_ml: self.raft_ml + other.raft_ml,
        }
    }
}

/// What each part of `trees` takes as [`super::mesh_trees`] builds them clear of `placed`.
pub fn resin_by_part(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> ResinByPart {
    let trees = &propped(trees, placed, profiles);
    let grown = trees
        .par_iter()
        .map(|tree| of_tree(tree, placed, profiles))
        .reduce(ResinByPart::default, ResinByPart::add);
    let braces_ml = braces(trees, placed, profiles)
        .into_iter()
        .map(|(from, to, radius_mm, group)| {
            tube_ml(&brace(from, to, radius_mm, profiles.of(group)))
        })
        .fold(0.0, |sum, ml| sum + ml);
    let raft_ml = raft(trees, placed, profiles).map_or(0.0, |slab| ml(&slab));
    ResinByPart {
        braces_ml,
        raft_ml,
        ..grown
    }
}

fn of_tree(tree: &SupportTree, placed: &Placed, profiles: Profiles) -> ResinByPart {
    let mut parts = ResinByPart::default();
    let mut bodies = bodies(tree, profiles, placed);
    if let Some(trunk) = bodies.pop() {
        parts.trunks_ml = tube_ml(&trunk.tube);
    }
    for body in bodies {
        let resin = tube_ml(&body.tube);
        if body.point.is_some() {
            parts.heads_ml += resin;
        } else {
            parts.branches_ml += resin;
        }
    }
    let mut feet = Mesh::default();
    stand_on(&mut feet, tree, profiles.of(tree.group()));
    parts.feet_ml = ml(&feet);
    parts
}

fn tube_ml(tube: &super::Tube) -> Scalar {
    let mut mesh = Mesh::default();
    sweep(&mut mesh, tube);
    ml(&mesh)
}

/// A closed mesh's volume, in millilitres of its cubic millimetres.
fn ml(mesh: &Mesh) -> Scalar {
    signed_volume(mesh).abs() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SupportPoint;
    use crate::fixtures::{meshed, profile, trees_on};
    use core_geometry::{Bvh, Transform, Vec3};

    fn split(points: &[SupportPoint]) -> (ResinByPart, Scalar) {
        let profile = profile();
        let profiles = Profiles::single(&profile);
        let model = Mesh::default();
        let trees = trees_on(points, &model, Transform::default(), profiles);
        let bvh = Bvh::build(&model);
        let placed = Placed::new(&model, &bvh, Transform::default());
        let whole = signed_volume(&meshed(&trees, profiles)) / 1000.0;
        (resin_by_part(&trees, &placed, profiles), whole)
    }

    #[test]
    fn a_lone_column_is_a_head_a_trunk_and_a_foot() {
        let (parts, whole) = split(&[SupportPoint::new(Vec3::new(10.0, 10.0, 20.0))]);
        assert_eq!(parts.branches_ml + parts.braces_ml + parts.raft_ml, 0.0);
        assert!(parts.heads_ml > 0.0 && parts.trunks_ml > 0.0 && parts.feet_ml > 0.0);
        assert!(
            parts.total_ml() >= whole && parts.total_ml() < whole * 1.1,
            "a column's parts overlap only where they meet: {} ml against its {whole} ml",
            parts.total_ml()
        );
    }

    #[test]
    fn a_merged_pair_has_heads_and_counts_its_joint_twice() {
        let (parts, whole) = split(&[
            SupportPoint::new(Vec3::new(10.0, 10.0, 20.0)),
            SupportPoint::new(Vec3::new(14.0, 10.0, 20.0)),
        ]);
        assert!(parts.heads_ml > 0.0, "both tips lean into a joint");
        assert!(parts.trunks_ml > 0.0 && parts.feet_ml > 0.0);
        assert!(
            parts.total_ml() >= whole,
            "the parts, {} ml, are never less than their union, {whole} ml",
            parts.total_ml()
        );
    }
}
