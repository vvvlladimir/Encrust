use core_geometry::{Mesh, Scalar, Winding, split};

/// Faces of a shell asked whether something else stands around it.
const SAMPLES: usize = 64;

/// How far past its face a sample stands, as a fraction of the shell's own diagonal: off
/// the face, and far short of any wall that could be around it.
const STAND_OFF: Scalar = 1e-3;

/// The shells of `mesh` that bound the solid, or `None` when that is all of them.
///
/// A shell lying inside another adds nothing to what prints, since the rasteriser fills
/// by the non-zero rule, but a field built over it reads its outside as air and cuts a
/// cavity wall around it. Such a shell is left out of the fields and stays in the model,
/// so it still prints solid wherever it is. See `docs/design/hollowing.md`.
pub(crate) fn outer_shells(mesh: &Mesh) -> Option<Mesh> {
    if is_one_piece(mesh) {
        return None;
    }
    let pieces = split(mesh);

    let winding = Winding::build(mesh);
    let total = pieces.len();
    let outer: Vec<Mesh> = pieces
        .into_iter()
        .filter(|piece| !is_enclosed(piece, mesh, &winding))
        .collect();
    if outer.len() == total || outer.is_empty() {
        return None;
    }

    let mut surface = Mesh::default();
    for piece in &outer {
        let offset = surface.vertices.len() as u32;
        surface.vertices.extend_from_slice(&piece.vertices);
        surface.faces.extend(
            piece
                .faces
                .iter()
                .map(|face| face.map(|corner| corner + offset)),
        );
    }
    Some(surface)
}

/// Whether every face of `mesh` is joined to every other through shared vertices: what
/// nearly every model is, told without copying it into pieces.
fn is_one_piece(mesh: &Mesh) -> bool {
    let mut parent: Vec<u32> = (0..mesh.vertices.len() as u32).collect();
    let find = |parent: &mut Vec<u32>, mut node: u32| {
        while parent[node as usize] != node {
            let up = parent[parent[node as usize] as usize];
            parent[node as usize] = up;
            node = up;
        }
        node
    };
    for face in &mesh.faces {
        let root = find(&mut parent, face[0]);
        for corner in &face[1..] {
            let other = find(&mut parent, *corner);
            parent[other as usize] = root;
        }
    }
    let Some(first) = mesh.faces.first() else {
        return true;
    };
    let root = find(&mut parent, first[0]);
    mesh.faces
        .iter()
        .all(|face| find(&mut parent, face[0]) == root)
}

/// Whether most of `piece` has the rest of `mesh` standing around it: just outside its
/// faces the winding number of the whole is still that of the solid.
///
/// Most rather than all, because a shell resting in another often shares a face with it,
/// and just outside that face is the air both of them stand on.
fn is_enclosed(piece: &Mesh, mesh: &Mesh, winding: &Winding) -> bool {
    let Some(bounds) = piece.aabb() else {
        return false;
    };
    let stand_off = (bounds.maxs - bounds.mins).length() * STAND_OFF;
    let step = (piece.faces.len() / SAMPLES).max(1);

    let (mut asked, mut inside) = (0, 0);
    for face in (0..piece.faces.len()).step_by(step) {
        let Some(triangle) = piece.triangle(face) else {
            continue;
        };
        let Some(normal) = triangle.normal_unnormalized().try_normalize() else {
            continue;
        };
        let centroid = (triangle.a + triangle.b + triangle.c) / 3.0;
        asked += 1;
        if winding.is_inside(mesh, centroid + normal * stand_off) {
            inside += 1;
        }
    }
    asked > 0 && 2 * inside > asked
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;

    /// An axis-aligned box from `low` to `high`, wound outward.
    fn cube(low: Vec3, high: Vec3) -> Mesh {
        let corner = |x: bool, y: bool, z: bool| {
            Vec3::new(
                if x { high.x } else { low.x },
                if y { high.y } else { low.y },
                if z { high.z } else { low.z },
            )
        };
        Mesh::new(
            vec![
                corner(false, false, false),
                corner(true, false, false),
                corner(true, true, false),
                corner(false, true, false),
                corner(false, false, true),
                corner(true, false, true),
                corner(true, true, true),
                corner(false, true, true),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
        )
    }

    fn joined(parts: &[Mesh]) -> Mesh {
        let mut whole = Mesh::default();
        for part in parts {
            let offset = whole.vertices.len() as u32;
            whole.vertices.extend_from_slice(&part.vertices);
            whole.faces.extend(
                part.faces
                    .iter()
                    .map(|face| face.map(|corner| corner + offset)),
            );
        }
        whole
    }

    #[test]
    fn a_shell_inside_another_is_left_out() {
        let outer = cube(Vec3::ZERO, Vec3::splat(20.0));
        let inner = cube(Vec3::splat(5.0), Vec3::splat(8.0));
        let surface =
            outer_shells(&joined(&[outer.clone(), inner])).expect("the inner box bounds nothing");
        assert_eq!(surface.faces.len(), outer.faces.len());
        assert_eq!(
            surface.aabb(),
            outer.aabb(),
            "what is left is the outer box"
        );
    }

    #[test]
    fn a_shell_resting_on_the_floor_of_another_is_still_inside_it() {
        let outer = cube(Vec3::ZERO, Vec3::splat(20.0));
        let resting = cube(Vec3::new(5.0, 5.0, 0.0), Vec3::new(9.0, 9.0, 2.0));
        let surface =
            outer_shells(&joined(&[outer.clone(), resting])).expect("the floor is shared");
        assert_eq!(surface.faces.len(), outer.faces.len());
    }

    #[test]
    fn two_bodies_side_by_side_both_bound_the_solid() {
        let left = cube(Vec3::ZERO, Vec3::splat(10.0));
        let right = cube(Vec3::new(20.0, 0.0, 0.0), Vec3::new(30.0, 10.0, 10.0));
        assert!(outer_shells(&joined(&[left, right])).is_none());
    }

    #[test]
    fn a_wall_wound_inward_is_the_inside_of_a_hollow_model_and_kept() {
        let outer = cube(Vec3::ZERO, Vec3::splat(20.0));
        let mut cavity = cube(Vec3::splat(2.0), Vec3::splat(18.0));
        for face in &mut cavity.faces {
            face.swap(1, 2);
        }
        assert!(outer_shells(&joined(&[outer, cavity])).is_none());
    }
}
