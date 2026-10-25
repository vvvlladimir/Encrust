use core_geometry::{Mesh, Scalar, Vec2};
use printer_profiles::{RaftShape, RaftStyle, SupportProfile};
use rayon::prelude::*;

use crate::base::{SAME_PLACE_MM, hull};
use crate::group::Profiles;
use crate::mesher::footing_radius_mm;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// Model vertices one parallel hull takes before the hulls of the chunks are merged.
const HULL_CHUNK: usize = 16_384;

/// The most one segment of a rounded corner turns through, radians: fifteen degrees.
const ARC_STEP_RAD: Scalar = std::f32::consts::PI / 12.0;

/// Sides of the polygon a round foot is drawn as.
const FOOT_SIDES: usize = 16;

/// Width of one bar of a grid raft, and the side of the square hole between two,
/// millimetres.
pub const GRID_BAR_MM: Scalar = 1.0;
pub const GRID_HOLE_MM: Scalar = 2.0;

/// Width of a skate raft's lip, millimetres. It stands as tall as the slab is thick.
pub const LIP_WIDTH_MM: Scalar = 1.0;

/// The raft under every support that reaches the plate, or `None` when none does; built to the
/// ground profile, supports pass through it by the overlap rule (ADR 0041, 0241).
pub(crate) fn raft(trees: &[SupportTree], placed: &Placed, profiles: Profiles) -> Option<Mesh> {
    let profile = profiles.ground();
    let raft = &profile.raft;
    if !raft.enabled {
        return None;
    }
    let feet: Vec<Foot> = trees
        .iter()
        .filter(|tree| !tree.landing().on_model)
        .map(|tree| Foot {
            centre: tree.landing().base.truncate(),
            radius_mm: footing_radius_mm(tree, profiles.of(tree.group())),
        })
        .collect();
    if feet.is_empty() {
        return None;
    }

    let (overhang, thickness) = (raft.overhang_mm(), raft.thickness_mm);
    let mut mesh = Mesh::default();
    match raft.style {
        RaftStyle::Skates => {
            let top = outline(&feet, placed, profile, overhang + LIP_WIDTH_MM);
            saucer(&mut mesh, &top, overhang, thickness);
        }
        RaftStyle::Grid => {
            let top = outline(&feet, placed, profile, overhang + GRID_BAR_MM);
            grid(&mut mesh, &top, &feet, profile);
        }
        RaftStyle::Connected => connected(&mut mesh, &feet, profile),
    }
    (!mesh.faces.is_empty()).then_some(mesh)
}

/// Where a support meets the raft, and how wide.
#[derive(Debug, Clone, Copy)]
struct Foot {
    centre: Vec2,
    radius_mm: Scalar,
}

/// The outline a skates or grid raft is drawn to: footprint and every foot pushed `margin_mm` out,
/// counter-clockwise; a corner is never tighter than twice `rim_mm` or the inset inverts it.
fn outline(feet: &[Foot], placed: &Placed, profile: &SupportProfile, rim_mm: Scalar) -> Vec<Vec2> {
    let mut points = footprint(placed);
    for foot in feet {
        points.extend(circle(foot.centre, foot.radius_mm));
    }
    let around = hull(&points);
    let margin = profile.raft.margin_mm.max(0.0);
    match profile.raft.shape {
        RaftShape::Hull => {
            let corner = margin.max(2.0 * rim_mm);
            // Half the inradius at most, so a narrow footprint is never drawn in to nothing.
            let inset = (corner - margin).min(inradius(&around) * 0.5);
            rounded(&hull(&mitred(&around, -inset)), corner)
        }
        RaftShape::Rectangle => mitred(&smallest_rectangle(&around), margin),
    }
}

