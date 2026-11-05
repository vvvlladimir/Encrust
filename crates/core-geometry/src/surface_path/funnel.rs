use std::iter::once;

use super::unfold::Strip;
use crate::{Scalar, Vec2, Vec3};

/// Where the taut walk through `strip` crosses each of its portals, placed, in order.
pub(super) fn crossings(strip: &Strip) -> Vec<Vec3> {
    let corners = corners(strip);
    strip
        .portals
        .iter()
        .enumerate()
        .filter_map(|(index, portal)| {
            let gate = index + 1;
            let after = corners.partition_point(|(_, at)| *at <= gate);
            let (from, to) = (corners.get(after.checked_sub(1)?)?.0, corners.get(after)?.0);
            let t = across(portal.left, portal.right, from, to);
            Some(portal.left_at.lerp(portal.right_at, t))
        })
        .collect()
}

/// The turning points of the taut walk, each with the gate it is on: the simple stupid
/// funnel algorithm (Mononen, 2010), over the start, the portals, then the end as gates.
fn corners(strip: &Strip) -> Vec<(Vec2, usize)> {
    let gates: Vec<(Vec2, Vec2)> = once((strip.start, strip.start))
        .chain(
            strip
                .portals
                .iter()
                .map(|portal| (portal.left, portal.right)),
        )
        .chain(once((strip.end, strip.end)))
        .collect();
    let mut corners = vec![(strip.start, 0)];
    let (mut apex, mut left, mut right) = ((strip.start, 0), (strip.start, 0), (strip.start, 0));
    let mut gate = 1;
    // A restart moves the apex to a later gate, so there are fewer than `gates.len()`.
    while let Some(&(next_left, next_right)) = gates.get(gate) {
        if turn(apex.0, right.0, next_right) >= 0.0 {
            if apex.0 == right.0 || turn(apex.0, left.0, next_right) < 0.0 {
                right = (next_right, gate);
            } else {
                apex = left;
                corners.push(apex);
                (left, right, gate) = (apex, apex, apex.1 + 1);
                continue;
            }
        }
        if turn(apex.0, left.0, next_left) <= 0.0 {
            if apex.0 == left.0 || turn(apex.0, right.0, next_left) > 0.0 {
                left = (next_left, gate);
            } else {
                apex = right;
                corners.push(apex);
                (left, right, gate) = (apex, apex, apex.1 + 1);
                continue;
            }
        }
        gate += 1;
    }
    let last = gates.len() - 1;
    if corners.last().is_none_or(|corner| corner.1 < last) {
        corners.push((strip.end, last));
    }
    corners
}

/// Positive when `b` lies left of the ray from `apex` through `a`.
fn turn(apex: Vec2, a: Vec2, b: Vec2) -> Scalar {
    (a - apex).perp_dot(b - apex)
}

/// How far along the portal from `left` to `right` the segment from `from` to `to` meets
/// it, from 0 to 1; the nearest point to `from` when the two run parallel.
fn across(left: Vec2, right: Vec2, from: Vec2, to: Vec2) -> Scalar {
    let (portal, segment) = (right - left, to - from);
    let denominator = portal.perp_dot(segment);
    let t = if denominator.abs() > Scalar::EPSILON * portal.length() * segment.length() {
        (from - left).perp_dot(segment) / denominator
    } else {
        (from - left).dot(portal) / portal.length_squared().max(Scalar::MIN_POSITIVE)
    };
    t.clamp(0.0, 1.0)
}
