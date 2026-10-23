use std::collections::HashMap;

use core_geometry::glam::IVec2;
use core_geometry::{Scalar, Vec3};
use printer_profiles::Grid;

use super::{Forest, Front, SAME_PLACE_MM};
use crate::clear::{Beam, clear};
use crate::column::{MAX_LEAN_PER_DROP, landing};
use crate::placed::Placed;
use crate::tree::TreeNode;

/// How many rings of nodes round a tip are tried for its trunk: the cell it stands in and
/// the one round that, past which the strut to a node is longer than a merge would be.
const RINGS: i32 = 2;

/// A tip that fits a node, and the node it fits.
struct Hung {
    front: usize,
    /// Where its strut meets the trunk over the node.
    meet: Vec3,
}

impl Forest<'_> {
    /// Stands every front on a node of a square grid `grid.spacing_mm` across, anchored to
    /// the plate: the nearest node a 45 degree strut reaches clear of the model and a trunk
    /// stands under straight down. The fronts of one node hang one above another on one
    /// trunk, and a front no node takes is left where it is. See `docs/decisions/0335`.
    pub(super) fn snap_to_grid(&mut self, placed: &Placed, grid: Grid) {
        let spacing_mm = grid
            .spacing_mm
            .clamp(*Grid::SPACING_MM.start(), *Grid::SPACING_MM.end());
        let mut at_node: HashMap<IVec2, Vec<Hung>> = HashMap::new();
        for front in 0..self.fronts.len() {
            if !self.fronts[front].alive {
                continue;
            }
            if let Some((node, meet)) = self.node_for(front, placed, spacing_mm) {
                at_node.entry(node).or_default().push(Hung { front, meet });
            }
        }
        // In node order, so a run is repeatable whatever the map's order.
        let mut nodes: Vec<(IVec2, Vec<Hung>)> = at_node.into_iter().collect();
        nodes.sort_unstable_by_key(|(node, _)| (node.x, node.y));
        for (_, mut hung) in nodes {
            hung.sort_by(|a, b| b.meet.z.total_cmp(&a.meet.z).then(a.front.cmp(&b.front)));
            self.hang_on_trunk(&hung);
        }
    }

    /// The node `front` stands its trunk on and where its strut meets that trunk, nearest
    /// first, or `None` when no node in reach takes it.
    fn node_for(&self, front: usize, placed: &Placed, spacing_mm: Scalar) -> Option<(IVec2, Vec3)> {
        let at = self.fronts[front];
        let profile = self.profile(front);
        let home = (at.position.truncate() / spacing_mm).round().as_ivec2();
        let mut nodes: Vec<(Scalar, IVec2)> = (-RINGS..=RINGS)
            .flat_map(|x| (-RINGS..=RINGS).map(move |y| home + IVec2::new(x, y)))
            .map(|node| {
                let run = (node.as_vec2() * spacing_mm).distance(at.position.truncate());
                (run, node)
            })
            .collect();
        nodes.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then((a.1.x, a.1.y).cmp(&(b.1.x, b.1.y)))
        });
        nodes.into_iter().find_map(|(run_mm, node)| {
            let meet =
                (node.as_vec2() * spacing_mm).extend(at.position.z - run_mm / MAX_LEAN_PER_DROP);
            let strut = Beam {
                from: at.position,
                to: meet,
                from_radius_mm: at.radius_mm,
                to_radius_mm: at.radius_mm,
            }
            .with_clearance(profile.clearance_mm);
            let reaches = run_mm < SAME_PLACE_MM || clear(placed, &strut);
            let stands = landing(placed, meet, at.radius_mm, at.tips, profile).is_some();
            (reaches && stands).then_some((node, meet))
        })
    }

    /// Hangs the fronts of one node, highest first, each off a joint on one trunk over the
    /// node, and leaves the lowest joint the one front that stands it.
    fn hang_on_trunk(&mut self, hung: &[Hung]) {
        let mut below: Option<(usize, Vec3)> = None;
        let mut carried: Option<Front> = None;
        for Hung { front, meet } in hung {
            let at = self.fronts[*front];
            self.fronts[*front].alive = false;
            let joint = match below {
                Some((joint, over)) if over.distance(*meet) < SAME_PLACE_MM => joint,
                _ if at.position.distance(*meet) < SAME_PLACE_MM => at.node,
                _ => {
                    self.nodes.push(TreeNode {
                        position: *meet,
                        radius_mm: at.radius_mm,
                        parent: None,
                        point: None,
                        head: None,
                    });
                    self.nodes.len() - 1
                }
            };
            if joint != at.node {
                self.nodes[at.node].parent = Some(joint);
            }
            if let Some((above, _)) = below.filter(|(above, _)| *above != joint) {
                self.nodes[above].parent = Some(joint);
            }
            below = Some((joint, *meet));
            carried = Some(match carried {
                None => at,
                Some(trunk) => Front {
                    radius_mm: trunk.radius_mm.max(at.radius_mm),
                    tips: trunk.tips + at.tips,
                    group: self.profiles.heavier(trunk.group, at.group),
                    ..trunk
                },
            });
        }
        if let (Some((node, position)), Some(trunk)) = (below, carried) {
            self.push_front(Front {
                position,
                node,
                alive: true,
                ..trunk
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::{box_mesh, profile, trees_on};
    use crate::{Profiles, SupportPoint, SupportTree};
    use core_geometry::{Scalar, Transform, Vec2, Vec3};
    use printer_profiles::{Grid, SupportProfile};

    /// The test profile with its trunks on a grid 5 mm across.
    fn on_grid() -> SupportProfile {
        SupportProfile {
            grid: Grid {
                enabled: true,
                spacing_mm: 5.0,
            },
            ..profile()
        }
    }

    /// A slab 20 mm square with its underside 10 mm over the plate.
    fn slab() -> core_geometry::Mesh {
        box_mesh(Vec3::new(-10.0, -10.0, 10.0), Vec3::new(10.0, 10.0, 12.0))
    }

    fn under(contacts: &[(Scalar, Scalar)], profile: &SupportProfile) -> Vec<SupportTree> {
        let points: Vec<SupportPoint> = contacts
            .iter()
            .map(|(x, y)| SupportPoint::new(Vec3::new(*x, *y, 10.0)))
            .collect();
        let table = [profile.clone()];
        let profiles = Profiles::new(&table).expect("one profile");
        trees_on(&points, &slab(), Transform::default(), profiles)
    }

    #[test]
    fn every_trunk_stands_on_a_node_and_one_node_carries_its_tips_down_one_trunk() {
        let trees = under(&[(1.0, 1.0), (2.0, -1.0), (6.0, 1.0)], &on_grid());

        assert_eq!(
            trees.len(),
            2,
            "two tips nearest the node at 0, 0 and one at 5, 0"
        );
        let mut feet: Vec<Vec2> = trees
            .iter()
            .map(|tree| tree.landing().base.truncate())
            .collect();
        feet.sort_by(|a, b| a.x.total_cmp(&b.x));
        assert!(feet[0].distance(Vec2::ZERO) < 1e-4, "a foot at {}", feet[0]);
        assert!(
            feet[1].distance(Vec2::new(5.0, 0.0)) < 1e-4,
            "a foot at {}",
            feet[1]
        );
        let tips: Vec<usize> = trees.iter().map(SupportTree::tip_count).collect();
        assert_eq!(tips.iter().sum::<usize>(), 3);
        assert!(
            tips.contains(&2),
            "the node at 0, 0 holds both tips nearest it"
        );
    }

    #[test]
    fn a_tip_leans_to_its_node_at_45_degrees() {
        let trees = under(&[(1.0, 1.0)], &on_grid());
        let [tree] = trees.as_slice() else {
            panic!("one tip stands one trunk");
        };
        let root = tree.root().position;
        assert!(
            root.truncate().distance(Vec2::ZERO) < 1e-4,
            "the joint is over the node"
        );
        let neck = tree
            .nodes()
            .iter()
            .find(|node| node.parent == Some(tree.root_index()))
            .expect("the neck hangs from the joint")
            .position;
        let run = neck.truncate().distance(root.truncate());
        assert!(
            ((neck.z - root.z) - run).abs() < 1e-3,
            "a 45 degree strut drops as far as it runs: drop {} run {run}",
            neck.z - root.z
        );
    }

    #[test]
    fn a_tip_on_a_node_stands_straight_down() {
        let trees = under(&[(5.0, 5.0)], &on_grid());
        let [tree] = trees.as_slice() else {
            panic!("one tip stands one trunk");
        };
        assert!(tree.landing().base.truncate().distance(Vec2::new(5.0, 5.0)) < 1e-4);
    }

    #[test]
    fn off_the_grid_tips_merge_where_they_meet() {
        let trees = under(&[(1.0, 1.0), (2.0, -1.0), (6.0, 1.0)], &profile());
        assert!(
            trees
                .iter()
                .any(|tree| tree.landing().base.truncate().distance(Vec2::ZERO) > 0.5),
            "a merged trunk stands between its tips, not on a node"
        );
    }

    #[test]
    fn a_node_the_model_stands_on_gives_way_to_the_next_nearest() {
        // A post of the model standing on the plate under the node at 0, 0.
        let mut model = slab();
        let post = box_mesh(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 8.0));
        let offset = model.vertices.len() as u32;
        model.vertices.extend_from_slice(&post.vertices);
        model
            .faces
            .extend(post.faces.iter().map(|face| face.map(|at| at + offset)));
        let profile = SupportProfile {
            land_on_model: false,
            ..on_grid()
        };
        let table = [profile];
        let profiles = Profiles::new(&table).expect("one profile");
        let point = SupportPoint::new(Vec3::new(1.5, 2.0, 10.0));
        let trees = trees_on(&[point], &model, Transform::default(), profiles);

        let [tree] = trees.as_slice() else {
            panic!("the tip still stands");
        };
        let foot = tree.landing().base.truncate();
        assert!(
            foot.distance(Vec2::new(0.0, 5.0)) < 1e-4,
            "the next nearest node to 1.5, 2 past the post is 0, 5, got {foot}"
        );
    }
}
