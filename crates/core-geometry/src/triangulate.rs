//! Filling a flat ring with triangles, which both a cut's cap and a patch over a hole
//! need; see `docs/decisions/0089`.

use crate::{FastMap, Vec2};

/// Fills one ring and the rings inside it, by ear clipping. The corners are indices into
/// `flat`, and the triangles come back wound as the outer ring is.
pub(crate) fn triangulate(outer: &[u32], holes: &[&Vec<u32>], flat: &[Vec2]) -> Vec<[u32; 3]> {
    let mut patches = Vec::new();
    let mut ring: Vec<u32> = without_repeats(outer, flat, &mut patches);
    let mut hole_starts = Vec::with_capacity(holes.len());
    for hole in holes {
        let hole = without_repeats(hole, flat, &mut patches);
        if hole.len() < 3 {
            continue;
        }
        hole_starts.push(ring.len());
        ring.extend(hole);
    }

    let coordinates: Vec<f64> = ring
        .iter()
        .flat_map(|point| {
            let flat = flat[*point as usize];
            [f64::from(flat.x), f64::from(flat.y)]
        })
        .collect();

    let Ok(indices) = earcutr::earcut(&coordinates, &hole_starts, 2) else {
        return Vec::new();
    };
    let mut triangles: Vec<[u32; 3]> = indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|corner| [ring[corner[0]], ring[corner[1]], ring[corner[2]]])
        .collect();
    cover_repeats(&mut triangles, patches);
    flip_flat(&mut triangles, flat);
    triangles
}

/// Adds the triangle each repeated corner needs, wound against the edge the fill left
/// where the corner was taken out.
fn cover_repeats(triangles: &mut Vec<[u32; 3]>, patches: Vec<[u32; 3]>) {
    let across = facing_edges(triangles);
    for [from, corner, to] in patches {
        if across.contains_key(&(to, from)) {
            triangles.push([to, corner, from]);
        } else {
            triangles.push([from, corner, to]);
        }
    }
}

/// The ring with the corners that stand on the one before them taken out, each of them
/// left in `patches` as the triangle that covers the two edges reaching it.
///
/// Ear clipping drops such a corner rather than covering those edges, which leaves a hole
/// in the surface wherever the model touched itself on the plane.
fn without_repeats(ring: &[u32], flat: &[Vec2], patches: &mut Vec<[u32; 3]>) -> Vec<u32> {
    let place = |corner: u32| flat[corner as usize];
    let mut kept: Vec<u32> = Vec::with_capacity(ring.len());
    let mut repeats: Vec<(usize, u32)> = Vec::new();
    for corner in ring.iter().copied() {
        match kept.last() {
            Some(last) if place(*last) == place(corner) => repeats.push((kept.len(), corner)),
            _ => kept.push(corner),
        }
    }
    // The ring closes, so its last corner can stand on its first as well as on its own
    // neighbour.
    if kept.len() > 2
        && place(kept[0]) == place(kept[kept.len() - 1])
        && let Some(last) = kept.pop()
    {
        repeats.push((kept.len(), last));
    }
    if kept.len() < 3 {
        return kept;
    }
    for (at, corner) in repeats {
        patches.push([kept[at - 1], corner, kept[at % kept.len()]]);
    }
    kept
}

/// How many times the flipping sweep goes round. A sweep takes every flat triangle whose
/// neighbour it has not just moved, so the few left for the next one are soon gone.
const PASSES: usize = 8;

/// Flips the triangles with no area out of a filled ring, by handing each to the triangle
/// across its longest edge: the usual diagonal flip, and the quadrilateral the two cover
/// is convex because one of them is flat.
///
/// Ear clipping answers three corners in a straight line with a triangle of no area, which
/// covers the ring but cuts into two coincident points when a plane crosses it later.
fn flip_flat(triangles: &mut [[u32; 3]], flat: &[Vec2]) {
    for _ in 0..PASSES {
        let across = facing_edges(triangles);
        let mut taken = vec![false; triangles.len()];
        let mut flipped = false;
        for index in 0..triangles.len() {
            let Some(corner) = flat_corner(triangles[index], flat).filter(|_| !taken[index]) else {
                continue;
            };
            let [x, y, m] = rotate(triangles[index], corner);
            let Some(other) = across.get(&(y, x)).copied().filter(|other| !taken[*other]) else {
                continue;
            };
            let Some(d) = triangles[other]
                .iter()
                .copied()
                .find(|v| *v != x && *v != y)
            else {
                continue;
            };
            triangles[index] = [y, m, d];
            triangles[other] = [m, x, d];
            (taken[index], taken[other]) = (true, true);
            flipped = true;
        }
        if !flipped {
            return;
        }
    }
}

