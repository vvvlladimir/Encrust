//! The seam two bodies cut, split into the places it runs through apart from each other.

use core_geometry::{
    Aabb, BoundingVolume, Components, FastMap, Scalar, Vec3, cell_of_point, cells_around,
};

/// How near two pieces of seam stand to be one place, millimetres: a contact a finger's
/// width across is one place, and supports a few millimetres apart are as many.
const ONE_PLACE_MM: Scalar = 1.0;

/// The box round each run of `seam` whose pieces stand within [`ONE_PLACE_MM`] of the next.
pub(super) fn places(seam: &[[Vec3; 2]]) -> Vec<Aabb> {
    let mut by_cell: FastMap<[i32; 3], Vec<u32>> = FastMap::default();
    for (index, ends) in seam.iter().enumerate() {
        for end in ends {
            by_cell
                .entry(cell_of_point(*end, ONE_PLACE_MM))
                .or_default()
                .push(index as u32);
        }
    }
    let mut runs = Components::new(seam.len());
    for (index, ends) in seam.iter().enumerate() {
        let near = ends
            .iter()
            .flat_map(|end| cells_around(cell_of_point(*end, ONE_PLACE_MM)))
            .filter_map(|cell| by_cell.get(&cell))
            .flatten();
        for &other in near {
            runs.union(index as u32, other);
        }
    }
    let mut boxes: FastMap<u32, Aabb> = FastMap::default();
    for (index, [from, to]) in seam.iter().enumerate() {
        let piece = Aabb::new(from.min(*to), from.max(*to));
        boxes
            .entry(runs.find(index as u32))
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
}
