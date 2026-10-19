use core_geometry::{
    Aabb, FastMap, Mesh, Scalar, Winding, diagnose, signed_volume, split_at_edges,
};

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
    let report = diagnose(mesh);
    report.boundary_edges == 0 && report.non_manifold_edges == 0
}

/// The shells of `mesh`: faces joined through edges exactly two faces share.
pub(crate) fn shells(mesh: &Mesh) -> Vec<Mesh> {
    split_at_edges(mesh)
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
