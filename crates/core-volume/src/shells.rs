use core_geometry::{Mesh, Scalar, Vec3, Winding, signed_volume, split};

mod buried;

/// Faces of a shell asked a question of, most of which must answer yes.
pub(crate) const SAMPLES: usize = 64;

/// How far past its face a sample stands, as a fraction of the shell's own diagonal: off
/// the face, and far short of any wall that could be around it.
const STAND_OFF: Scalar = 1e-3;

/// A shell enclosing less than this over its area, in millimetres, is a sheet with no
/// inside: a fiftieth of the finest lattice (`FINE_LATTICE_MM`), and a body's ratio is a third of its radius.
const FLAT_MM: Scalar = 1e-3;

/// The shells of `mesh` that bound the solid, and the winding number of all of it.
pub(crate) struct Bounding {
    /// Those shells as one surface, or `None` when that is all of `mesh`.
    pub(crate) surface: Option<Mesh>,
    /// Built over all of `mesh` where more than one shell bounds the solid, so the field
    /// signing them needs no hierarchy of its own (`Signer::of_shells`).
    pub(crate) winding: Option<Winding>,
}

/// The shells of `mesh` that bound the solid.
///
/// A shell lying inside another adds nothing to what prints, since the rasteriser fills
/// by the non-zero rule, but a field built over it reads its outside as air and cuts a
/// cavity wall around it. Such a shell is left out of the fields and stays in the model,
/// so it still prints solid wherever it is; with `bury`, so are buried faces (ADR 0450).
pub(crate) fn outer_shells(mesh: &Mesh, bury: bool) -> Bounding {
    if is_one_piece(mesh) {
        return Bounding {
            surface: None,
            winding: None,
        };
    }
    let pieces = split(mesh);

    let winding = Winding::build(mesh);
    let total = pieces.len();
    let bounding: Vec<bool> = pieces
        .iter()
        .map(|piece| !is_flat(piece) && !is_enclosed(piece, mesh, &winding))
        .collect();
    let outer: Vec<&Mesh> = pieces
        .iter()
        .zip(&bounding)
        .filter_map(|(piece, bounds)| bounds.then_some(piece))
        .collect();
    if bury && outer.len() > 1 {
        return Bounding {
            surface: Some(buried::unburied(&pieces, &bounding)),
            winding: Some(winding),
        };
    }
    if outer.len() == total || outer.is_empty() {
        return Bounding {
            surface: None,
            winding: Some(winding),
        };
    }

    let mut surface = Mesh::default();
    for piece in &outer {
        surface.append(piece);
    }
    Bounding {
        surface: Some(surface),
        winding: (outer.len() > 1).then_some(winding),
    }
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

/// Whether `piece` encloses next to nothing for the surface it has.
fn is_flat(piece: &Mesh) -> bool {
    signed_volume(piece).abs() < FLAT_MM * piece.surface_area()
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
    most_faces(piece, |centroid, normal| {
        winding.is_inside(mesh, centroid + normal * stand_off)
    })
}

/// Whether `asked` holds of most of up to [`SAMPLES`] faces of `piece`, spread over it, given
/// each one's centroid and unit normal; a face with no normal is not asked.
pub(crate) fn most_faces(piece: &Mesh, asked: impl Fn(Vec3, Vec3) -> bool) -> bool {
    let step = (piece.faces.len() / SAMPLES).max(1);
    let (mut count, mut held) = (0, 0);
    for face in (0..piece.faces.len()).step_by(step) {
        let Some(triangle) = piece.triangle(face) else {
            continue;
        };
        let Some(normal) = triangle.normal_unnormalized().try_normalize() else {
            continue;
        };
        count += 1;
        let centroid = (triangle.a + triangle.b + triangle.c) / 3.0;
        held += usize::from(asked(centroid, normal));
    }
    count > 0 && 2 * held > count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{joined, sealed};

    /// An axis-aligned box from `low` to `high`, wound outward.
    fn cube(low: Vec3, high: Vec3) -> Mesh {
        core_geometry::cuboid(low, high)
    }

    #[test]
    fn a_shell_inside_another_is_left_out() {
        let outer = cube(Vec3::ZERO, Vec3::splat(20.0));
        let inner = cube(Vec3::splat(5.0), Vec3::splat(8.0));
        let surface = outer_shells(&joined(&[outer.clone(), inner]), false)
            .surface
            .expect("the inner box bounds nothing");
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
        let surface = outer_shells(&joined(&[outer.clone(), resting]), false)
            .surface
            .expect("the floor is shared");
        assert_eq!(surface.faces.len(), outer.faces.len());
    }

    #[test]
    fn a_sheet_with_no_thickness_is_left_out() {
        let body = cube(Vec3::ZERO, Vec3::splat(20.0));
        let sheet = cube(Vec3::new(10.0, 5.0, 15.0), Vec3::new(10.0, 15.0, 25.0));
        let surface = outer_shells(&joined(&[body.clone(), sheet]), false)
            .surface
            .expect("the sheet has no inside");
        assert_eq!(surface.faces.len(), body.faces.len());
    }

    #[test]
    fn two_bodies_side_by_side_both_bound_the_solid() {
        let left = cube(Vec3::ZERO, Vec3::splat(10.0));
        let right = cube(Vec3::new(20.0, 0.0, 0.0), Vec3::new(30.0, 10.0, 10.0));
        assert!(
            outer_shells(&joined(&[left, right]), false)
                .surface
                .is_none()
        );
    }

    #[test]
    fn a_wall_wound_inward_is_the_inside_of_a_hollow_model_and_kept() {
        let outer = cube(Vec3::ZERO, Vec3::splat(20.0));
        let mut cavity = cube(Vec3::splat(2.0), Vec3::splat(18.0));
        for face in &mut cavity.faces {
            face.swap(1, 2);
        }
        assert!(
            outer_shells(&joined(&[outer, cavity]), false)
                .surface
                .is_none()
        );
    }

    #[test]
    fn the_inside_wall_of_a_hollow_model_is_not_buried() {
        let model = sealed(
            (Vec3::ZERO, Vec3::splat(20.0)),
            &[(Vec3::splat(2.0), Vec3::splat(18.0))],
        );
        let surface = outer_shells(&model, true)
            .surface
            .expect("two bounding shells are asked what buries them");
        assert_eq!(
            surface.faces.len(),
            model.faces.len(),
            "the box's 12 triangles and its void's 12 all bound the solid"
        );
    }

    #[test]
    fn overlapping_bodies_keep_every_face_unless_buried_faces_are_asked_for() {
        let left = cube(Vec3::ZERO, Vec3::splat(20.0));
        let right = cube(Vec3::new(10.0, 0.0, 0.0), Vec3::new(30.0, 20.0, 20.0));
        let mesh = joined(&[left, right]);
        assert!(outer_shells(&mesh, false).surface.is_none());
        let buried = outer_shells(&mesh, true)
            .surface
            .expect("each box buries an end in the other");
        assert_eq!(
            buried.faces.len(),
            24 - 4,
            "each box loses its two buried triangles"
        );
    }
}
