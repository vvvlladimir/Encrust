use rayon::prelude::*;

/// Stands in for a distance no cell reaches.
const FAR: f32 = 1e20;

/// Squared distance, in cells, from every cell of a `width` by `height` grid to the
/// nearest cell `is_target` marks, or `FAR`-sized where it marks none.
///
/// Exact Euclidean, one column pass and one row pass of the lower envelope of parabolas;
/// see `docs/design/joints.md`.
pub(crate) fn squared(width: usize, height: usize, is_target: &[bool]) -> Vec<f32> {
    let columns: Vec<Vec<f32>> = (0..width)
        .into_par_iter()
        .map(|x| {
            let column: Vec<f32> = (0..height)
                .map(|y| if is_target[y * width + x] { 0.0 } else { FAR })
                .collect();
            envelope(&column)
        })
        .collect();
    let mut grid = vec![0.0; width * height];
    for (x, column) in columns.iter().enumerate() {
        for (y, value) in column.iter().enumerate() {
            grid[y * width + x] = *value;
        }
    }
    grid.par_chunks_mut(width).for_each(|row| {
        let done = envelope(row);
        row.copy_from_slice(&done);
    });
    grid
}

/// One line's squared distances to its sampled `cost`, the lower envelope of the
/// parabolas rooted at each cell.
fn envelope(cost: &[f32]) -> Vec<f32> {
    let n = cost.len();
    let mut out = vec![FAR; n];
    let mut roots = vec![0usize; n];
    let mut bounds = vec![0.0f32; n + 1];
    let mut top = 0usize;
    bounds[0] = -FAR;
    bounds[1] = FAR;
    let meet = |a: usize, b: usize| {
        let (fa, fb) = (f64::from(cost[a]), f64::from(cost[b]));
        let (a, b) = (a as f64, b as f64);
        (((fb + b * b) - (fa + a * a)) / (2.0 * (b - a))) as f32
    };
    let first = cost.iter().position(|value| *value < FAR);
    let Some(first) = first else {
        return out;
    };
    roots[0] = first;
    for (q, value) in cost.iter().enumerate().skip(first + 1) {
        if *value >= FAR {
            continue;
        }
        let mut s = meet(roots[top], q);
        while s <= bounds[top] {
            top -= 1;
            s = meet(roots[top], q);
        }
        top += 1;
        roots[top] = q;
        bounds[top] = s;
        bounds[top + 1] = FAR;
    }
    let mut k = 0;
    for (q, value) in out.iter_mut().enumerate() {
        while bounds[k + 1] < q as f32 {
            k += 1;
        }
        let gap = q as f32 - roots[k] as f32;
        *value = gap * gap + cost[roots[k]];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_target_is_as_far_from_each_cell_as_pythagoras_says() {
        let (width, height) = (7, 5);
        let mut target = vec![false; width * height];
        target[2 * width + 3] = true;
        let squared = squared(width, height, &target);
        for y in 0..height {
            for x in 0..width {
                let (dx, dy) = (x as f32 - 3.0, y as f32 - 2.0);
                assert_eq!(
                    squared[y * width + x],
                    dx * dx + dy * dy,
                    "cell ({x}, {y}) from the target at (3, 2)"
                );
            }
        }
    }

    #[test]
    fn the_nearest_of_two_targets_wins() {
        let target = [true, false, false, false, false, false, true];
        let squared = squared(7, 1, &target);
        assert_eq!(squared, vec![0.0, 1.0, 4.0, 9.0, 4.0, 1.0, 0.0]);
    }
}