/// The convex hull of the model seen from above, taken in parallel over its vertices.
fn footprint(placed: &Placed) -> Vec<Vec2> {
    let matrix = placed.transform.to_matrix();
    placed
        .model
        .vertices
        .par_chunks(HULL_CHUNK)
        .flat_map_iter(|chunk| {
            let flat: Vec<Vec2> = chunk
                .iter()
                .map(|&vertex| matrix.transform_point3(vertex).truncate())
                .collect();
            hull(&flat)
        })
        .collect()
}

/// The smallest-area rectangle around a convex `outline`, counter-clockwise. One of its
/// sides always lies along a side of the outline, so only those directions are tried.
fn smallest_rectangle(outline: &[Vec2]) -> Vec<Vec2> {
    let count = outline.len();
    let best = (0..count)
        .into_par_iter()
        .filter_map(|index| {
            let along = (outline[(index + 1) % count] - outline[index]).try_normalize()?;
            let across = along.perp();
            let (mut low, mut high) = (
                Vec2::splat(Scalar::INFINITY),
                Vec2::splat(Scalar::NEG_INFINITY),
            );
            for &point in outline {
                let local = Vec2::new(point.dot(along), point.dot(across));
                (low, high) = (low.min(local), high.max(local));
            }
            let size = high - low;
            Some((size.x * size.y, along, low, high))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, along, low, high)) = best else {
        return outline.to_vec();
    };
    let across = along.perp();
    [
        (low.x, low.y),
        (high.x, low.y),
        (high.x, high.y),
        (low.x, high.y),
    ]
    .into_iter()
    .map(|(u, v)| along * u + across * v)
    .collect()
}

/// The outward normal of the edge `from`..`to` of a counter-clockwise polygon.
fn normal(from: Vec2, to: Vec2) -> Vec2 {
    -(to - from).normalize_or_zero().perp()
}

/// `outline` pushed `margin_mm` out with every corner rounded: its Minkowski sum with a
/// disc.
fn rounded(outline: &[Vec2], margin_mm: Scalar) -> Vec<Vec2> {
    let count = outline.len();
    if margin_mm <= SAME_PLACE_MM || count < 3 {
        return outline.to_vec();
    }
    let mut out = Vec::new();
    for index in 0..count {
        let here = outline[index];
        let before = normal(outline[(index + count - 1) % count], here);
        let after = normal(here, outline[(index + 1) % count]);
        let turn = before.perp_dot(after).atan2(before.dot(after));
        if turn <= SAME_PLACE_MM {
            out.push(here + before * margin_mm);
        } else {
            arc(&mut out, here, before, turn, margin_mm);
        }
    }
    out
}

/// The points of an arc of `radius_mm` about `centre`, from direction `start` turning
/// `turn_rad` counter-clockwise, both ends included.
fn arc(out: &mut Vec<Vec2>, centre: Vec2, start: Vec2, turn_rad: Scalar, radius_mm: Scalar) {
    let steps = (turn_rad / ARC_STEP_RAD).ceil().max(1.0) as usize;
    for step in 0..=steps {
        let angle = turn_rad * step as Scalar / steps as Scalar;
        out.push(centre + Vec2::from_angle(angle).rotate(start) * radius_mm);
    }
}

/// Every side of a convex `outline` moved `distance_mm` out, or in when negative, its
/// corners kept sharp. The vertex count does not change.
fn mitred(outline: &[Vec2], distance_mm: Scalar) -> Vec<Vec2> {
    let count = outline.len();
    (0..count)
        .map(|index| {
            let here = outline[index];
            let before = normal(outline[(index + count - 1) % count], here);
            let after = normal(here, outline[(index + 1) % count]);
            let scale = 1.0 + before.dot(after);
            if scale < SAME_PLACE_MM {
                here
            } else {
                here + (before + after) * (distance_mm / scale)
            }
        })
        .collect()
}

/// The distance from the middle of a convex `outline` to its nearest side.
fn inradius(outline: &[Vec2]) -> Scalar {
    let count = outline.len();
    let middle = outline.iter().fold(Vec2::ZERO, |sum, &p| sum + p) / count.max(1) as Scalar;
    (0..count)
        .map(|index| {
            let here = outline[index];
            normal(here, outline[(index + 1) % count]).dot(here - middle)
        })
        .fold(Scalar::INFINITY, Scalar::min)
}

