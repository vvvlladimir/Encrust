//! Filling a flat ring with triangles, which both a cut's cap and a patch over a hole
//! need; see `docs/decisions/0089`.

use crate::{FastMap, FastSet, Vec2};

/// Fills one ring and the rings inside it, by ear clipping: a cut's cap, a patch over a hole,
/// a letter of a label. The corners are indices into `flat`, and the triangles come back
/// wound as the outer ring is.
pub fn triangulate(outer: &[u32], holes: &[&Vec<u32>], flat: &[Vec2]) -> Vec<[u32; 3]> {
    let mut patches = Vec::new();
    let mut ring: Vec<u32> = without_repeats(outer, flat, &mut patches);
    let mut kept = vec![ring.clone()];
    let mut hole_starts = Vec::with_capacity(holes.len());
    for hole in holes {
        let hole = without_repeats(hole, flat, &mut patches);
        if hole.len() < 3 {
            continue;
        }
        hole_starts.push(ring.len());
        ring.extend(&hole);
        kept.push(hole);
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
    cover_dropped(&mut triangles, &kept, flat);
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

/// How far off the line between its neighbours a dropped corner may stand and still be put
/// back on it, as a share of that line's length squared: rounding, not a bend.
const ON_LINE: f32 = 1e-6;

/// Puts back the corners of `rings` that ear clipping passed over for lying on the line
/// between two others, fanning the triangle over that line out to each of them.
///
/// The wall standing on the ring still runs to every one of them, so without them the fill
/// and the wall meet along nothing there and the surface has a slit. Such a line is an edge
/// of the fill with no triangle beyond it that is not a side of a ring, and the corners left
/// out are those with a side of their ring no triangle covers.
fn cover_dropped(triangles: &mut Vec<[u32; 3]>, rings: &[Vec<u32>], flat: &[Vec2]) {
    let mut across = facing_edges(triangles);
    let covered = |a: u32, b: u32, across: &FastMap<(u32, u32), usize>| {
        across.contains_key(&(a, b)) || across.contains_key(&(b, a))
    };
    let mut sides: FastSet<(u32, u32)> = FastSet::default();
    let mut left_out: Vec<u32> = Vec::new();
    for ring in rings {
        for (at, corner) in ring.iter().enumerate() {
            let next = ring[(at + 1) % ring.len()];
            sides.insert(((*corner).min(next), (*corner).max(next)));
            if !covered(*corner, next, &across) {
                left_out.extend([*corner, next]);
            }
        }
    }
    if left_out.is_empty() {
        return;
    }
    left_out.sort_unstable();
    left_out.dedup();
    let spans: Vec<(u32, u32)> = across
        .keys()
        .copied()
        .filter(|(a, b)| {
            !across.contains_key(&(*b, *a)) && !sides.contains(&((*a).min(*b), (*a).max(*b)))
        })
        .collect();
    for (a, b) in spans {
        let mut between: Vec<u32> = left_out
            .iter()
            .copied()
            .filter(|corner| *corner != a && *corner != b && on_line(a, b, &[*corner], flat))
            .collect();
        let (from, span) = (flat[a as usize], flat[b as usize] - flat[a as usize]);
        between.sort_by(|p, q| {
            let along = |corner: &u32| (flat[*corner as usize] - from).dot(span);
            along(p).total_cmp(&along(q))
        });
        if !between.is_empty() {
            fan(triangles, &mut across, a, b, &between);
        }
    }
}

/// Whether every one of `between` stands on the segment from `a` to `b`.
fn on_line(a: u32, b: u32, between: &[u32], flat: &[Vec2]) -> bool {
    let (a, b) = (flat[a as usize], flat[b as usize]);
    let span = b - a;
    let length = span.length_squared();
    between.iter().all(|corner| {
        let offset = flat[*corner as usize] - a;
        let along = offset.dot(span);
        span.perp_dot(offset).abs() <= ON_LINE * length && along > 0.0 && along < length
    })
}

/// The triangle over the edge from `a` to `b`, either way round, split into a fan through
/// `between`, the corners on that edge in order from `a`.
fn fan(
    triangles: &mut Vec<[u32; 3]>,
    across: &mut FastMap<(u32, u32), usize>,
    a: u32,
    b: u32,
    between: &[u32],
) {
    let mut line: Vec<u32> = std::iter::once(a)
        .chain(between.iter().copied())
        .chain(std::iter::once(b))
        .collect();
    let Some(index) = across.get(&(a, b)).copied().or_else(|| {
        line.reverse();
        across.get(&(b, a)).copied()
    }) else {
        return;
    };
    let triangle = triangles[index];
    let Some(corner) = (0..3).find(|at| triangle[*at] == line[0]) else {
        return;
    };
    let apex = triangle[(corner + 2) % 3];
    for (piece, pair) in line.windows(2).enumerate() {
        let made = [pair[0], pair[1], apex];
        let slot = if piece == 0 {
            triangles[index] = made;
            index
        } else {
            triangles.push(made);
            triangles.len() - 1
        };
        for at in 0..3 {
            across.insert((made[at], made[(at + 1) % 3]), slot);
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

    /// Every edge of the rings is covered once, which is what the wall on the other side of
    /// it needs, and nothing the fill added is left on its own.
    fn covers(rings: &[&[u32]], triangles: &[[u32; 3]]) {
        let uses = edge_uses(triangles);
        let mut sides: FastSet<(u32, u32)> = FastSet::default();
        for ring in rings {
            for corner in 0..ring.len() {
                let (a, b) = (ring[corner], ring[(corner + 1) % ring.len()]);
                if a == b {
                    continue;
                }
                let key = (a.min(b), a.max(b));
                assert_eq!(uses.get(&key), Some(&1), "the ring edge {key:?} is covered");
                sides.insert(key);
            }
        }
        for (key, count) in uses {
            if !sides.contains(&key) {
                assert_eq!(count, 2, "the edge {key:?} the fill added is used twice");
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
        covers(&[&[0, 1, 2, 3]], &triangles);
    }

    #[test]
    fn a_hole_whose_side_runs_on_from_a_side_of_the_ring_is_still_covered() {
        // The hole's bottom lies on the line the ring's notch runs along, the line ear
        // clipping bridges the hole along, and the corners on it collapsed.
        let flat = [
            Vec2::new(0.0, -10.0),
            Vec2::new(1.0, -10.0),
            Vec2::new(2.0, -10.0),
            Vec2::new(3.0, -10.0),
            Vec2::new(5.0, -10.0),
            Vec2::new(5.0, -15.0),
            Vec2::new(30.0, -15.0),
            Vec2::new(30.0, 15.0),
            Vec2::new(0.0, 15.0),
            Vec2::new(20.0, -10.0),
            Vec2::new(20.0, 5.0),
            Vec2::new(25.0, 5.0),
            Vec2::new(25.0, -10.0),
            Vec2::new(22.5, -10.0),
        ];
        let ring = [0, 1, 2, 3, 4, 5, 6, 7, 8];
        let hole = vec![9, 10, 11, 12, 13];

        let triangles = triangulate(&ring, &[&hole], &flat);

        covers(&[&ring, &hole], &triangles);
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

        covers(&[&ring], &triangles);
    }
}
