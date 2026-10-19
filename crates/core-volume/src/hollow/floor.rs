use std::collections::HashSet;
use std::ops::Range;

use core_geometry::{
    Cut, FastMap, FastSet, Mesh, Plane, Scalar, Vec3, cut, diagnose, glam::IVec3, point_key, split,
};

use super::append;
use crate::boolean::{Boolean, combine_exact, combine_exact_toward};
use crate::csg::assemble;
use crate::grid::tile_of;
use crate::sdf::Sdf;

/// A point this close to the plane the cavity is cut at is on it, millimetres: the cut's
/// own band.
const ON_CUT_MM: Scalar = 1e-4;

/// How far past the cavity's side toward the plate a shell has to reach to go through the
/// engine with it, millimetres: a little over the clearance `combine_exact_toward` cuts the
/// model at above it.
const LOCAL_CLEARANCE_MM: Scalar = 1.0;

/// Planes tried through the cavity before the whole model goes through the engine instead.
const PLANES: usize = 8;

/// How far each plane tried stands from the last, as a share of a voxel: irrational, so no
/// two land on the same place in a lattice cell.
const PLANE_STEP: Scalar = 0.618;

/// Where an open floor's cavity is cut through: the wall and the cavity's band, and the
/// lattice it was cut on, millimetres.
#[derive(Debug, Clone, Copy)]
pub(super) struct Opening {
    pub toward: Vec3,
    pub wall_mm: Scalar,
    pub band_mm: Scalar,
    pub voxel_mm: Scalar,
}

/// `model` with `void` taken out of it exactly, so an opened floor is gone from the mesh and
/// not only outweighed in it, with the range of faces that bound the cavity.
///
/// Above the opening the cavity stands clear inside the model and appending it wound inward
/// is already exact, so only the cavity's side toward the plate is taken out through the
/// engine, with only the model's bottom under it, and the rest welded on along the cut. A
/// cut of the cavity that leaves a half the engine cannot read sends the whole model through
/// instead. `None` where the engine refuses the model. See `docs/design/hollowing.md`.
pub(super) fn cut_through(
    model: &Mesh,
    void: &Mesh,
    opening: Opening,
) -> Option<(Mesh, Range<usize>)> {
    near_the_floor(model, void, opening).or_else(|| whole(model, void))
}

/// [`cut_through`] with the whole model and cavity through the engine.
fn whole(model: &Mesh, void: &Mesh) -> Option<(Mesh, Range<usize>)> {
    let shell = combine_exact(&[model.clone(), void.clone()], Boolean::Difference).ok()?;
    let from_void: FastSet<[u32; 3]> = void.vertices.iter().copied().map(point_key).collect();
    // A face the engine kept or split from the cavity still has one of its corners.
    let (cavity, outside): (Vec<[u32; 3]>, Vec<[u32; 3]>) = shell.faces.iter().partition(|face| {
        face.iter()
            .any(|vertex| from_void.contains(&point_key(shell.vertices[*vertex as usize])))
    });
    let range = outside.len()..outside.len() + cavity.len();
    let mut faces = outside;
    faces.extend(cavity);
    Some((Mesh::new(shell.vertices, faces), range))
}

