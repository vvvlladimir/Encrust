use crate::hash::{FastMap, FastSet};
use crate::repair::boundary_loops;
use crate::topology::{EdgeUse, edge_groups, edge_uses};
use crate::validate::{degenerate_faces, duplicate_faces};
use crate::{Components, Mesh, Region};

/// Where the faults [`crate::diagnose`] counts lie in a mesh, for a window to mark them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Faults {
    /// Edges used by one face only, as their two vertices, lower first.
    pub open_edges: Vec<[u32; 2]>,
    /// The one face using each open edge, in the same order: which side the rim lies on.
    pub open_faces: Vec<u32>,
    /// The holes those edges rim, one vertex list each: a loop wound as the patch closing it
    /// would be, or, where the rim branches, the vertices of the edges no loop took.
    pub holes: Vec<Vec<u32>>,
    /// Faces with a repeated vertex or no area.
    pub flat_faces: Region,
    /// Faces laid over others that a repair would drop.
    pub doubled_faces: Region,
}

/// The places of the faults in `mesh`, each kind as many as `diagnose` counts of it.
pub fn faults(mesh: &Mesh) -> Faults {
    let uses = edge_uses(mesh);
    let (open_edges, open_faces): (Vec<[u32; 2]>, Vec<u32>) = edge_groups(&uses)
        .filter(|group| group.len() == 1)
        .map(|group| ([group[0].edge.0, group[0].edge.1], group[0].face))
        .unzip();
    let holes = holes(mesh, &uses, &open_edges);
    let mut flat_faces = Region::default();
    for (face, flat) in degenerate_faces(mesh).into_iter().enumerate() {
        flat_faces.set(face, flat);
    }
    let mut doubled_faces = Region::default();
    for face in duplicate_faces(mesh) {
        doubled_faces.set(face, true);
    }
    Faults {
        open_edges,
        open_faces,
        holes,
        flat_faces,
        doubled_faces,
    }
}

/// The loops `boundary_loops` walks, then the open edges none of them took, grouped by the
/// vertices they share.
fn holes(mesh: &Mesh, uses: &[EdgeUse], open_edges: &[[u32; 2]]) -> Vec<Vec<u32>> {
    let (mut holes, _) = boundary_loops(uses);
    let walked: FastSet<[u32; 2]> = holes
        .iter()
        .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)))
        .map(|(&a, &b)| [a.min(b), a.max(b)])
        .collect();
    let left: Vec<[u32; 2]> = open_edges
        .iter()
        .filter(|edge| !walked.contains(*edge))
        .copied()
        .collect();
    let mut joined = Components::new(mesh.vertices.len());
    for [a, b] in &left {
        joined.union(*a, *b);
    }
    let mut chain_of: FastMap<u32, usize> = FastMap::default();
    let mut chains: Vec<Vec<u32>> = Vec::new();
    let mut seen = FastSet::default();
    for corner in left.iter().flatten() {
        if !seen.insert(*corner) {
            continue;
        }
        let at = *chain_of.entry(joined.find(*corner)).or_insert_with(|| {
            chains.push(Vec::new());
            chains.len() - 1
        });
        chains[at].push(*corner);
    }
    holes.extend(chains);
    holes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Vec3, cuboid, diagnose};

    #[test]
    fn a_cube_has_no_faults() {
        assert_eq!(faults(&cuboid(Vec3::ZERO, Vec3::ONE)), Faults::default());
    }

    #[test]
    fn a_cube_missing_its_top_has_the_four_rim_edges_open_at_height_one() {
        let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
        let vertices = mesh.vertices.clone();
        mesh.faces
            .retain(|face| face.iter().any(|&corner| vertices[corner as usize].z < 1.0));

        let found = faults(&mesh);
        assert_eq!(found.open_edges.len(), 4, "the square rim of the hole");
        assert_eq!(found.open_edges.len(), diagnose(&mesh).boundary_edges);
        for (edge, face) in found.open_edges.iter().zip(&found.open_faces) {
            assert!(
                edge.iter()
                    .all(|corner| mesh.faces[*face as usize].contains(corner)),
                "face {face} uses the open edge {edge:?}"
            );
        }
        assert!(
            found
                .open_edges
                .iter()
                .flatten()
                .all(|&corner| (mesh.vertices[corner as usize].z - 1.0).abs() < 1e-6),
            "every open edge lies on the rim at z = 1"
        );
    }

    #[test]
    fn two_cubes_missing_their_tops_have_two_holes_of_four_corners() {
        let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
        mesh.append(&cuboid(Vec3::splat(5.0), Vec3::splat(6.0)));
        let vertices = mesh.vertices.clone();
        let on = |face: &[u32; 3], z: f32| {
            face.iter()
                .all(|&corner| (vertices[corner as usize].z - z).abs() < 1e-6)
        };
        mesh.faces.retain(|face| !on(face, 1.0) && !on(face, 6.0));

        let found = faults(&mesh);
        assert_eq!(found.holes.len(), diagnose(&mesh).holes);
        let mut corners: Vec<usize> = found.holes.iter().map(Vec::len).collect();
        corners.sort_unstable();
        assert_eq!(corners, vec![4, 4], "each rim is a square of four corners");
    }

    #[test]
    fn a_face_drawn_twice_and_one_without_area_are_found_where_they_are() {
        let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
        let first = mesh.faces[0];
        mesh.faces.push(first);
        mesh.faces.push([first[0], first[0], first[1]]);

        let found = faults(&mesh);
        let diagnostics = diagnose(&mesh);
        assert_eq!(found.doubled_faces.len(), diagnostics.duplicate_faces);
        assert_eq!(found.flat_faces.faces().collect::<Vec<_>>(), vec![13]);
        assert_eq!(diagnostics.degenerate_faces, 1);
    }
}
