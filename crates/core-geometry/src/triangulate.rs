//! Filling a flat ring with triangles, which both a cut's cap and a patch over a hole
//! need; see `docs/decisions/0089`.

use crate::Vec2;

/// Fills one ring and the rings inside it, by ear clipping. The corners are indices into
/// `flat`, and the triangles come back wound as the outer ring is.
pub(crate) fn triangulate(outer: &[u32], holes: &[&Vec<u32>], flat: &[Vec2]) -> Vec<[u32; 3]> {
    let mut ring: Vec<u32> = outer.to_vec();
    let mut hole_starts = Vec::with_capacity(holes.len());
    for hole in holes {
        hole_starts.push(ring.len());
        ring.extend(hole.iter().copied());
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
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|corner| [ring[corner[0]], ring[corner[1]], ring[corner[2]]])
        .collect()
}