/// [`cut_through`] with only the cavity's side toward the plate through the engine, or
/// `None` where no plane tried cuts the cavity into halves it can read, or they do not join.
fn near_the_floor(model: &Mesh, void: &Mesh, opening: Opening) -> Option<(Mesh, Range<usize>)> {
    let toward = opening.toward.try_normalize()?;
    let (halves, plane) = readable_cut(model, void, toward, opening)?;
    let low_side: FastSet<[u32; 3]> = halves
        .above
        .vertices
        .iter()
        .copied()
        .map(point_key)
        .collect();
    let cap_at = halves.above.faces.len() - halves.cap_faces;
    let cap: FastSet<[u32; 3]> = halves.above.faces[cap_at..]
        .iter()
        .flatten()
        .map(|vertex| point_key(halves.above.vertices[*vertex as usize]))
        .collect();

    // The engine reads one shell near the plane at a time, so the others stay as they were.
    let (standing, others) = standing(model, plane);
    let near =
        combine_exact_toward(&[standing, halves.above], Boolean::Difference, plane.normal).ok()?;
    let rest = &halves.below;
    let rest_faces = &rest.faces[..rest.faces.len() - halves.cap_faces];
    let (joined, cavity) = joined(&near, &cap, &low_side, rest, rest_faces, plane)?;
    let mut whole = others;
    let offset = whole.faces.len();
    append(&mut whole, &joined);
    Some((whole, cavity.start + offset..cavity.end + offset))
}

/// The cavity cut across clear above the opening into halves the engine reads, and the
/// plane it was cut at: the first of [`PLANES`] tried that leaves the side toward the plate
/// closed. Surface nets pinch a cavity into edges four faces share, and a plane through one
/// leaves that side open.
fn readable_cut(model: &Mesh, void: &Mesh, toward: Vec3, opening: Opening) -> Option<(Cut, Plane)> {
    // Clear above where the opening's outline is read, so the plane is in the cavity proper.
    let first = floor_at(model, toward)? - opening.wall_mm - 2.0 * opening.band_mm;
    (0..PLANES).find_map(|tried| {
        let height = first - tried as Scalar * PLANE_STEP * opening.voxel_mm;
        let plane = Plane::at(toward, height)?;
        let halves = cut(void, plane);
        let report = diagnose(&halves.above);
        let readable = halves.open_loops == 0
            && !halves.below.faces.is_empty()
            && !halves.above.faces.is_empty()
            && report.boundary_edges == 0
            && report.unbalanced_edges == 0;
        readable.then_some((halves, plane))
    })
}

/// The one shell of `model` reaching past `plane` toward the plate, and the others in one
/// mesh; the whole model and nothing where more than one reaches it.
fn standing(model: &Mesh, plane: Plane) -> (Mesh, Mesh) {
    let shells = split(model);
    let reaches = |shell: &Mesh| {
        // As far up as the engine's own cut stands over the cavity's side toward the plate.
        let height = plane.point.dot(plane.normal) - LOCAL_CLEARANCE_MM;
        shell
            .vertices
            .iter()
            .any(|vertex| vertex.dot(plane.normal) > height)
    };
    let (low, high): (Vec<Mesh>, Vec<Mesh>) = shells.into_iter().partition(reaches);
    match <[Mesh; 1]>::try_from(low) {
        Ok([standing]) if !high.is_empty() => {
            let mut others = Mesh::default();
            for shell in &high {
                append(&mut others, shell);
            }
            (standing, others)
        }
        _ => (model.clone(), Mesh::default()),
    }
}

