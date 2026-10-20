use core_geometry::{FastMap, FastSet, Mesh, Scalar, Vec3};
use rayon::prelude::*;

use crate::cancel::Cancel;
use crate::error::VolumeError;

/// A mesh split finer where it was painted, each face knowing the face it was cut from.
pub(super) struct Refined {
    pub mesh: Mesh,
    /// The face of the mesh handed in that each face lies on.
    pub origin: Vec<u32>,
    pub painted: Vec<bool>,
}

/// `mesh` with every face `painted` names split at the middle of each edge longer than
/// `edge_mm`, until none is or another pass could pass `most_faces`, and whether it stopped
/// for that. A face beside a painted one is split along their shared edge, so it stays closed.
pub(super) fn refine(
    mesh: &Mesh,
    painted: &(dyn Fn(usize) -> bool + Sync),
    edge_mm: Scalar,
    most_faces: usize,
    cancel: Cancel<'_>,
) -> Result<(Refined, bool), VolumeError> {
    let mut refined = Refined {
        mesh: mesh.clone(),
        origin: (0..mesh.faces.len() as u32).collect(),
        painted: (0..mesh.faces.len()).into_par_iter().map(painted).collect(),
    };
    let limit = edge_mm * edge_mm;
    loop {
        if cancel.asked() {
            return Err(VolumeError::Cancelled);
        }
        let middles = long_edges(&refined, limit);
        if middles.is_empty() {
            return Ok((refined, false));
        }
        // Each split edge adds a face on either side of it, slivers or not.
        if refined.mesh.faces.len() + 2 * middles.len() > most_faces {
            return Ok((refined, true));
        }
        split(&mut refined, &middles);
    }
}

fn key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// The painted edges longer than `limit` squared, each given the vertex its middle
/// becomes, in an order that does not depend on the threads.
fn long_edges(refined: &Refined, limit: Scalar) -> FastMap<(u32, u32), u32> {
    let vertices = &refined.mesh.vertices;
    let long: FastSet<(u32, u32)> = refined
        .mesh
        .faces
        .par_iter()
        .zip(&refined.painted)
        .filter(|(_, painted)| **painted)
        .flat_map_iter(|([a, b, c], _)| [(*a, *b), (*b, *c), (*c, *a)])
        .filter(|(a, b)| vertices[*a as usize].distance_squared(vertices[*b as usize]) > limit)
        .map(|(a, b)| key(a, b))
        .collect();
    let mut long: Vec<(u32, u32)> = long.into_iter().collect();
    long.sort_unstable();
    let first = vertices.len() as u32;
    long.into_iter()
        .enumerate()
        .map(|(at, edge)| (edge, first + at as u32))
        .collect()
}

/// Puts a vertex at the middle of every edge of `middles` and splits each face across
/// the ones it has.
fn split(refined: &mut Refined, middles: &FastMap<(u32, u32), u32>) {
    let mut added = vec![Vec3::ZERO; middles.len()];
    let first = refined.mesh.vertices.len();
    for ((a, b), at) in middles {
        let vertices = &refined.mesh.vertices;
        added[*at as usize - first] = (vertices[*a as usize] + vertices[*b as usize]) / 2.0;
    }
    refined.mesh.vertices.extend(added);

    let vertices = &refined.mesh.vertices;
    let pieces: Vec<([u32; 3], u32, bool)> = refined
        .mesh
        .faces
        .par_iter()
        .zip(&refined.origin)
        .zip(&refined.painted)
        .flat_map_iter(|((face, origin), painted)| {
            let (faces, count) = pieces(*face, middles, vertices);
            faces
                .into_iter()
                .take(count)
                .map(move |piece| (piece, *origin, *painted))
        })
        .collect();
    refined.mesh.faces = pieces.iter().map(|piece| piece.0).collect();
    refined.origin = pieces.iter().map(|piece| piece.1).collect();
    refined.painted = pieces.iter().map(|piece| piece.2).collect();
}

