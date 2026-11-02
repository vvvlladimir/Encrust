//! The seam two bodies cut, split into the places it runs through apart from each other.

use core_geometry::{
    Aabb, BoundingVolume, Components, FastMap, Scalar, Vec3, cell_of_point, cells_around,
};

/// How near two pieces of seam stand to be one place, millimetres: a contact a finger's
/// width across is one place, and supports a few millimetres apart are as many.
const ONE_PLACE_MM: Scalar = 1.0;

/// The box round each run of `seam` whose pieces stand within [`ONE_PLACE_MM`] of the next.
pub(super) fn places(seam: &[[Vec3; 2]]) -> Vec<Aabb> {
    // Cells are joined, not pieces: a dense seam holds hundreds of pieces a cell, which
    // joined pairwise cost their square.
    let mut cells: FastMap<[i32; 3], u32> = FastMap::default();
    let ends_in: Vec<[u32; 2]> = seam
        .iter()
        .map(|ends| {
            ends.map(|end| {
                let next = cells.len() as u32;
                *cells
                    .entry(cell_of_point(end, ONE_PLACE_MM))
                    .or_insert(next)
            })
        })
        .collect();
    let mut runs = Components::new(cells.len());
    for &[from, to] in &ends_in {
        runs.union(from, to);
    }
    for (&cell, &index) in &cells {
        for &near in cells_around(cell).filter_map(|around| cells.get(&around)) {
            runs.union(index, near);
        }
    }
    let mut boxes: FastMap<u32, Aabb> = FastMap::default();
    for ([from, to], [cell, _]) in seam.iter().zip(&ends_in) {
        let piece = Aabb::new(from.min(*to), from.max(*to));
        boxes
            .entry(runs.find(*cell))
            .and_modify(|run| run.merge(&piece))
            .or_insert(piece);
    }
    let mut places: Vec<Aabb> = boxes.into_values().collect();
    places.sort_unstable_by(|a, b| {
        a.mins
            .to_array()
            .partial_cmp(&b.mins.to_array())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    places
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seam_in_two_runs_far_apart_is_two_places_each_round_its_run() {
        let seam = [
            [Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0)],
            [Vec3::new(0.5, 0.0, 0.0), Vec3::new(0.5, 0.5, 0.0)],
            [Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.5)],
        ];
        let places = places(&seam);
        assert_eq!(places.len(), 2, "10 mm apart is two places: {places:?}");
        assert_eq!(
            (places[0].mins, places[0].maxs),
            (Vec3::ZERO, Vec3::new(0.5, 0.5, 0.0))
        );
        assert_eq!(
            (places[1].mins, places[1].maxs),
            (Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.5))
        );
    }

    #[test]
    fn pieces_in_cells_side_by_side_are_one_place_and_a_cell_apart_are_two() {
        let piece = |x: f32| [Vec3::new(x, 0.5, 0.5), Vec3::new(x + 0.1, 0.5, 0.5)];
        let places = places(&[piece(0.1), piece(1.5), piece(3.5)]);
        assert_eq!(
            places.len(),
            2,
            "cells 0 and 1 touch, cell 3 is past the empty cell 2: {places:?}"
        );
        assert_eq!(
            (places[0].mins.x, places[0].maxs.x),
            (0.1, 1.6),
            "the first place spans x = 0.1..1.6 mm"
        );
    }
}
