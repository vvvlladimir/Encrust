//! How each support gives under the tip it holds, its tree read as a beam from the tip down:
//! see `docs/design/supports.md`, "How a support gives", and ADR 0455.

use core_geometry::{Scalar, Vec3};

use crate::base::ties;
use crate::mesher::{trunk_axis_at, trunk_radius_at_mm};
use crate::{Placed, Profiles, SupportPoint, SupportTree};

/// Poisson's ratio of a cured acrylate: a round strut twists `1 + ν` times as easily as it bends.
const POISSON: f64 = 0.35;

/// How a support gives at one tip, as compliance times Young's modulus, so that the
/// modulus stays the caller's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Give {
    /// The point it holds, an index into the points the trees were grown from.
    pub point: usize,
    /// The tree it stands on, an index into the trees given.
    pub tree: usize,
    /// Along z, per mm.
    pub axial_per_mm: f64,
    /// Across, per mm.
    pub lateral_per_mm: f64,
    /// Its top turning, per mm³.
    pub turning_per_mm3: f64,
    /// The second moment of its trunk, mm⁴.
    pub trunk_mm4: f64,
    /// How long its trunk stands free to buckle, mm: from its highest brace, or where it
    /// stands, up to its highest tip.
    pub free_mm: f64,
    /// Where it stands on the model, and the radius of the contact it meets it by, mm;
    /// `None` where it stands on the plate or on another support.
    pub stands_on: Option<(Vec3, Scalar)>,
}

/// One strut of a tree: its ends, its radius, mm, and how many tips share it.
#[derive(Debug, Clone, Copy)]
struct Strut {
    from: Vec3,
    to: Vec3,
    radius_mm: Scalar,
    shared: f64,
}

/// How each tip of `trees`, grown from `points`, gives. Braces hold a trunk still across below
/// the highest of them, so only its stretch is counted there.
pub fn gives(
    trees: &[SupportTree],
    points: &[SupportPoint],
    placed: &Placed,
    profiles: Profiles,
) -> Vec<Give> {
    let braced = braced_to(trees, placed, profiles);
    trees
        .iter()
        .enumerate()
        .flat_map(|(index, tree)| of_tree((index, tree), points, profiles, braced[index]))
        .collect()
}

/// The height of each tree's highest brace, or `None` where none ties it.
fn braced_to(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Vec<Option<Scalar>> {
    let mut braced = vec![None; trees.len()];
    for (one, two, struts) in ties(trees, placed, profiles) {
        let top = struts
            .iter()
            .map(|(from, to, _, _)| from.z.max(to.z))
            .fold(Scalar::MIN, Scalar::max);
        for tree in [one, two] {
            let was: &mut Option<Scalar> = &mut braced[tree];
            *was = Some(was.map_or(top, |was| was.max(top)));
        }
    }
    braced
}

/// How each tip of `tree` gives, its trunk braced up to `braced`.
fn of_tree<'t>(
    (index, tree): (usize, &'t SupportTree),
    points: &'t [SupportPoint],
    profiles: Profiles,
    braced: Option<Scalar>,
) -> impl Iterator<Item = Give> + 't {
    let profile = profiles.of(tree.group());
    let nodes = tree.nodes();
    let shared = tips_above(tree);
    let root = tree.root_index();
    let bottom = trunk_axis_at(tree, profile, Scalar::MIN);
    let top = nodes[root].position;
    let trunk_radius = trunk_radius_at_mm(tree, profile, Scalar::midpoint(top.z, bottom.z));
    let trunk = Strut {
        from: bottom,
        to: top,
        radius_mm: trunk_radius,
        shared: shared[root],
    };
    let floor = braced.map_or(bottom.z, |z| z.max(bottom.z));
    let highest = nodes
        .iter()
        .filter(|node| node.is_leaf())
        .map(|node| node.position.z)
        .fold(top.z, Scalar::max);
    let trunk_mm4 = std::f64::consts::PI * f64::from(trunk_radius).powi(4) / 4.0;
    let free_mm = f64::from(highest - floor).max(0.0);
    let landing = tree.landing();
    let stands_on = (landing.on_model && !landing.on_support)
        .then(|| (landing.base, profile.surface_radius_mm()));
    nodes.iter().enumerate().filter_map(move |(leaf, node)| {
        let point = node.point?;
        let contact = points
            .get(point)
            .map_or(node.position, |point| point.contact);
        let path = path_of(tree, (leaf, contact), &shared, trunk);
        let give = give_at(contact, &path, floor, (point, index), (trunk_mm4, free_mm));
        Some(Give { stands_on, ..give })
    })
}