/// `near` without the cap the cavity's side toward the plate left in it on `plane`, and
/// `rest_faces` of the cavity on the other side wound inward, welded along the cut; `None`
/// unless every edge of the cut is walked once each way. `low_side` holds the points of the
/// cavity's side toward the plate, whose faces in `near` are the cavity's too.
fn joined(
    near: &Mesh,
    cap: &FastSet<[u32; 3]>,
    low_side: &FastSet<[u32; 3]>,
    rest: &Mesh,
    rest_faces: &[[u32; 3]],
    plane: Plane,
) -> Option<(Mesh, Range<usize>)> {
    let on_cap = |vertex: u32| {
        let point = near.vertices[vertex as usize];
        (point - plane.point).dot(plane.normal).abs() <= ON_CUT_MM
            && cap.contains(&point_key(point))
    };
    let (ceiling, kept): (Vec<[u32; 3]>, Vec<[u32; 3]>) = near
        .faces
        .iter()
        .partition(|face| face.iter().all(|vertex| on_cap(*vertex)));
    let seam: FastMap<[u32; 3], u32> = ceiling
        .iter()
        .flatten()
        .map(|vertex| (point_key(near.vertices[*vertex as usize]), *vertex))
        .collect();

    let mut vertices = near.vertices.clone();
    let remap: Vec<u32> = rest
        .vertices
        .iter()
        .map(|point| {
            seam.get(&point_key(*point)).copied().unwrap_or_else(|| {
                vertices.push(*point);
                vertices.len() as u32 - 1
            })
        })
        .collect();
    let inward: Vec<[u32; 3]> = rest_faces
        .iter()
        .map(|[a, b, c]| [remap[*a as usize], remap[*c as usize], remap[*b as usize]])
        .collect();
    let seam_vertices: FastSet<u32> = seam.values().copied().collect();
    if !closes(&kept, &inward, &seam_vertices) {
        return None;
    }

    // What the engine left of the cavity's side toward the plate bounds it too.
    let (low, outside): (Vec<[u32; 3]>, Vec<[u32; 3]>) = kept.into_iter().partition(|face| {
        face.iter()
            .any(|vertex| low_side.contains(&point_key(vertices[*vertex as usize])))
    });
    let cavity = outside.len()..outside.len() + low.len() + inward.len();
    let mut faces = outside;
    faces.extend(low);
    faces.extend(inward);
    Some((Mesh::new(vertices, faces), cavity))
}

/// Whether every edge along the seam is walked once by a face of `lower` and once back by
/// one of `upper`.
fn closes(lower: &[[u32; 3]], upper: &[[u32; 3]], seam: &FastSet<u32>) -> bool {
    let along = |faces: &[[u32; 3]]| -> Vec<(u32, u32)> {
        faces
            .iter()
            .flat_map(|[a, b, c]| [(*a, *b), (*b, *c), (*c, *a)])
            .filter(|(a, b)| seam.contains(a) && seam.contains(b))
            .collect()
    };
    let mut down = along(lower);
    let mut up: Vec<(u32, u32)> = along(upper).into_iter().map(|(a, b)| (b, a)).collect();
    down.sort_unstable();
    up.sort_unstable();
    !down.is_empty() && down.windows(2).all(|pair| pair[0] != pair[1]) && down == up
}

/// `cavity` carried on through the floor of `mesh` facing the opening's way, or `None` for no
/// direction: the outline it has a band in from the floor's inner face, swept out past the
/// floor's outer face, so the floor is gone wherever the cavity stood over it. See
/// `docs/design/hollowing.md`.
pub(super) fn opened(cavity: &Sdf, mesh: &Mesh, opening: Opening) -> Option<Sdf> {
    let toward = opening.toward.try_normalize()?;
    let floor = floor_at(mesh, toward)?;

    let grid = cavity.grid();
    let band_mm = cavity.band_mm();
    // Read a band inside the cavity, where its outline is the cavity's own, and swept a
    // band past the floor, where the surface it leaves is clear of the model's.
    let outline_at = floor - opening.wall_mm - band_mm;
    let out_to = floor + band_mm;
    let swept = |point: Vec3| {
        let along = point.dot(toward);
        let outline = cavity.sample(point - toward * (along - outline_at));
        (outline_at - along).max(along - out_to).max(outline)
    };
    let candidates = sweep_tiles(cavity, toward, outline_at, out_to);
    Some(assemble(grid, band_mm, candidates, |voxel| {
        cavity.value(voxel).min(swept(grid.position(voxel)))
    }))
}