/// Which triangle each directed edge belongs to. A ring's triangles are all wound the
/// same way, so an inside edge appears once each way.
fn facing_edges(triangles: &[[u32; 3]]) -> FastMap<(u32, u32), usize> {
    let mut edges = FastMap::default();
    for (index, triangle) in triangles.iter().enumerate() {
        for corner in 0..3 {
            edges.insert((triangle[corner], triangle[(corner + 1) % 3]), index);
        }
    }
    edges
}

/// The corner a triangle with no area folds at: the one its longest edge faces, or `None`
/// when the triangle has area.
fn flat_corner(triangle: [u32; 3], flat: &[Vec2]) -> Option<usize> {
    let points = triangle.map(|corner| flat[corner as usize]);
    if (points[1] - points[0]).perp_dot(points[2] - points[0]) != 0.0 {
        return None;
    }
    (0..3).max_by(|a, b| {
        let span =
            |corner: usize| points[(corner + 1) % 3].distance_squared(points[(corner + 2) % 3]);
        span(*a).total_cmp(&span(*b))
    })
}

/// The triangle read from `corner` on, as the edge facing it and then the corner itself.
fn rotate(triangle: [u32; 3], corner: usize) -> [u32; 3] {
    [
        triangle[(corner + 1) % 3],
        triangle[(corner + 2) % 3],
        triangle[corner],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How often each undirected edge of the fill is used.
    fn edge_uses(triangles: &[[u32; 3]]) -> FastMap<(u32, u32), usize> {
        let mut uses: FastMap<(u32, u32), usize> = FastMap::default();
        for triangle in triangles {
            for corner in 0..3 {
                let (a, b) = (triangle[corner], triangle[(corner + 1) % 3]);
                *uses.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        uses
    }

    /// Every edge of the ring is covered once, which is what the wall on the other side
    /// of it needs, and nothing the fill added is left on its own.
    fn covers(ring: &[u32], triangles: &[[u32; 3]]) {
        let uses = edge_uses(triangles);
        for corner in 0..ring.len() {
            let (a, b) = (ring[corner], ring[(corner + 1) % ring.len()]);
            if a == b {
                continue;
            }
            let key = (a.min(b), a.max(b));
            assert_eq!(uses.get(&key), Some(&1), "the ring edge {key:?} is covered");
        }
        for ((a, b), count) in uses {
            let on_ring = ring
                .iter()
                .enumerate()
                .any(|(corner, point)| *point == a && ring[(corner + 1) % ring.len()] == b)
                || ring
                    .iter()
                    .enumerate()
                    .any(|(corner, point)| *point == b && ring[(corner + 1) % ring.len()] == a);
            if !on_ring {
                assert_eq!(count, 2, "the edge {a}-{b} the fill added is used twice");
            }
        }
    }

    #[test]
    fn a_triangle_with_no_area_is_flipped_into_the_one_beside_it() {
        // Corner 1 stands halfway along the edge from corner 0 to corner 2, so the ear
        // over the three of them has no area.
        let flat = [
            Vec2::new(0.0, 0.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(5.0, 10.0),
        ];
        let mut triangles = [[0, 1, 2], [0, 2, 3]];

        flip_flat(&mut triangles, &flat);

        for triangle in &triangles {
            let points = triangle.map(|corner| flat[corner as usize]);
            let area = (points[1] - points[0]).perp_dot(points[2] - points[0]);
            assert!(area != 0.0, "{triangle:?} has area, got {area}");
        }
        covers(&[0, 1, 2, 3], &triangles);
    }

    #[test]
    fn a_corner_standing_on_the_one_before_it_is_still_covered() {
        // Corner 3 is corner 2 over again, as a section has wherever the model touched
        // itself on the plane.
        let flat = [
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        let ring = [0, 1, 2, 3, 4];

        let triangles = triangulate(&ring, &[], &flat);

        covers(&ring, &triangles);
    }
}