fn circle(centre: Vec2, radius_mm: Scalar) -> Vec<Vec2> {
    (0..FOOT_SIDES)
        .map(|side| {
            let angle = std::f32::consts::TAU * side as Scalar / FOOT_SIDES as Scalar;
            centre + Vec2::from_angle(angle) * radius_mm
        })
        .collect()
}

/// A closed slab: `outline` at `thickness_mm`, pulled `overhang_mm` in where it meets the plate
/// so the narrow rim is on the plate (ADR 0044); the pull is held to half the inradius.
fn slab(mesh: &mut Mesh, outline: &[Vec2], overhang_mm: Scalar, thickness_mm: Scalar) {
    let count = outline.len();
    if count < 3 {
        return;
    }
    let middle = outline.iter().fold(Vec2::ZERO, |sum, &p| sum + p) / count as Scalar;
    let overhang = overhang_mm.min(inradius(outline) * 0.5).max(0.0);

    let first = mesh.vertices.len() as u32;
    for point in mitred(outline, -overhang) {
        mesh.vertices.push(point.extend(0.0));
    }
    for point in outline {
        mesh.vertices.push(point.extend(thickness_mm));
    }
    let low_middle = mesh.vertices.len() as u32;
    mesh.vertices.push(middle.extend(0.0));
    let high_middle = low_middle + 1;
    mesh.vertices.push(middle.extend(thickness_mm));

    let low = |index: usize| first + (index % count) as u32;
    let high = |index: usize| first + (count + index % count) as u32;
    for index in 0..count {
        mesh.faces.push([low_middle, low(index + 1), low(index)]);
        mesh.faces.push([high_middle, high(index), high(index + 1)]);
        mesh.faces
            .push([low(index), low(index + 1), high(index + 1)]);
        mesh.faces.push([low(index), high(index + 1), high(index)]);
    }
}

/// A grid raft: a solid rim around `outline`, bars both ways across its inside, and a pad
/// under every foot so none of them stands over a hole.
fn grid(mesh: &mut Mesh, outline: &[Vec2], feet: &[Foot], profile: &SupportProfile) {
    let raft = &profile.raft;
    let (overhang, thickness) = (raft.overhang_mm(), raft.thickness_mm);
    let rim = overhang + GRID_BAR_MM;
    if inradius(outline) < 2.0 * rim + GRID_HOLE_MM {
        slab(mesh, outline, overhang, thickness);
        return;
    }

    let inner = mitred(outline, -rim);
    ring(
        mesh,
        outline,
        &mitred(outline, -overhang),
        &inner,
        thickness,
    );
    let along = longest_side(outline);
    for direction in [along, along.perp()] {
        for bar in bars(&inner, direction, rim - overhang) {
            slab(mesh, &bar, 0.0, thickness);
        }
    }
    for foot in feet {
        slab(mesh, &circle(foot.centre, foot.radius_mm), 0.0, thickness);
    }
}

/// A closed band: `top` at `thickness_mm` leaning in to `bottom` on the plate outside,
/// and a straight wall down `inner` inside. All three have the same vertex count.
fn ring(mesh: &mut Mesh, top: &[Vec2], bottom: &[Vec2], inner: &[Vec2], thickness_mm: Scalar) {
    let count = top.len();
    let first = mesh.vertices.len() as u32;
    for (points, z) in [
        (bottom, 0.0),
        (top, thickness_mm),
        (inner, thickness_mm),
        (inner, 0.0),
    ] {
        mesh.vertices
            .extend(points.iter().map(|point| point.extend(z)));
    }
    let at = |layer: u32, index: usize| first + layer * count as u32 + (index % count) as u32;
    let (outer_low, outer_high, inner_high, inner_low) = (0, 1, 2, 3);
    for index in 0..count {
        let next = index + 1;
        for [a, b, c] in [
            [(outer_low, index), (outer_low, next), (outer_high, next)],
            [(outer_low, index), (outer_high, next), (outer_high, index)],
            [(outer_high, index), (outer_high, next), (inner_high, next)],
            [(outer_high, index), (inner_high, next), (inner_high, index)],
            [(inner_low, index), (inner_high, next), (inner_low, next)],
            [(inner_low, index), (inner_high, index), (inner_high, next)],
            [(outer_low, index), (inner_low, next), (outer_low, next)],
            [(outer_low, index), (inner_low, index), (inner_low, next)],
        ] {
            mesh.faces.push([at(a.0, a.1), at(b.0, b.1), at(c.0, c.1)]);
        }
    }
}