/// The tiles the sweep of `cavity` from `from` to `to` along `toward` may cross: the
/// cavity's own, and every tile its outline at `from` passes through on the way.
fn sweep_tiles(cavity: &Sdf, toward: Vec3, from: Scalar, to: Scalar) -> HashSet<IVec3> {
    let grid = cavity.grid();
    let reach = grid.tile_radius() + cavity.band_mm();
    let step = grid.tile_radius();
    let mut candidates: HashSet<IVec3> = cavity.tile_keys().collect();
    let outline = cavity
        .tile_keys()
        .chain(cavity.inside_tiles())
        .filter(|tile| {
            let along = grid.tile_center(*tile).dot(toward);
            (from - reach..=from + reach).contains(&along)
        });
    for tile in outline.collect::<Vec<_>>() {
        let center = grid.tile_center(tile);
        let mut along = center.dot(toward);
        while along <= to + reach {
            let passed = tile_of(grid.voxel(center + toward * (along - center.dot(toward))));
            for z in -1..=1 {
                for y in -1..=1 {
                    for x in -1..=1 {
                        candidates.insert(passed + IVec3::new(x, y, z));
                    }
                }
            }
            along += step;
        }
    }
    candidates
}

/// How far along `toward` the furthest point of `model` lies: the floor facing that way.
fn floor_at(model: &Mesh, toward: Vec3) -> Option<Scalar> {
    let floor = model
        .vertices
        .iter()
        .map(|vertex| vertex.dot(toward))
        .fold(Scalar::NEG_INFINITY, Scalar::max);
    floor.is_finite().then_some(floor)
}

/// `solid` cut flat where `model`'s floor facing `toward` lies, keeping the side inside the
/// model: the opened cavity reaches a band past the floor, and a lattice standing in it would
/// stand under the plate.
pub(super) fn stopped_at_floor(solid: Mesh, model: &Mesh, toward: Vec3) -> Mesh {
    let Some(toward) = toward.try_normalize() else {
        return solid;
    };
    let plane = floor_at(model, toward).and_then(|floor| Plane::at(toward, floor));
    match plane {
        Some(plane) if !solid.faces.is_empty() => cut(&solid, plane).below,
        _ => solid,
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{cuboid, signed_volume};

    use super::*;

    /// A 20 mm cube on the plate with a 16 mm square void from inside its roof down through
    /// its floor, and a 4 mm cube standing clear over it as a shell of its own.
    fn opened_cube() -> (Mesh, Mesh, Opening) {
        let mut model = cuboid(Vec3::ZERO, Vec3::splat(20.0));
        append(
            &mut model,
            &cuboid(Vec3::new(8.0, 8.0, 30.0), Vec3::new(12.0, 12.0, 34.0)),
        );
        let void = cuboid(Vec3::new(2.0, 2.0, -0.5), Vec3::new(18.0, 18.0, 18.0));
        let opening = Opening {
            toward: Vec3::NEG_Z,
            wall_mm: 2.0,
            band_mm: 0.4,
            voxel_mm: 0.2,
        };
        (model, void, opening)
    }

    #[test]
    fn the_floor_alone_through_the_engine_gives_what_the_whole_model_does() {
        let (model, void, opening) = opened_cube();
        let (near, near_cavity) =
            near_the_floor(&model, &void, opening).expect("a box of a cavity cuts cleanly");
        let (whole, whole_cavity) = whole(&model, &void).expect("the cube is a solid");

        let report = diagnose(&near);
        assert!(
            report.unbalanced_edges == 0 && report.boundary_edges == 0,
            "{report:?}"
        );
        // 8000 - 16 * 16 * 18 for the open cube, and 64 for the cube over it.
        let (got, expected) = (signed_volume(&near), signed_volume(&whole));
        assert!(
            (expected - (8000.0 - 4608.0 + 64.0)).abs() < 1e-2,
            "got {expected}"
        );
        assert!(
            (got - expected).abs() < 1e-3,
            "near the floor {got}, whole {expected}"
        );
        let area = |mesh: &Mesh, faces: Range<usize>| {
            let part = Mesh::new(mesh.vertices.clone(), mesh.faces[faces].to_vec());
            part.surface_area()
        };
        assert!(
            (area(&near, near_cavity) - area(&whole, whole_cavity)).abs() < 1e-2,
            "both bound the same cavity"
        );
    }
}
