use core_geometry::{Mesh, Vec3};
use rayon::prelude::*;

use super::Lattice;

/// Which lattice points of `lattice` lie inside `mesh`, as a dense array in its order.
///
/// Each face is projected down onto the columns of lattice points it covers, and a column
/// is inside wherever the faces below a point wind round it; see `docs/design/parts.md`.
pub(super) fn inside(mesh: &Mesh, lattice: &Lattice) -> Vec<bool> {
    let mut crossings: Vec<Crossing> = (0..mesh.faces.len())
        .into_par_iter()
        .fold(Vec::new, |mut found, face| {
            crossings_of(mesh, face, lattice, &mut found);
            found
        })
        .flatten()
        .collect();
    crossings.par_sort_unstable_by(|a, b| a.column.cmp(&b.column).then(a.z_mm.total_cmp(&b.z_mm)));
    let runs: Vec<(usize, usize, usize)> = crossings
        .par_chunk_by(|a, b| a.column == b.column)
        .flat_map_iter(|column| runs_of(column, lattice))
        .collect();
    let mut inside = vec![false; lattice.len()];
    let (width, depth) = (lattice.size.x as usize, lattice.size.y as usize);
    for (column, first, last) in runs {
        for z in first..last {
            inside[z * width * depth + column] = true;
        }
    }
    inside
}

/// Where one face crosses the column of lattice points at `column`, and which way it
/// faces: `rising` where its normal points up.
struct Crossing {
    column: usize,
    z_mm: f64,
    rising: bool,
}

/// The runs of lattice levels, `first..last`, that lie inside along one column's
/// crossings, which are sorted upward. Going up, a face turned down is a way in.
fn runs_of(column: &[Crossing], lattice: &Lattice) -> Vec<(usize, usize, usize)> {
    let mut runs = Vec::new();
    let mut winding = 0i32;
    let mut entered: Option<usize> = None;
    for crossing in column {
        winding += if crossing.rising { -1 } else { 1 };
        let level = first_level_above(crossing.z_mm, lattice);
        match (winding > 0, entered) {
            (true, None) => entered = Some(level),
            (false, Some(first)) => {
                if level > first {
                    runs.push((column[0].column, first, level));
                }
                entered = None;
            }
            _ => {}
        }
    }
    runs
}

/// The first lattice level strictly above `z_mm`, clamped to the lattice.
fn first_level_above(z_mm: f64, lattice: &Lattice) -> usize {
    let mut level = (z_mm / f64::from(lattice.grid.voxel_mm)).floor() as i32;
    // The division may round either way; the comparison is with the points themselves.
    while along(lattice, level) > z_mm {
        level -= 1;
    }
    while along(lattice, level) <= z_mm {
        level += 1;
    }
    (level - lattice.low.z).clamp(0, lattice.size.z) as usize
}

/// Where lattice coordinate `coord` stands on its axis, exactly as `VoxelGrid::position`
/// puts it.
fn along(lattice: &Lattice, coord: i32) -> f64 {
    f64::from(coord as f32 * lattice.grid.voxel_mm)
}

/// Pushes where face `face` crosses the lattice's columns.
///
/// A column through an edge two faces share is given to exactly one of them, by which way
/// the edge runs, so a closed mesh is never counted twice or missed along a seam. The
/// arithmetic is in `f64`, where differences and products of `f32` coordinates are exact.
fn crossings_of(mesh: &Mesh, face: usize, lattice: &Lattice, found: &mut Vec<Crossing>) {
    let Some(corners) = mesh.faces.get(face) else {
        return;
    };
    let [mut a, mut b, c] = corners.map(|corner| as_f64(mesh.vertices[corner as usize]));
    let mut area = cross(b, c, a);
    if area == 0.0 {
        return;
    }
    let rising = area > 0.0;
    if !rising {
        std::mem::swap(&mut a, &mut b);
        area = -area;
    }
    let step = f64::from(lattice.grid.voxel_mm);
    // One column wider each side: the division may round a point standing on a corner out
    // of range, and the weights below decide each point exactly.
    let columns = |low: f64, high: f64, first: i32, count: i32| {
        let from = ((low / step).ceil() as i64 - 1 - i64::from(first)).max(0);
        let to = ((high / step).floor() as i64 + 1 - i64::from(first)).min(i64::from(count) - 1);
        from..=to
    };
    let xs = columns(
        a[0].min(b[0]).min(c[0]),
        a[0].max(b[0]).max(c[0]),
        lattice.low.x,
        lattice.size.x,
    );
    let ys = columns(
        a[1].min(b[1]).min(c[1]),
        a[1].max(b[1]).max(c[1]),
        lattice.low.y,
        lattice.size.y,
    );
    for y in ys {
        let at_y = along(lattice, lattice.low.y + y as i32);
        for x in xs.clone() {
            let point = [along(lattice, lattice.low.x + x as i32), at_y, 0.0];
            let weights = [cross(b, c, point), cross(c, a, point), cross(a, b, point)];
            let owned = [(b, c), (c, a), (a, b)]
                .iter()
                .zip(weights)
                .all(|((from, to), weight)| weight > 0.0 || (weight == 0.0 && owns(*from, *to)));
            if owned {
                let z_mm = (weights[0] * a[2] + weights[1] * b[2] + weights[2] * c[2]) / area;
                let column = y as usize * lattice.size.x as usize + x as usize;
                found.push(Crossing {
                    column,
                    z_mm,
                    rising,
                });
            }
        }
    }
}

/// Whether a point on the edge from `from` to `to` of an anticlockwise triangle belongs to
/// it. The neighbour runs the edge the other way, so it gets the point exactly when this
/// one does not.
fn owns(from: [f64; 3], to: [f64; 3]) -> bool {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    dy > 0.0 || (dy == 0.0 && dx < 0.0)
}

/// Twice the signed area of `from`, `to`, `point` seen from above.
fn cross(from: [f64; 3], to: [f64; 3], point: [f64; 3]) -> f64 {
    (to[0] - from[0]) * (point[1] - from[1]) - (to[1] - from[1]) * (point[0] - from[0])
}

fn as_f64(vertex: Vec3) -> [f64; 3] {
    [
        f64::from(vertex.x),
        f64::from(vertex.y),
        f64::from(vertex.z),
    ]
}

#[cfg(test)]
mod tests {
    use core_geometry::{cuboid, glam::IVec3};

    use super::*;
    use crate::grid::VoxelGrid;

    #[test]
    fn a_box_on_the_lattice_holds_the_points_on_one_side_of_each_face() {
        let grid = VoxelGrid::new(1.0);
        let lattice = Lattice {
            grid,
            low: IVec3::splat(-2),
            size: IVec3::splat(9),
        };
        let inside = inside(&cuboid(Vec3::ZERO, Vec3::splat(4.0)), &lattice);
        let count = inside.iter().filter(|inside| **inside).count();
        // A point on a face goes to one side of it only: the box takes those on its faces
        // at 4 mm and leaves those at 0 mm, so 1 to 4 on each axis.
        assert_eq!(count, 4 * 4 * 4, "a 4 mm box on a 1 mm lattice");
    }
}
