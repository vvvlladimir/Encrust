use rayon::prelude::*;

use super::cells::Grid;
use crate::{FastMap, Scalar, Vec3, cells_around};

/// A difference of mean curvature under three over the model's diagonal is no detail: a ball
/// a whole model across bends nearly that much. Below it a map is not stretched to fill 0 to 1.
const FAINT_PER_DIAGONAL: Scalar = 3.0;
/// The share of cells with the most change that reads as full detail: one sharp tip reads far
/// past the rest, and scaling by the largest flattened a figure's every other detail.
const FULL_DETAIL_SHARE: Scalar = 0.05;

/// How detailed the surface is around each finest cell, 0 to 1: the mean curvature smoothed
/// at each level, minus the same one level coarser, after Lee, Varshney and Jacobs.
pub(super) fn saliency(levels: &[Grid], diagonal_mm: Scalar) -> FastMap<[i32; 3], Scalar> {
    let finest = &levels[0];
    let keys: Vec<[i32; 3]> = finest.cells.keys().copied().collect();
    let smoothed: Vec<Vec<Scalar>> = keys
        .par_iter()
        .map(|key| {
            let at = finest.cells[key].centroid();
            levels.iter().map(|level| smoothed_at(level, at)).collect()
        })
        .collect();
    let faint = FAINT_PER_DIAGONAL / diagonal_mm;
    let mut summed = vec![0.0; keys.len()];
    for level in 0..levels.len() - 1 {
        let difference: Vec<Scalar> = smoothed
            .iter()
            .map(|at| (at[level] - at[level + 1]).abs())
            .collect();
        let normalised = normalised(&difference, faint);
        for (sum, value) in summed.iter_mut().zip(normalised) {
            *sum += value / (levels.len() - 1) as Scalar;
        }
    }
    keys.into_iter().zip(summed).collect()
}

/// The mean curvature around `at` on `level`: its cell and the 26 around it, each weighted
/// by its area and by how near its surface is, out to two cells.
fn smoothed_at(level: &Grid, at: Vec3) -> Scalar {
    let reach_mm = 2.0 * level.cell_mm;
    let (curvature_mm, area_mm2) = cells_around(level.key_of(at))
        .filter_map(|key| level.cells.get(&key))
        .fold((0.0, 0.0), |(curvature, area), cell| {
            let near = (1.0 - cell.centroid().distance(at) / reach_mm).max(0.0);
            let weight = near * near;
            (
                curvature + weight * cell.curvature_mm,
                area + weight * cell.area_mm2,
            )
        });
    curvature_mm / area_mm2.max(Scalar::EPSILON)
}

/// One level's differences over what its top hundredth reach, or over `faint` where that is
/// larger, so that a smooth model's noise is not stretched to full detail.
fn normalised(difference: &[Scalar], faint: Scalar) -> Vec<Scalar> {
    let full = top_share(difference).max(faint);
    difference
        .iter()
        .map(|value| (value / full).min(1.0))
        .collect()
}

/// The difference the top [`FULL_DETAIL_SHARE`] of cells reach, found in linear time.
fn top_share(difference: &[Scalar]) -> Scalar {
    if difference.is_empty() {
        return 0.0;
    }
    let mut ranked = difference.to_vec();
    let at = ((ranked.len() - 1) as Scalar * (1.0 - FULL_DETAIL_SHARE)) as usize;
    *ranked.select_nth_unstable_by(at, Scalar::total_cmp).1
}