/// The direction of the longest side of `outline`, which the grid's bars follow.
fn longest_side(outline: &[Vec2]) -> Vec2 {
    let count = outline.len();
    (0..count)
        .map(|index| outline[(index + 1) % count] - outline[index])
        .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .and_then(Vec2::try_normalize)
        .unwrap_or(Vec2::X)
}

/// The bars running along `along` across a convex `inner`, a hole apart, each reaching
/// `reach_mm` past `inner` into the rim around it.
fn bars(inner: &[Vec2], along: Vec2, reach_mm: Scalar) -> Vec<Vec<Vec2>> {
    let across = along.perp();
    let (low, high) = inner
        .iter()
        .map(|point| point.dot(across))
        .fold((Scalar::INFINITY, Scalar::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    let pitch = GRID_BAR_MM + GRID_HOLE_MM;
    let middle = Scalar::midpoint(low, high);
    let half = GRID_BAR_MM / 2.0;
    let count = ((high - low) / pitch).ceil() as i32;

    (-count..=count)
        .filter_map(|step| {
            let offset = middle + step as Scalar * pitch;
            let (start, end) = span(inner, along, offset - half)?;
            let (other_start, other_end) = span(inner, along, offset + half)?;
            let (start, end) = (
                start.max(other_start) - reach_mm,
                end.min(other_end) + reach_mm,
            );
            (end > start).then(|| {
                let at = |t: Scalar, side: Scalar| along * t + across * (offset + side);
                vec![
                    at(start, -half),
                    at(end, -half),
                    at(end, half),
                    at(start, half),
                ]
            })
        })
        .collect()
}

/// Where the line `across · p = offset` enters and leaves a convex `outline`, measured
/// along `along`, or `None` when it misses.
fn span(outline: &[Vec2], along: Vec2, offset: Scalar) -> Option<(Scalar, Scalar)> {
    let across = along.perp();
    let count = outline.len();
    let mut range: Option<(Scalar, Scalar)> = None;
    for index in 0..count {
        let (a, b) = (outline[index], outline[(index + 1) % count]);
        let (da, db) = (a.dot(across) - offset, b.dot(across) - offset);
        if (da > 0.0) == (db > 0.0) || (da - db).abs() < SAME_PLACE_MM {
            continue;
        }
        let t = (a + (b - a) * (da / (da - db))).dot(along);
        range = Some(range.map_or((t, t), |(lo, hi)| (lo.min(t), hi.max(t))));
    }
    range
}

/// A skate raft: a slab whose sloped wall carries up past its top into a lip round the edge,
/// the floor inside at `thickness_mm`, one closed solid; the lip is `LIP_WIDTH_MM` wide throughout.
fn saucer(mesh: &mut Mesh, outline: &[Vec2], overhang_mm: Scalar, thickness_mm: Scalar) {
    let count = outline.len();
    let lean = overhang_mm / thickness_mm.max(SAME_PLACE_MM);
    let lip_top = thickness_mm * 2.0;
    let flare = lean * (lip_top - thickness_mm);
    if count < 3 || inradius(outline) < 2.0 * (overhang_mm + LIP_WIDTH_MM) {
        slab(mesh, outline, overhang_mm, thickness_mm);
        return;
    }

    let first = mesh.vertices.len() as u32;
    let layers = [
        (mitred(outline, -overhang_mm), 0.0),
        (mitred(outline, flare), lip_top),
        (mitred(outline, flare - LIP_WIDTH_MM), lip_top),
        (mitred(outline, -LIP_WIDTH_MM), thickness_mm),
    ];
    for (points, z) in &layers {
        mesh.vertices
            .extend(points.iter().map(|point| point.extend(*z)));
    }
    let middle = outline.iter().fold(Vec2::ZERO, |sum, &p| sum + p) / count as Scalar;
    let (low_middle, floor_middle) = (first + 4 * count as u32, first + 4 * count as u32 + 1);
    mesh.vertices.push(middle.extend(0.0));
    mesh.vertices.push(middle.extend(thickness_mm));

    let at = |layer: u32, index: usize| first + layer * count as u32 + (index % count) as u32;
    let (bottom, rim, lip, floor) = (0, 1, 2, 3);
    for index in 0..count {
        let next = index + 1;
        mesh.faces
            .push([low_middle, at(bottom, next), at(bottom, index)]);
        mesh.faces
            .push([floor_middle, at(floor, index), at(floor, next)]);
        for [a, b, c] in [
            [(bottom, index), (bottom, next), (rim, next)],
            [(bottom, index), (rim, next), (rim, index)],
            [(rim, index), (rim, next), (lip, next)],
            [(rim, index), (lip, next), (lip, index)],
            [(floor, index), (lip, next), (floor, next)],
            [(floor, index), (lip, index), (lip, next)],
        ] {
            mesh.faces.push([at(a.0, a.1), at(b.0, b.1), at(c.0, c.1)]);
        }
    }
}

/// A connected raft: a pad under every foot and a strip a foot wide along the shortest
/// network that joins them all, their minimum spanning tree.
fn connected(mesh: &mut Mesh, feet: &[Foot], profile: &SupportProfile) {
    let raft = &profile.raft;
    let (overhang, thickness) = (raft.overhang_mm(), raft.thickness_mm);
    let margin = raft.margin_mm.max(0.0);
    for foot in feet {
        let pad = circle(foot.centre, foot.radius_mm + margin);
        slab(mesh, &pad, overhang, thickness);
    }
    let centres: Vec<Vec2> = feet.iter().map(|foot| foot.centre).collect();
    for (one, two) in spanning_tree(&centres) {
        let (from, to) = (centres[one], centres[two]);
        let radius = feet[one].radius_mm.min(feet[two].radius_mm);
        let side = (to - from).normalize_or_zero().perp() * radius;
        let strip = [from - side, to - side, to + side, from + side];
        slab(mesh, &strip, overhang, thickness);
    }
}

/// The edges of the Euclidean minimum spanning tree of `points`, by Prim's algorithm on
/// the complete graph. Quadratic in the feet, which number in the hundreds, not the faces.
fn spanning_tree(points: &[Vec2]) -> Vec<(usize, usize)> {
    let count = points.len();
    let mut joined = vec![false; count];
    let mut nearest: Vec<(Scalar, usize)> = vec![(Scalar::INFINITY, 0); count];
    let mut edges = Vec::with_capacity(count.saturating_sub(1));
    let mut latest = 0;
    for _ in 1..count {
        joined[latest] = true;
        let mut best: Option<usize> = None;
        for index in 0..count {
            if joined[index] {
                continue;
            }
            let distance = (points[index] - points[latest]).length_squared();
            if distance < nearest[index].0 {
                nearest[index] = (distance, latest);
            }
            if best.is_none_or(|other| nearest[index].0 < nearest[other].0) {
                best = Some(index);
            }
        }
        let Some(next) = best else { break };
        edges.push((nearest[next].1, next));
        latest = next;
    }
    edges
}

#[cfg(test)]
mod tests;
