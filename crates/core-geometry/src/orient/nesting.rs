use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{Aabb, Bvh, FastMap, Mesh, Ray, Scalar, Vec3, ray_triangle};

/// For each shell, the smallest closed shell wholly around it, or `None`; an open shell
/// neither holds nor is held. `volume` is six times each shell's signed volume.
pub(super) fn holders(
    mesh: &Mesh,
    shell: &[u32],
    closed: &[bool],
    volume: &[Scalar],
) -> Vec<Option<u32>> {
    let shells = closed.len();
    let bounds = shell_bounds(mesh, shell, closed);
    let (held, holding) = boxed(&bounds);
    if held.iter().all(|held| !held) {
        return vec![None; shells];
    }
    // Only the faces of shells boxing another can be crossed on the way out of one.
    let mut walls = Mesh::new(mesh.vertices.clone(), Vec::new());
    let mut wall_shell = Vec::new();
    for (face, &label) in shell.iter().enumerate() {
        if holding[label as usize] {
            walls.faces.push(mesh.faces[face]);
            wall_shell.push(label);
        }
    }
    let bvh = Bvh::build(&walls);
    let top = bounds
        .iter()
        .flatten()
        .fold(Scalar::MIN, |top, (_, aabb, _)| {
            top.max(aabb.maxs.max_element())
        });
    (0..shells)
        .into_par_iter()
        .map(|s| {
            let (_, _, from) = bounds[s].filter(|_| held[s])?;
            let size = volume[s].abs();
            around(&walls, &bvh, &wall_shell, s as u32, from, top + 1.0)
                .into_iter()
                .filter(|&holder| volume[holder as usize].abs() > size)
                .min_by(|a, b| {
                    volume[*a as usize]
                        .abs()
                        .total_cmp(&volume[*b as usize].abs())
                })
        })
        .collect()
}

/// Each closed shell's label, bounds and a point inside its first face with an area, off
/// the centroid: from there axis rays run along a symmetric body's diagonals.
fn shell_bounds(mesh: &Mesh, shell: &[u32], closed: &[bool]) -> Vec<Option<(u32, Aabb, Vec3)>> {
    let mut boxes: Vec<Option<Aabb>> = vec![None; closed.len()];
    let mut seeds: Vec<Option<Vec3>> = vec![None; closed.len()];
    for (face, &label) in shell.iter().enumerate() {
        let label = label as usize;
        let Some(triangle) = mesh.triangle(face).filter(|_| closed[label]) else {
            continue;
        };
        let mins = triangle.a.min(triangle.b).min(triangle.c);
        let maxs = triangle.a.max(triangle.b).max(triangle.c);
        let grown = boxes[label].map_or(Aabb::new(mins, maxs), |aabb| {
            Aabb::new(aabb.mins.min(mins), aabb.maxs.max(maxs))
        });
        boxes[label] = Some(grown);
        if seeds[label].is_none() && triangle.area() > 0.0 {
            seeds[label] = Some(triangle.a * 0.5 + triangle.b * 0.3 + triangle.c * 0.2);
        }
    }
    (0..closed.len())
        .map(|label| Some((label as u32, boxes[label]?, seeds[label]?)))
        .collect()
}

/// Which shells lie inside another's bounds, and which have another inside theirs: a sweep
/// along x keeping the shells whose span still reaches the one at hand.
fn boxed(bounds: &[Option<(u32, Aabb, Vec3)>]) -> (Vec<bool>, Vec<bool>) {
    let mut held = vec![false; bounds.len()];
    let mut holding = vec![false; bounds.len()];
    let mut order: Vec<(u32, Aabb)> = bounds
        .iter()
        .flatten()
        .map(|(label, aabb, _)| (*label, *aabb))
        .collect();
    order.sort_by(|a, b| a.1.mins.x.total_cmp(&b.1.mins.x));
    let inside = |inner: &Aabb, outer: &Aabb| {
        outer.mins.cmple(inner.mins).all() && outer.maxs.cmpge(inner.maxs).all()
    };
    let mut reaching: Vec<(u32, Aabb)> = Vec::new();
    for (label, aabb) in order {
        reaching.retain(|(_, other)| other.maxs.x >= aabb.mins.x);
        for (other, around) in &reaching {
            if inside(&aabb, around) {
                held[label as usize] = true;
                holding[*other as usize] = true;
            }
            // Equal on the lowest x, the one around may come later in the sweep.
            if inside(around, &aabb) {
                held[*other as usize] = true;
                holding[label as usize] = true;
            }
        }
        reaching.push((label, aabb));
    }
    (held, holding)
}

/// Shells other than `own` that `from` lies inside, by the parity of crossings along the
/// three axes up to `top`; two of three decide, so a ray grazing an edge cannot.
fn around(
    walls: &Mesh,
    bvh: &Bvh,
    wall_shell: &[u32],
    own: u32,
    from: Vec3,
    top: Scalar,
) -> Vec<u32> {
    let mut odd: FastMap<u32, u8> = FastMap::default();
    let mut found = Vec::new();
    for axis in 0..3 {
        let mut crossings: FastMap<u32, u32> = FastMap::default();
        let mut to = from;
        to[axis] = top;
        bvh.faces_meeting(&Aabb::new(from, to), &mut found);
        let ray = Ray::new(from, Vec3::AXES[axis]);
        for &face in &found {
            let label = wall_shell[face as usize];
            let crosses = label != own
                && walls
                    .triangle(face as usize)
                    .and_then(|triangle| ray_triangle(&ray, &triangle))
                    .is_some();
            if crosses {
                *crossings.entry(label).or_default() += 1;
            }
        }
        for (label, count) in crossings {
            if count % 2 == 1 {
                *odd.entry(label).or_default() += 1;
            }
        }
    }
    odd.into_iter()
        .filter(|(_, axes)| *axes >= 2)
        .map(|(label, _)| label)
        .collect()
}
