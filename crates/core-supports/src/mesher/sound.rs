use super::foot::trunk_bottom;
use super::head::ground_normal;
use super::tube::{RING_EPSILON_MM, Ring, Tube};
use super::{Style, strut, trunk};
use crate::clear::{Beam, clear};
use crate::column::{SURFACE_EPSILON_MM, face_normal};
use crate::group::Profiles;
use crate::placed::Placed;
use crate::tree::SupportTree;
use core_geometry::{Ray, Scalar, Vec3};
use printer_profiles::SupportProfile;

/// One straight run of a support as it is drawn, with the outward normal of the face each
/// end is built to meet: the tip at its top, the part it stands on at its bottom.
pub(crate) struct Body {
    /// The group whose mesh it is drawn in.
    pub group: u16,
    pub tube: Tube,
    pub meets_top: Option<Vec3>,
    /// The point whose tip the tube carries from its contact, for a tube that starts at one.
    pub point: Option<usize>,
    /// The face's normal and how far down the axis it is met, millimetres: the tube
    /// itself runs on past it by its bite.
    pub meets_bottom: Option<(Vec3, Scalar)>,
}

/// Every strut of `tree` and the trunk under it, as the mesher draws them and as
/// [`sound`] tests them; the foot is the pad's, tested where it is placed.
pub(crate) fn bodies(tree: &SupportTree, profiles: Profiles, placed: &Placed) -> Vec<Body> {
    let profile = profiles.of(tree.group());
    let style = Style::of(tree, profile);
    let nodes = tree.nodes();
    let drawn_in = tree.node_groups(profiles);
    let mut bodies: Vec<Body> = tree
        .struts()
        .map(|(child, parent)| {
            let leaf = nodes[child].is_leaf();
            let (style, own) = if leaf {
                let into = nodes[child].position - nodes[parent].position;
                let style = style.at_tip(placed, nodes[child].position, into);
                (style, profiles.of(tree.head_group(child)))
            } else {
                (style, profile)
            };
            let tube = strut(tree, child, parent, own, style);
            Body {
                group: drawn_in[child],
                meets_top: leaf.then(|| style.face.unwrap_or(tube.direction)),
                point: nodes[child].point.filter(|_| leaf),
                meets_bottom: None,
                tube,
            }
        })
        .collect();
    bodies.push(trunk_body(tree, profile, style, placed));
    bodies
}

/// The trunk of `tree` as a body. A tree of one node has its tip on the trunk itself.
fn trunk_body(tree: &SupportTree, profile: &SupportProfile, style: Style, placed: &Placed) -> Body {
    let (root, landing) = (tree.root(), tree.landing());
    let style = Style {
        ground: ground_normal(placed, landing),
        ..style
    };
    let style = if root.is_leaf() {
        style.at_tip(
            placed,
            root.position,
            root.position - trunk_bottom(tree, profile),
        )
    } else {
        style
    };
    let tube = trunk(tree, profile, style);
    Body {
        group: tree.group(),
        meets_top: root.is_leaf().then(|| style.face.unwrap_or(tube.direction)),
        point: root.point.filter(|_| root.is_leaf()),
        meets_bottom: (landing.on_model && !landing.on_support).then(|| {
            let ground = style.ground.unwrap_or(-tube.direction);
            (ground, (landing.base - tube.top).dot(tube.direction))
        }),
        tube,
    }
}

impl Style {
    /// This style at a tip touching the model at `contact`, coming in along `into`: held
    /// to half the wall behind it, and laid on the face it holds.
    fn at_tip(self, placed: &Placed, contact: Vec3, into: Vec3) -> Self {
        Self {
            bite_limit_mm: wall_mm(placed, contact, into) * WALL_BITTEN,
            face: face_normal(placed, contact),
            ..self
        }
    }
}

/// How far a body may graze the model and still be sound, millimetres: a layer of the
/// finest printer, under which a facet of the model and a facet of the body only touch.
pub(super) const GRAZE_MM: Scalar = 0.05;

/// Whether every body of `tree`, as it is drawn, keeps out of the model past the ends
/// built to meet it: the one test a finished support is held to. See ADR 0337.
pub(crate) fn sound(tree: &SupportTree, profiles: Profiles, placed: &Placed) -> bool {
    bodies(tree, profiles, placed)
        .iter()
        .all(|body| body_clear(placed, body))
}

/// What of a tree fails [`sound`].
#[derive(Debug, PartialEq)]
pub(crate) enum Fault {
    /// Only the tubes of these points, from their contacts to where they join.
    Heads(Vec<usize>),
    /// A strut between joints, or the trunk.
    Trunk,
}

/// What of `tree` fails [`sound`], or `None` when nothing does.
pub(crate) fn faults(tree: &SupportTree, profiles: Profiles, placed: &Placed) -> Option<Fault> {
    let mut heads = Vec::new();
    for body in bodies(tree, profiles, placed) {
        if body_clear(placed, &body) {
            continue;
        }
        match body.point {
            Some(point) => heads.push(point),
            None => return Some(Fault::Trunk),
        }
    }
    (!heads.is_empty()).then_some(Fault::Heads(heads))
}

