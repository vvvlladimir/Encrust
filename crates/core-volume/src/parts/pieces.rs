use core_geometry::{Aabb, FastMap, Mesh, Scalar, Winding, signed_volume};

/// Points of a shell asked whether another shell stands around it.
const SAMPLES: usize = 16;

/// A closed body, or a scrap of surface, with every shell that lies inside it.
#[derive(Debug, Clone)]
pub(crate) struct Body {
    pub mesh: Mesh,
    /// What it holds, mm³: its volume where it is closed, and for an open scrap the
    /// volume of the cube of the same surface area.
    pub size_mm3: Scalar,
}

impl Body {
    pub fn of(mesh: Mesh) -> Self {
        let size_mm3 = size_of_mesh(&mesh);
        Self { mesh, size_mm3 }
    }
}

fn size_of_mesh(mesh: &Mesh) -> Scalar {
    if is_closed(mesh) {
        signed_volume(mesh).abs()
    } else {
        (mesh.surface_area() / 6.0).max(0.0).powf(1.5)
    }
}

/// Whether every edge of `mesh` has two faces.
fn is_closed(mesh: &Mesh) -> bool {
    let mut uses: FastMap<(u32, u32), u32> = FastMap::default();
    for face in &mesh.faces {
        for side in 0..3 {
            let (a, b) = (face[side], face[(side + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    uses.values().all(|count| *count == 2)
}

/// The shells of `mesh`: faces joined through edges exactly two faces share.
///
/// Two bodies that meet only at a corner, or along an edge three or four faces use, are
/// two shells, where joining by shared vertices would read them as one.
pub(crate) fn shells(mesh: &Mesh) -> Vec<Mesh> {
    let mut edges: FastMap<(u32, u32), Vec<u32>> = FastMap::default();
    for (index, face) in mesh.faces.iter().enumerate() {
        for side in 0..3 {
            let (a, b) = (face[side], face[(side + 1) % 3]);
            edges
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push(index as u32);
        }
    }
    let mut parent: Vec<u32> = (0..mesh.faces.len() as u32).collect();
    for faces in edges.values() {
        if let [left, right] = faces.as_slice() {
            union(&mut parent, *left, *right);
        }
    }
    let mut groups: FastMap<u32, Vec<u32>> = FastMap::default();
    for face in 0..mesh.faces.len() as u32 {
        groups
            .entry(find(&mut parent, face))
            .or_default()
            .push(face);
    }
    groups
        .into_values()
        .map(|faces| compact(mesh, &faces))
        .collect()
}

/// The faces `faces` of `mesh` as a mesh of their own, holding only the vertices they use.
fn compact(mesh: &Mesh, faces: &[u32]) -> Mesh {
    let mut moved: FastMap<u32, u32> = FastMap::default();
    let mut piece = Mesh::default();
    for face in faces {
        let corners = mesh.faces[*face as usize].map(|index| {
            *moved.entry(index).or_insert_with(|| {
                piece.vertices.push(mesh.vertices[index as usize]);
                piece.vertices.len() as u32 - 1
            })
        });
        piece.faces.push(corners);
    }
    piece
}

/// `shells` gathered into bodies: each shell no other one stands around, with every shell
/// inside it — a cavity, the lattice in it, a body sunk into another.
///
/// A shell is inside another when most of its sampled points are; see
/// `docs/design/parts.md`.
pub(crate) fn nest(shells: Vec<Mesh>) -> Vec<Body> {
    let mut order: Vec<(Mesh, Aabb)> = shells
        .into_iter()
        .filter_map(|shell| shell.aabb().map(|bounds| (shell, bounds)))
        .collect();
    order.sort_by(|(_, a), (_, b)| box_volume(b).total_cmp(&box_volume(a)));
    let mut windings: Vec<Option<Winding>> = vec![None; order.len()];
    let mut owner: Vec<usize> = (0..order.len()).collect();
    for inner in 1..order.len() {
        // The smallest earlier shell standing round this one is the one it is in.
        for outer in (0..inner).rev() {
            if owner[outer] != outer || !contains(&order[outer].1, &order[inner].1) {
                continue;
            }
            let winding = windings[outer].get_or_insert_with(|| Winding::build(&order[outer].0));
            if is_inside(&order[inner].0, &order[outer].0, winding) {
                owner[inner] = outer;
                break;
            }
        }
    }
    let mut bodies: FastMap<usize, Mesh> = FastMap::default();
    for (index, (shell, _)) in order.into_iter().enumerate() {
        bodies.entry(owner[index]).or_default().append(&shell);
    }
    bodies.into_values().map(Body::of).collect()
}

fn box_volume(bounds: &Aabb) -> Scalar {
    let size = bounds.maxs - bounds.mins;
    size.x * size.y * size.z
}

fn contains(outer: &Aabb, inner: &Aabb) -> bool {
    (outer.mins.cmple(inner.mins) & outer.maxs.cmpge(inner.maxs)).all()
}

/// Whether most of `shell`'s sampled vertices lie inside `around`. Most rather than all,
/// since a shell resting in another often shares a face with it.
fn is_inside(shell: &Mesh, around: &Mesh, winding: &Winding) -> bool {
    let step = (shell.vertices.len() / SAMPLES).max(1);
    let (mut asked, mut inside) = (0, 0);
    for vertex in shell.vertices.iter().step_by(step) {
        asked += 1;
        inside += usize::from(winding.is_inside(around, *vertex));
    }
    asked > 0 && 2 * inside > asked
}

fn find(parent: &mut [u32], index: u32) -> u32 {
    let mut root = index;
    while parent[root as usize] != root {
        root = parent[root as usize];
    }
    let mut walk = index;
    while parent[walk as usize] != root {
        let next = parent[walk as usize];
        parent[walk as usize] = root;
        walk = next;
    }
    root
}

fn union(parent: &mut [u32], left: u32, right: u32) {
    let (left, right) = (find(parent, left), find(parent, right));
    if left != right {
        parent[right.max(left) as usize] = left.min(right);
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid, weld};

    use super::*;

    fn joined(meshes: &[Mesh]) -> Mesh {
        let mut whole = Mesh::default();
        for mesh in meshes {
            whole.append(mesh);
        }
        weld(&whole, 1e-5).mesh
    }

    #[test]
    fn two_boxes_meeting_along_an_edge_are_two_shells() {
        let mesh = joined(&[
            cuboid(Vec3::ZERO, Vec3::splat(10.0)),
            cuboid(Vec3::new(10.0, 10.0, 0.0), Vec3::new(20.0, 20.0, 10.0)),
        ]);
        assert_eq!(shells(&mesh).len(), 2, "the shared edge has four faces");
    }

    #[test]
    fn a_cavity_stays_with_the_body_round_it() {
        let mut cavity = cuboid(Vec3::splat(2.0), Vec3::splat(8.0));
        for face in &mut cavity.faces {
            face.swap(1, 2);
        }
        let away = cuboid(Vec3::new(20.0, 0.0, 0.0), Vec3::new(25.0, 5.0, 5.0));
        let mesh = joined(&[cuboid(Vec3::ZERO, Vec3::splat(10.0)), cavity, away]);
        let mut bodies = nest(shells(&mesh));
        bodies.sort_by(|a, b| b.size_mm3.total_cmp(&a.size_mm3));
        assert_eq!(bodies.len(), 2);
        assert!(
            (bodies[0].size_mm3 - (1000.0 - 216.0)).abs() < 1e-2,
            "a 10 mm box less a 6 mm cavity, got {}",
            bodies[0].size_mm3
        );
    }
}
