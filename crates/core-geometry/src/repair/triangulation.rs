use crate::triangulate::triangulate;
use crate::{Scalar, Triangle, Vec2, Vec3};

/// Smallest cross product treated as a direction, mm². Below it the loop is a line and
/// has no plane to be triangulated in.
pub(super) const MIN_NORMAL: Scalar = 1e-12;

/// Thinnest triangle a patch may hold, as twice its area over its longest edge squared:
/// below it the triangle is a sliver that would read as a face with no area.
const MIN_ASPECT: f64 = 1e-6;

/// The least-area triangulation of a loop of corners with no sliver, or `None` if none exists.
pub(super) fn least_area(points: &[Vec3]) -> Option<Vec<[u32; 3]>> {
    let n = points.len();
    let mut cost = vec![0.0f64; n * n];
    let mut split = vec![0u32; n * n];
    for span in 2..n {
        for first in 0..n - span {
            let last = first + span;
            let mut best = (f64::INFINITY, 0);
            for middle in first + 1..last {
                let inner = cost[first * n + middle] + cost[middle * n + last];
                if inner >= best.0 {
                    continue;
                }
                if let Some(area) = weight(points[first], points[middle], points[last])
                    && inner + area < best.0
                {
                    best = (inner + area, middle as u32);
                }
            }
            cost[first * n + last] = best.0;
            split[first * n + last] = best.1;
        }
    }
    if !cost[n - 1].is_finite() {
        return None;
    }

    let mut faces = Vec::with_capacity(n - 2);
    let mut spans = vec![(0, n - 1)];
    while let Some((first, last)) = spans.pop() {
        if last - first < 2 {
            continue;
        }
        let middle = split[first * n + last] as usize;
        faces.push([first as u32, middle as u32, last as u32]);
        spans.push((first, middle));
        spans.push((middle, last));
    }
    Some(faces)
}

/// The area of a triangle the patch may use, mm², or `None` for a sliver.
fn weight(a: Vec3, b: Vec3, c: Vec3) -> Option<f64> {
    let twice = f64::from((b - a).cross(c - a).length());
    let longest = (b - a)
        .length_squared()
        .max((c - b).length_squared())
        .max((a - c).length_squared());
    (twice > MIN_ASPECT * f64::from(longest)).then_some(twice / 2.0)
}

/// The loop ear clipped in its own plane, or `None` when it has none, its projection
/// folds over itself or a face would have no area.
pub(super) fn ear_clipped(points: &[Vec3]) -> Option<Vec<[u32; 3]>> {
    let normal = newell(points);
    if normal.length_squared() < MIN_NORMAL {
        return None;
    }
    let across = normal.normalize().any_orthonormal_vector();
    let up = normal.normalize().cross(across);
    let flat: Vec<Vec2> = points
        .iter()
        .map(|point| Vec2::new(point.dot(across), point.dot(up)))
        .collect();
    let corners: Vec<u32> = (0..points.len() as u32).collect();
    let faces = triangulate(&corners, &[], &flat);
    // Corners in a row can round into an ear in the plane; such a face has no area.
    let with_area = faces.iter().all(|face| {
        let [a, b, c] = face.map(|corner| points[corner as usize]);
        !Triangle::new(a, b, c).is_degenerate()
    });
    (faces.len() + 2 == points.len() && with_area).then_some(faces)
}

/// Newell's normal: the area-weighted normal of a loop that need not be flat, and
/// independent of where the loop stands.
pub(super) fn newell(points: &[Vec3]) -> Vec3 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(point, next)| point.cross(*next))
        .sum()
}