/// `face` split across the middles it has, wound as it was, and how many pieces that is.
fn pieces(
    face: [u32; 3],
    middles: &FastMap<(u32, u32), u32>,
    vertices: &[Vec3],
) -> ([[u32; 3]; 4], usize) {
    let middle = |[a, b, c]: [u32; 3], side: usize| {
        let (p, q) = [(a, b), (b, c), (c, a)][side];
        middles.get(&key(p, q)).copied()
    };
    let marked = |face| (0..3).filter(|side| middle(face, *side).is_some()).count();
    // Turned so that the sides split come first: one is `ab`, two are `ab` and `bc`.
    let turned = (0..3)
        .map(|turn| [face[turn], face[(turn + 1) % 3], face[(turn + 2) % 3]])
        .find(|turned| match marked(face) {
            1 => middle(*turned, 0).is_some(),
            2 => middle(*turned, 2).is_none(),
            _ => true,
        })
        .unwrap_or(face);
    let [a, b, c] = turned;
    let [ab, bc, ca] = [0, 1, 2].map(|side| middle(turned, side));
    match (ab, bc, ca) {
        (Some(on_ab), None, None) => ([[a, on_ab, c], [on_ab, b, c], [0; 3], [0; 3]], 2),
        (Some(on_ab), Some(on_bc), None) => {
            // The quad left beside the corner cut off is split along its shorter diagonal.
            let at = |index: u32| vertices[index as usize];
            let quad = if at(a).distance_squared(at(on_bc)) <= at(on_ab).distance_squared(at(c)) {
                [[a, on_ab, on_bc], [a, on_bc, c]]
            } else {
                [[a, on_ab, c], [on_ab, on_bc, c]]
            };
            ([[on_ab, b, on_bc], quad[0], quad[1], [0; 3]], 3)
        }
        (Some(on_ab), Some(on_bc), Some(on_ca)) => (
            [
                [a, on_ab, on_ca],
                [on_ab, b, on_bc],
                [on_ca, on_bc, c],
                [on_ab, on_bc, on_ca],
            ],
            4,
        ),
        _ => ([face, [0; 3], [0; 3], [0; 3]], 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Vec3, cuboid, diagnose};

    fn lid_of(mesh: &Mesh) -> impl Fn(usize) -> bool + Sync + '_ {
        |face: usize| {
            mesh.triangle(face)
                .is_some_and(|triangle| triangle.normal_unnormalized().z > 0.0)
        }
    }

    #[test]
    fn a_painted_face_is_split_finer_than_asked_and_the_mesh_stays_closed() {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let lid = lid_of(&mesh);
        let (refined, cut_short) =
            refine(&mesh, &lid, 1.0, usize::MAX, Cancel::never()).expect("nothing stops it");
        assert!(!cut_short);
        let longest = refined
            .mesh
            .faces
            .iter()
            .zip(&refined.painted)
            .filter(|(_, painted)| **painted)
            .flat_map(|([a, b, c], _)| [(*a, *b), (*b, *c), (*c, *a)])
            .map(|(a, b)| {
                refined.mesh.vertices[a as usize].distance(refined.mesh.vertices[b as usize])
            })
            .fold(0.0, Scalar::max);
        assert!(
            longest <= 1.0,
            "the longest painted edge is {longest} mm, over 1 mm"
        );
        assert!(
            diagnose(&refined.mesh).is_closed(),
            "a split side splits both faces on it"
        );
        let area = |painted: bool| -> Scalar {
            (0..refined.mesh.faces.len())
                .filter(|face| refined.painted[*face] == painted)
                .filter_map(|face| refined.mesh.triangle(face))
                .map(|triangle| triangle.area())
                .sum()
        };
        assert!(
            (area(true) - 100.0).abs() < 1e-3 && (area(false) - 500.0).abs() < 1e-3,
            "the 10 mm lid and the five other sides keep their areas"
        );
    }

    #[test]
    fn a_split_stops_short_of_more_faces_than_it_may_make() {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let (refined, cut_short) =
            refine(&mesh, &lid_of(&mesh), 0.01, 200, Cancel::never()).expect("nothing stops it");
        assert!(
            cut_short,
            "a 0.01 mm edge over 100 mm² needs millions of faces"
        );
        assert!(refined.mesh.faces.len() <= 200);
        assert!(diagnose(&refined.mesh).is_closed());
    }

    #[test]
    fn a_split_asked_to_stop_stops() {
        let mesh = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let stopped = || true;
        assert!(matches!(
            refine(
                &mesh,
                &lid_of(&mesh),
                1.0,
                usize::MAX,
                Cancel::when(&stopped)
            ),
            Err(VolumeError::Cancelled)
        ));
    }
}
