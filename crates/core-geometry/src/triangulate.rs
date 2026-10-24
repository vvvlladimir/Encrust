//! Filling a flat ring with triangles, which both a cut's cap and a patch over a hole
//! need; see `docs/decisions/0089`.

use crate::{FastMap, FastSet, Scalar, Vec2, ring_area};

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
    wind_as(&mut triangles, &kept[0], flat);
    triangles
}

/// Turns `triangles` to wind as `outer` does, which ear clipping does not promise.
fn wind_as(triangles: &mut [[u32; 3]], outer: &[u32], flat: &[Vec2]) {
    let corner = |index: u32| flat[index as usize];
    let filled: Scalar = triangles
        .iter()
        .map(|[a, b, c]| (corner(*b) - corner(*a)).perp_dot(corner(*c) - corner(*a)))
        .sum();
    if filled * ring_area(outer.iter().map(|index| corner(*index))) < 0.0 {
        for triangle in triangles {
            triangle.swap(1, 2);
        }
    }
}

/// The triangle a corner taken out for standing on the one before it needs, and the edge
/// the fill runs in its place, from the kept corner before its run to the one after.
struct Patch {
    span: (u32, u32),
    triangle: [u32; 3],
}

/// Adds the triangle each repeated corner needs, wound against the edge the fill left
/// where its run was taken out.
fn cover_repeats(triangles: &mut Vec<[u32; 3]>, patches: Vec<Patch>) {
    let across = facing_edges(triangles);
    for Patch { span, triangle } in patches {
        if across.contains_key(&(span.1, span.0)) {
            triangles.push([triangle[2], triangle[1], triangle[0]]);
        } else {
            triangles.push(triangle);
        }
    }
}

/// How far off the line between its neighbours a dropped corner may stand and still be put
/// back on it, as a share of that line's length squared: rounding, not a bend.
const ON_LINE: f32 = 1e-6;

/// Puts back ring corners that ear clipping skipped for lying on a line between two others.
/// Without them the fill and the wall meet along nothing there and the surface has a slit.
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

/// The ring with corners standing on the one before them taken out, each left in `patches` as a
/// triangle. Ear clipping drops them and leaves a hole where the model touched itself on the plane.
fn without_repeats(ring: &[u32], flat: &[Vec2], patches: &mut Vec<Patch>) -> Vec<u32> {
    let count = ring.len();
    let before = |at: usize| ring[(at + count - 1) % count];
    // The ring closes, so its first corner can stand on its last as well.
    let repeated = |at: usize| flat[ring[at] as usize] == flat[before(at) as usize];
    let kept: Vec<u32> = (0..count)
        .filter(|at| !repeated(*at))
        .map(|at| ring[at])
        .collect();
    if kept.len() < 3 {
        return kept;
    }
    // A run's corners fan from the kept corner after it, each over the edge into it, so
    // the edges along the run are covered one after another.
    for at in (0..count).filter(|at| repeated(*at)) {
        let mut after = (at + 1) % count;
        while repeated(after) {
            after = (after + 1) % count;
        }
        let mut back = (at + count - 1) % count;
        while repeated(back) {
            back = (back + count - 1) % count;
        }
        patches.push(Patch {
            span: (ring[back], ring[after]),
            triangle: [before(at), ring[at], ring[after]],
        });
    }
    kept
}

/// How many times the flipping sweep goes round. A sweep takes every flat triangle whose
/// neighbour it has not just moved, so the few left for the next one are soon gone.
const PASSES: usize = 8;

/// Flips zero-area triangles out of a filled ring across their longest edge. Ear clipping covers
/// collinear corners with a zero-area triangle that breaks when a plane cuts it.
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
mod tests;
