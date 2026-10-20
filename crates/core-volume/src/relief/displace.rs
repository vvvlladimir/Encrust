use core_geometry::{Heightmap, Mesh, Region, Scalar, UvMap, Vec2, Vec3};
use rayon::prelude::*;

use super::refine::{Refined, refine};
use super::{BLEND_POWER, Relief, ReliefSettings, Tiling, prepared, pressable};
use crate::error::VolumeError;

/// What one face of a split mesh costs while it is split and moved, bytes: its corners,
/// its origin and its paint, half a vertex, and the vertex sums.
const BYTES_PER_FACE: usize = 48;

/// Presses a texture into the faces of `mesh` that `painted` names, read through its own
/// map, by splitting them and moving their corners; every other face stays as it was. See
/// `docs/design/volume.md`.
pub fn press_painted(
    mesh: &Mesh,
    uvs: &UvMap,
    heights: &[Heightmap],
    settings: &ReliefSettings,
    painted: &Region,
) -> Result<Relief, VolumeError> {
    pressable(mesh, settings)?;
    super::mapped(mesh, uvs, heights)?;
    let heights: Vec<_> = heights
        .iter()
        .map(|image| prepared(image, settings))
        .collect();
    let sample = |face: usize, point: Vec3, _normal: Vec3| {
        let (texture, uv) = uvs.at(mesh, face, point)?;
        Some(heights.get(texture)?.sample(uv))
    };
    Ok(moved(mesh, settings, painted, &sample))
}

/// [`press_painted`] with `image` laid over the painted faces as tiles, as [`super::press_tiled`]
/// lays it, each corner reading the three planes by the way it faces.
pub fn press_painted_tiled(
    mesh: &Mesh,
    image: &Heightmap,
    tiling: Tiling,
    settings: &ReliefSettings,
    painted: &Region,
) -> Result<Relief, VolumeError> {
    pressable(mesh, settings)?;
    tiling.check()?;
    let image = prepared(image, settings);
    let (sin, cos) = tiling.angle_deg.to_radians().sin_cos();
    let on_plane = |a: Scalar, b: Scalar| {
        let turned = Vec2::new(a * cos - b * sin, a * sin + b * cos);
        image.sample(turned / tiling.tile_mm)
    };
    let sample = |_face: usize, point: Vec3, normal: Vec3| {
        let weights = normal.abs().powf(BLEND_POWER as Scalar);
        let total = weights.x + weights.y + weights.z;
        let height = weights.x * on_plane(point.y, point.z)
            + weights.y * on_plane(point.x, point.z)
            + weights.z * on_plane(point.x, point.y);
        (total > 0.0).then(|| height / total)
    };
    Ok(moved(mesh, settings, painted, &sample))
}

/// `mesh` split finer over the faces `painted` names and the mask does not hold, with
/// every corner inside them moved along its normal by what `sample` reads there.
fn moved(
    mesh: &Mesh,
    settings: &ReliefSettings,
    painted: &Region,
    sample: &(dyn Fn(usize, Vec3, Vec3) -> Option<Scalar> + Sync),
) -> Relief {
    let pressed = |face: usize| painted.contains(face) && !settings.mask.holds_face(mesh, face);
    let area: Scalar = (0..mesh.faces.len())
        .into_par_iter()
        .filter(|face| pressed(*face))
        .filter_map(|face| mesh.triangle(face))
        .map(|triangle| triangle.area())
        .sum();
    let (edge_mm, coarsened) = edge_mm(settings, area);
    let mut refined = refine(mesh, &pressed, edge_mm);
    let (normals, heights) = corners(&refined, sample);
    let fixed = edges_of_the_paint(&refined);
    for (vertex, position) in refined.mesh.vertices.iter_mut().enumerate() {
        let (Some(height), false) = (heights[vertex], fixed[vertex]) else {
            continue;
        };
        let lift_mm = settings.amplitude_mm * settings.lifted(height);
        *position += normals[vertex].normalize_or_zero() * lift_mm;
    }
    Relief {
        mesh: refined.mesh,
        voxel_mm: edge_mm,
        coarsened,
    }
}

/// The longest edge a painted face is split down to, mm: the lattice a field relief of
/// the painted area would be cut on, coarsened where the faces would outgrow the budget.
fn edge_mm(settings: &ReliefSettings, area_mm2: Scalar) -> (Scalar, bool) {
    let asked_mm = settings.voxel_mm(area_mm2);
    // Splitting at the middles leaves faces of half the longest edge's square, about.
    let faces = |edge_mm: Scalar| 2.0 * area_mm2 / (edge_mm * edge_mm);
    let affordable = (settings.budget_bytes / BYTES_PER_FACE) as Scalar;
    if faces(asked_mm) <= affordable {
        return (asked_mm, false);
    }
    ((2.0 * area_mm2 / affordable).sqrt(), true)
}