/// The struts from `contact`, through its head to `leaf`, up the parents and down `trunk`.
fn path_of(
    tree: &SupportTree,
    (leaf, contact): (usize, Vec3),
    shared: &[f64],
    trunk: Strut,
) -> Vec<Strut> {
    let nodes = tree.nodes();
    let mut path = vec![Strut {
        from: nodes[leaf].position,
        to: contact,
        radius_mm: nodes[leaf].radius_mm,
        shared: 1.0,
    }];
    let mut at = leaf;
    // No longer than the tree is deep; the count guards a file with a loop in it.
    for _ in 0..nodes.len() {
        let Some(parent) = nodes[at].parent else {
            break;
        };
        path.push(Strut {
            from: nodes[parent].position,
            to: nodes[at].position,
            radius_mm: nodes[at].radius_mm,
            shared: shared[at],
        });
        at = parent;
    }
    path.push(trunk);
    path
}

/// How many tips hang from each node, itself included.
fn tips_above(tree: &SupportTree) -> Vec<f64> {
    let nodes = tree.nodes();
    let mut shared = vec![0.0; nodes.len()];
    for leaf in (0..nodes.len()).filter(|at| nodes[*at].is_leaf()) {
        let mut at = Some(leaf);
        for _ in 0..=nodes.len() {
            let Some(here) = at else { break };
            shared[here] += 1.0;
            at = nodes[here].parent;
        }
    }
    shared
}

fn give_at(
    contact: Vec3,
    path: &[Strut],
    floor: Scalar,
    (point, tree): (usize, usize),
    (trunk_mm4, free_mm): (f64, f64),
) -> Give {
    let force = |e: Vec3| move |x: Vec3| ((contact - x).cross(e), e);
    let moment = |m: Vec3| move |_: Vec3| (m, Vec3::ZERO);
    let sum = |load: &dyn Fn(Vec3) -> (Vec3, Vec3)| -> f64 {
        path.iter()
            .flat_map(|strut| split(*strut, floor))
            .map(|(strut, braced)| work(strut, braced, load))
            .sum()
    };
    let (x, y) = (Vec3::X, Vec3::Y);
    Give {
        point,
        tree,
        axial_per_mm: sum(&force(Vec3::Z)),
        lateral_per_mm: f64::midpoint(sum(&force(x)), sum(&force(y))),
        turning_per_mm3: f64::midpoint(sum(&moment(x)), sum(&moment(y))),
        trunk_mm4,
        free_mm,
        stands_on: None,
    }
}

/// `strut` cut at the height its tree is braced to: below it only its stretch counts.
fn split(strut: Strut, floor: Scalar) -> Vec<(Strut, bool)> {
    let (low, high) = if strut.from.z <= strut.to.z {
        (strut.from, strut.to)
    } else {
        (strut.to, strut.from)
    };
    if high.z <= floor {
        return vec![(strut, true)];
    }
    if low.z >= floor {
        return vec![(strut, false)];
    }
    let cut = low.lerp(high, (floor - low.z) / (high.z - low.z));
    vec![
        (
            Strut {
                from: low,
                to: cut,
                ..strut
            },
            true,
        ),
        (
            Strut {
                from: cut,
                to: high,
                ..strut
            },
            false,
        ),
    ]
}

/// The strain energy over `strut`, times Young's modulus, of a unit `load` that answers the
/// moment and force it makes at each point; Simpson's rule is exact, the moment being linear.
fn work(strut: Strut, braced: bool, load: &dyn Fn(Vec3) -> (Vec3, Vec3)) -> f64 {
    let along = strut.to - strut.from;
    let length = f64::from(along.length());
    if length <= 0.0 {
        return 0.0;
    }
    let axis = along / along.length();
    let r = f64::from(strut.radius_mm);
    let (area, second) = (
        std::f64::consts::PI * r * r,
        std::f64::consts::PI * r.powi(4) / 4.0,
    );
    let density = |x: Vec3| {
        let (moment, force) = load(x);
        let normal = f64::from(force.dot(axis));
        let stretch = normal * normal / area;
        if braced {
            return stretch;
        }
        let twist = f64::from(moment.dot(axis));
        let bend = f64::from(moment.length_squared()) - twist * twist;
        stretch + (bend + twist * twist * (1.0 + POISSON)) / second
    };
    let mid = strut.from.lerp(strut.to, 0.5);
    strut.shared * length / 6.0 * (density(strut.from) + 4.0 * density(mid) + density(strut.to))
}

#[cfg(test)]
mod tests;
