use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::topology::{agree, edge_groups, edge_uses};
use crate::{Mesh, Scalar};

/// What [`orient_outward`] had to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Orientation {
    /// Faces whose winding differs from the input.
    pub flipped_faces: usize,
    /// Closed shells that were wound inwards and got turned inside out as a whole.
    pub inverted_shells: usize,
    /// False for a surface with no consistent orientation at all, such as a Mobius strip.
    pub orientable: bool,
}

impl Orientation {
    pub fn unchanged(&self) -> bool {
        self.flipped_faces == 0
    }
}

/// Makes every face wind the same way round, and turns closed shells outwards.
///
/// STL face normals are ignored throughout: exporters write them inconsistently, so
/// winding order is the only trustworthy source of orientation. Run [`crate::weld`]
/// first, or no two faces will look adjacent.
pub fn orient_outward(mesh: &mut Mesh) -> Orientation {
    if mesh.faces.is_empty() {
        return Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        };
    }

    let uses = edge_uses(mesh);
    let mut adjacency: Vec<Vec<(u32, bool)>> = vec![Vec::new(); mesh.faces.len()];
    let mut touches_open_edge = vec![false; mesh.faces.len()];

    for group in edge_groups(&uses) {
        if let [first, second] = group {
            let agreed = agree(first, second);
            adjacency[first.face as usize].push((second.face, agreed));
            adjacency[second.face as usize].push((first.face, agreed));
        } else {
            for edge_use in group {
                touches_open_edge[edge_use.face as usize] = true;
            }
        }
    }

    let (shell, shells, mut flip, orientable) = propagate(&adjacency);
    for (index, face) in mesh.faces.iter_mut().enumerate() {
        if flip[index] {
            face.swap(1, 2);
        }
    }

    let invert = shells_to_invert(mesh, &shell, shells, &touches_open_edge);
    let inverted_shells = invert.iter().filter(|wanted| **wanted).count();
    for (index, face) in mesh.faces.iter_mut().enumerate() {
        if invert[shell[index] as usize] {
            face.swap(1, 2);
            flip[index] = !flip[index];
        }
    }

    Orientation {
        flipped_faces: flip.iter().filter(|flipped| **flipped).count(),
        inverted_shells,
        orientable,
    }
}

/// Walks the face graph assigning each face the winding its neighbours imply.
///
/// Within a shell only the two assignments matter, not which one the walk happens to
/// start from, so the smaller one wins. Without that the result would depend on which
/// face the walk seeded at.
fn propagate(adjacency: &[Vec<(u32, bool)>]) -> (Vec<u32>, usize, Vec<bool>, bool) {
    let mut shell = vec![u32::MAX; adjacency.len()];
    let mut flip = vec![false; adjacency.len()];
    let mut orientable = true;
    let mut shells = 0;
    let mut queue = VecDeque::new();
    let mut members = Vec::new();

    for seed in 0..adjacency.len() {
        if shell[seed] != u32::MAX {
            continue;
        }
        shell[seed] = shells;
        queue.push_back(seed as u32);
        members.clear();
        members.push(seed as u32);

        while let Some(face) = queue.pop_front() {
            for &(next, agreed) in &adjacency[face as usize] {
                let wanted = flip[face as usize] == agreed;
                if shell[next as usize] == u32::MAX {
                    shell[next as usize] = shells;
                    flip[next as usize] = wanted;
                    members.push(next);
                    queue.push_back(next);
                } else if flip[next as usize] != wanted {
                    orientable = false;
                }
            }
        }

        let flipped = members.iter().filter(|face| flip[**face as usize]).count();
        if flipped * 2 > members.len() {
            for face in &members {
                flip[*face as usize] = !flip[*face as usize];
            }
        }
        shells += 1;
    }

    (shell, shells as usize, flip, orientable)
}

/// Closed shells whose volume came out negative, meaning they are wound inwards.
fn shells_to_invert(
    mesh: &Mesh,
    shell: &[u32],
    shells: usize,
    touches_open_edge: &[bool],
) -> Vec<bool> {
    let mut open = vec![false; shells];
    for (face, is_open) in touches_open_edge.iter().enumerate() {
        if *is_open {
            open[shell[face] as usize] = true;
        }
    }

    let mut volume = vec![0.0 as Scalar; shells];
    for index in 0..mesh.faces.len() {
        if let Some(triangle) = mesh.triangle(index) {
            volume[shell[index] as usize] += triangle.a.dot(triangle.b.cross(triangle.c));
        }
    }

    (0..shells).map(|s| !open[s] && volume[s] < 0.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Vec3, signed_volume};

    /// Corner tetrahedron with every face wound outwards. Volume is 1/6.
    fn tetrahedron() -> Mesh {
        Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        )
    }

    #[test]
    fn an_already_correct_mesh_is_left_alone() {
        let mut mesh = tetrahedron();
        let report = orient_outward(&mut mesh);

        assert!(report.unchanged());
        assert!(report.orientable);
        assert_eq!(report.inverted_shells, 0);
        assert!((signed_volume(&mesh) - 1.0 / 6.0).abs() < 1e-6);
    }

    #[test]
    fn a_single_inverted_face_is_flipped_back() {
        let mut mesh = tetrahedron();
        mesh.faces[2].swap(1, 2);
        let report = orient_outward(&mut mesh);

        assert_eq!(report.flipped_faces, 1);
        assert_eq!(report.inverted_shells, 0);
        assert_eq!(mesh.faces, tetrahedron().faces);
    }

    #[test]
    fn a_wholly_inverted_shell_is_turned_outwards() {
        let mut mesh = tetrahedron();
        for face in &mut mesh.faces {
            face.swap(1, 2);
        }
        let report = orient_outward(&mut mesh);

        assert_eq!(report.inverted_shells, 1);
        assert_eq!(report.flipped_faces, 4);
        assert!(signed_volume(&mesh) > 0.0);
    }

    #[test]
    fn an_empty_mesh_is_trivially_oriented() {
        let report = orient_outward(&mut Mesh::default());
        assert!(report.unchanged());
        assert!(report.orientable);
    }
}