/// Each vertex's normal over the painted faces it is a corner of, weighted by their area,
/// and the mean of the heights those faces read at it, where any reads one.
fn corners(
    refined: &Refined,
    sample: &(dyn Fn(usize, Vec3, Vec3) -> Option<Scalar> + Sync),
) -> (Vec<Vec3>, Vec<Option<Scalar>>) {
    let mesh = &refined.mesh;
    let mut normals = vec![Vec3::ZERO; mesh.vertices.len()];
    for (face, corners) in mesh.faces.iter().enumerate() {
        if !refined.painted[face] {
            continue;
        }
        let Some(triangle) = mesh.triangle(face) else {
            continue;
        };
        for corner in corners {
            normals[*corner as usize] += triangle.normal_unnormalized();
        }
    }
    let unit = &normals;
    let read: Vec<(u32, Scalar)> = (0..mesh.faces.len())
        .into_par_iter()
        .filter(|face| refined.painted[*face])
        .flat_map_iter(|face| {
            let origin = refined.origin[face] as usize;
            mesh.faces[face].into_iter().filter_map(move |corner| {
                let at = mesh.vertices[corner as usize];
                let normal = unit[corner as usize].normalize_or_zero();
                Some((corner, sample(origin, at, normal)?))
            })
        })
        .collect();
    let mut sums = vec![(0.0, 0u32); mesh.vertices.len()];
    for (corner, height) in read {
        let sum = &mut sums[corner as usize];
        *sum = (sum.0 + height, sum.1 + 1);
    }
    let heights = sums
        .into_iter()
        .map(|(sum, count)| (count > 0).then(|| sum / count as Scalar))
        .collect();
    (normals, heights)
}

/// Which vertices a face left unpainted has as a corner: the edge of the paint, which
/// stays where it is so the faces beyond it do not move.
fn edges_of_the_paint(refined: &Refined) -> Vec<bool> {
    let mut fixed = vec![false; refined.mesh.vertices.len()];
    for (face, corners) in refined.mesh.faces.iter().enumerate() {
        if !refined.painted[face] {
            for corner in corners {
                fixed[*corner as usize] = true;
            }
        }
    }
    fixed
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Mapping, cuboid, diagnose};

    fn flat(height: Scalar) -> Heightmap {
        Heightmap::new(1, 1, vec![height]).expect("one sample over one pixel")
    }

    fn raised() -> ReliefSettings {
        ReliefSettings {
            amplitude_mm: 0.4,
            ..ReliefSettings::default()
        }
    }

    /// A 10 mm cube and its lid, the faces turned up.
    fn cube_and_lid() -> (Mesh, Region) {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let mut lid = Region::default();
        for face in 0..mesh.faces.len() {
            if mesh
                .triangle(face)
                .expect("in range")
                .normal_unnormalized()
                .z
                > 0.0
            {
                lid.set(face, true);
            }
        }
        (mesh, lid)
    }

    #[test]
    fn only_a_painted_lid_rises_and_the_rest_keeps_every_vertex() {
        let (mesh, lid) = cube_and_lid();
        let pressed = press_painted_tiled(&mesh, &flat(1.0), Tiling::default(), &raised(), &lid)
            .expect("a painted cube presses")
            .mesh;
        let top = pressed
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(Scalar::MIN, Scalar::max);
        assert!(
            (top - 10.4).abs() < 1e-4,
            "the lid's inside rises 0.4 mm, got {top}"
        );
        assert!(
            pressed
                .vertices
                .iter()
                .all(|v| v.z >= 0.0 && (0.0..=10.0).contains(&v.x)),
            "the sides and the bottom stay where they were"
        );
        assert!(
            mesh.vertices
                .iter()
                .zip(&pressed.vertices)
                .all(|(was, now)| was == now),
            "the cube's own corners are the edge of the paint and do not move"
        );
        assert!(
            diagnose(&pressed).is_closed(),
            "moving corners leaves the surface closed"
        );
    }

    #[test]
    fn a_painted_face_reads_its_own_map() {
        let (mesh, lid) = cube_and_lid();
        let uvs = UvMap::new(vec![Some(Mapping::sole([Vec2::ZERO; 3])); mesh.faces.len()]);
        let pressed = press_painted(&mesh, &uvs, &[flat(1.0)], &raised(), &lid)
            .expect("a mapped cube presses")
            .mesh;
        let top = pressed
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(Scalar::MIN, Scalar::max);
        assert!((top - 10.4).abs() < 1e-4, "got {top}");
    }

    #[test]
    fn a_painted_press_refuses_what_a_field_press_refuses() {
        let (mesh, lid) = cube_and_lid();
        let short = UvMap::new(vec![Some(Mapping::sole([Vec2::ZERO; 3]))]);
        assert!(matches!(
            press_painted(&mesh, &short, &[flat(1.0)], &raised(), &lid),
            Err(VolumeError::UnmappedMesh { .. })
        ));
        let second = UvMap::new(vec![
            Some(Mapping {
                texture: 1,
                corners: [Vec2::ZERO; 3]
            });
            mesh.faces.len()
        ]);
        assert!(matches!(
            press_painted(&mesh, &second, &[flat(1.0)], &raised(), &lid),
            Err(VolumeError::MissingTexture {
                wanted: 2,
                given: 1
            })
        ));
        let no_tile = Tiling {
            tile_mm: 0.0,
            angle_deg: 0.0,
        };
        assert!(matches!(
            press_painted_tiled(&mesh, &flat(1.0), no_tile, &raised(), &lid),
            Err(VolumeError::BadTile(_))
        ));
        let nothing = ReliefSettings {
            amplitude_mm: 0.0,
            ..ReliefSettings::default()
        };
        assert!(matches!(
            press_painted_tiled(&mesh, &flat(1.0), Tiling::default(), &nothing, &lid),
            Err(VolumeError::BadAmplitude(_))
        ));
    }
}