/// Whether `body` meets the model only where an end of it is built to.
pub(super) fn body_clear(placed: &Placed, body: &Body) -> bool {
    let rings = &body.tube.rings;
    let (Some(first), Some(last)) = (rings.first(), rings.last()) else {
        return true;
    };
    let start = first.along_mm.max(0.0);
    let end = body
        .meets_bottom
        .map_or(last.along_mm, |(_, face_mm)| face_mm.min(last.along_mm));
    let from_mm = match body.meets_top {
        Some(face) => match leaves(rings, body.tube.direction, face, start, end) {
            Some(along) => along,
            None => return false,
        },
        None => start,
    };
    let to_mm = match body.meets_bottom {
        Some((face, _)) => match leaves(rings, -body.tube.direction, face, end, start) {
            Some(along) => along,
            None => return false,
        },
        None => end,
    };
    let at = |along: Scalar| body.tube.top + body.tube.direction * along;
    let mut marks: Vec<Scalar> = rings
        .iter()
        .map(|ring| ring.along_mm)
        .filter(|along| *along > from_mm && *along < to_mm)
        .collect();
    marks.insert(0, from_mm);
    marks.push(to_mm);
    marks.windows(2).all(|pair| {
        let beam = Beam {
            from: at(pair[0]),
            to: at(pair[1]),
            from_radius_mm: (radius_at(rings, pair[0]) - GRAZE_MM).max(0.0),
            to_radius_mm: (radius_at(rings, pair[1]) - GRAZE_MM).max(0.0),
        };
        pair[1] - pair[0] < RING_EPSILON_MM || clear(placed, &beam)
    })
}

/// How far along the axis, `from_mm` towards `to_mm`, a body leaving the face with outward normal
/// `face` towards `away` stands a graze off its plane, or `None` if it never does.
pub(super) fn leaves(
    rings: &[Ring],
    away: Vec3,
    face: Vec3,
    from_mm: Scalar,
    to_mm: Scalar,
) -> Option<Scalar> {
    let off = away.dot(face);
    if off <= 0.0 {
        return None;
    }
    let slant = (1.0 - off * off).max(0.0).sqrt();
    let span = (to_mm - from_mm).abs();
    let toward = (to_mm - from_mm).signum();
    let mut gone = 0.0;
    while gone <= span {
        let along = toward.mul_add(gone, from_mm);
        let radius_mm = (radius_at(rings, along) - GRAZE_MM).max(0.0);
        if gone * off >= radius_mm.mul_add(slant, GRAZE_MM) {
            return Some(along);
        }
        gone += LEAVING_STEP_MM;
    }
    None
}

/// How finely [`leaves`] walks down a body for where it stands off its face, millimetres.
pub(super) const LEAVING_STEP_MM: Scalar = 0.02;

/// The radius of the tube `rings` draw at `along_mm`, straight between rings.
pub(super) fn radius_at(rings: &[Ring], along_mm: Scalar) -> Scalar {
    let below = rings.iter().rposition(|ring| ring.along_mm <= along_mm);
    match below {
        None => rings.first().map_or(0.0, |ring| ring.radius_mm),
        Some(index) => match rings.get(index + 1) {
            None => rings[index].radius_mm,
            Some(next) => {
                let ring = rings[index];
                let span = next.along_mm - ring.along_mm;
                if span < RING_EPSILON_MM {
                    ring.radius_mm.max(next.radius_mm)
                } else {
                    let share = (along_mm - ring.along_mm) / span;
                    ring.radius_mm + (next.radius_mm - ring.radius_mm) * share
                }
            }
        },
    }
}

/// How much of the wall behind a contact a tip may bite into: half, so that what is left
/// over it still holds and nothing of the tip shows on the far side.
pub(super) const WALL_BITTEN: Scalar = 0.5;

/// How thick the model is behind `contact`, going `into` it, mm: to where a ray leaves it, or
/// unbounded; a contact a hair off the face is measured from the face it meets first.
pub(super) fn wall_mm(placed: &Placed, contact: Vec3, into: Vec3) -> Scalar {
    let Some(direction) = into.try_normalize() else {
        return Scalar::INFINITY;
    };
    let mut from = contact + direction * SURFACE_EPSILON_MM;
    let mut entered_mm = SURFACE_EPSILON_MM;
    for _ in 0..2 {
        let ray = Ray::new(from, direction);
        let Some(hit) = placed.raycast(&ray) else {
            return Scalar::INFINITY;
        };
        if hit.normal.dot(direction) > 0.0 {
            return entered_mm + hit.distance;
        }
        from = hit.point + direction * SURFACE_EPSILON_MM;
        entered_mm = SURFACE_EPSILON_MM;
    }
    Scalar::INFINITY
}
