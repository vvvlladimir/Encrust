use std::collections::HashMap;

use core_geometry::{ClosestPoint, Mesh, Scalar, Vec3, Winding, diagnose};

/// Barycentric slack within which the nearest point counts as sitting on an edge or a
/// vertex rather than inside the face. A hundredth of a percent of the triangle, which is
/// well past the error of a projection onto it.
const ON_THE_SEAM: Scalar = 1e-4;

/// The squared sine of a triangle's angle at its first corner under which its barycentrics
/// are not trusted: about six degrees, where `f32` cancellation is still a tenth of
/// [`ON_THE_SEAM`].
const WELL_SHAPED: Scalar = 1e-2;

/// How a field decides which side of the surface a point is on.
///
/// The two answers differ only on a mesh that is not watertight, and that is the whole
/// point of having both; see
/// `docs/decisions/0055-the-sign-is-pseudonormal-with-winding-behind-it.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignMode {
    /// The pseudonormal on a mesh [`diagnose`] calls closed, the winding number otherwise.
    #[default]
    Auto,
    /// The angle-weighted pseudonormal of the nearest point. Nearly free, and exact only
    /// while the mesh is closed and consistently wound.
    Pseudonormal,
    /// The generalised winding number. Costs a hierarchy and a traversal per voxel, and
    /// degrades gracefully around holes and self-intersections.
    Winding,
}

/// The side-of-the-surface question, answered whichever way the mode asked for.
pub(crate) enum Signer {
    Pseudonormal(Pseudonormals),
    Winding(Winding),
}

impl Signer {
    pub(crate) fn new(mesh: &Mesh, mode: SignMode) -> Self {
        let resolved = match mode {
            SignMode::Auto if diagnose(mesh).is_closed() => SignMode::Pseudonormal,
            SignMode::Auto => SignMode::Winding,
            picked => picked,
        };

        match resolved {
            SignMode::Pseudonormal => Self::Pseudonormal(Pseudonormals::build(mesh)),
            _ => Self::Winding(Winding::build(mesh)),
        }
    }

    /// Whether `point` is inside the surface. `found` is its nearest point, which the
    /// pseudonormal needs and the winding number ignores.
    pub(crate) fn is_inside(&self, mesh: &Mesh, point: Vec3, found: &ClosestPoint) -> bool {
        match self {
            Self::Pseudonormal(normals) => {
                (point - found.point).dot(normals.at(mesh, point, found)) < 0.0
            }
            Self::Winding(winding) => winding.is_inside(mesh, point),
        }
    }
}

/// The angle-weighted normals of Baerentzen and Aanaes: one per face, one per vertex and
/// one per edge, so that a nearest point landing on a seam is signed by the seam's own
/// normal rather than by whichever face happened to win.
pub(crate) struct Pseudonormals {
    face: Vec<Vec3>,
    vertex: Vec<Vec3>,
    edge: HashMap<(u32, u32), Vec3>,
}

impl Pseudonormals {
    pub(crate) fn build(mesh: &Mesh) -> Self {
        let mut face = vec![Vec3::ZERO; mesh.faces.len()];
        let mut vertex = vec![Vec3::ZERO; mesh.vertices.len()];
        let mut edge: HashMap<(u32, u32), Vec3> = HashMap::new();

        for (index, corners) in mesh.faces.iter().enumerate() {
            let Some(triangle) = mesh.triangle(index) else {
                continue;
            };
            let normal = triangle.normal_unnormalized().normalize_or_zero();
            face[index] = normal;

            let points = [triangle.a, triangle.b, triangle.c];
            for corner in 0..3 {
                let here = points[corner];
                let (first, second) = (points[(corner + 1) % 3], points[(corner + 2) % 3]);
                // The interior angle at this corner is what weights the vertex normal.
                let angle = (first - here)
                    .normalize_or_zero()
                    .dot((second - here).normalize_or_zero())
                    .clamp(-1.0, 1.0)
                    .acos();
                if let Some(slot) = vertex.get_mut(corners[corner] as usize) {
                    *slot += normal * angle;
                }
                *edge
                    .entry(seam(corners[corner], corners[(corner + 1) % 3]))
                    .or_insert(Vec3::ZERO) += normal;
            }
        }

        Self { face, vertex, edge }
    }

    /// Normal to sign `point` against at its nearest point: the face's, the edge's or the
    /// vertex's, depending on where inside its triangle the point landed.
    fn at(&self, mesh: &Mesh, point: Vec3, found: &ClosestPoint) -> Vec3 {
        let (Some(corners), Some(triangle)) =
            (mesh.faces.get(found.face), mesh.triangle(found.face))
        else {
            return Vec3::ZERO;
        };
        let face = self.face.get(found.face).copied().unwrap_or(Vec3::ZERO);

        let edge1 = triangle.b - triangle.a;
        let edge2 = triangle.c - triangle.a;
        let to_point = found.point - triangle.a;
        let (d00, d01, d11) = (edge1.dot(edge1), edge1.dot(edge2), edge2.dot(edge2));
        let determinant = d00 * d11 - d01 * d01;
        // Cancellation can leave a sliver's determinant at zero or below it, so a face
        // with no area is a sliver too: its seams still have normals where it has none.
        if determinant <= WELL_SHAPED * d00 * d11 {
            return self.on_a_sliver(corners, [triangle.a, triangle.b, triangle.c], point, found);
        }

        let (d20, d21) = (to_point.dot(edge1), to_point.dot(edge2));
        let beta = (d11 * d20 - d01 * d21) / determinant;
        let gamma = (d00 * d21 - d01 * d20) / determinant;
        let alpha = 1.0 - beta - gamma;

        for (weight, corner) in [(alpha, 0), (beta, 1), (gamma, 2)] {
            if weight > 1.0 - ON_THE_SEAM {
                return self.vertex_normal(corners[corner], face);
            }
        }
        for (weight, ends) in [(alpha, (1, 2)), (beta, (2, 0)), (gamma, (0, 1))] {
            if weight < ON_THE_SEAM {
                return self.edge_normal(corners[ends.0], corners[ends.1], face);
            }
        }
        face
    }

    /// [`Self::at`] on a triangle too thin for its barycentrics, which lose the seam to
    /// cancellation there: told apart by where the offset points and which corner or
    /// edge is nearest instead. The face normal on the wrong side of a thin edge would
    /// invert a whole block of the field.
    fn on_a_sliver(
        &self,
        corners: &[u32; 3],
        points: [Vec3; 3],
        point: Vec3,
        found: &ClosestPoint,
    ) -> Vec3 {
        let face = self.face.get(found.face).copied().unwrap_or(Vec3::ZERO);
        // A nearest point inside the face lies straight under `point`.
        let offset = point - found.point;
        if offset.dot(face).abs() >= (1.0 - ON_THE_SEAM) * offset.length() {
            return face;
        }

        let longest = (0..3)
            .map(|corner| (points[(corner + 1) % 3] - points[corner]).length())
            .fold(0.0, Scalar::max);
        let (corner, from_corner) =
            nearest_of((0..3).map(|corner| (corner, (found.point - points[corner]).length())));
        if from_corner <= ON_THE_SEAM * longest {
            return self.vertex_normal(corners[corner], face);
        }

        let (edge, _) = nearest_of((0..3).map(|corner| {
            let (from, to) = (points[corner], points[(corner + 1) % 3]);
            (
                corner,
                (found.point - closest_on_segment(found.point, from, to)).length(),
            )
        }));
        self.edge_normal(corners[edge], corners[(edge + 1) % 3], face)
    }

    fn vertex_normal(&self, vertex: u32, face: Vec3) -> Vec3 {
        self.vertex.get(vertex as usize).copied().unwrap_or(face)
    }

    fn edge_normal(&self, one: u32, other: u32, face: Vec3) -> Vec3 {
        self.edge.get(&seam(one, other)).copied().unwrap_or(face)
    }
}

/// The entry with the smallest distance.
fn nearest_of(entries: impl Iterator<Item = (usize, Scalar)>) -> (usize, Scalar) {
    entries.fold((0, Scalar::INFINITY), |best, entry| {
        if entry.1 < best.1 { entry } else { best }
    })
}

/// The point of the segment from `from` to `to` nearest `point`.
fn closest_on_segment(point: Vec3, from: Vec3, to: Vec3) -> Vec3 {
    let span = to - from;
    let squared = span.length_squared();
    if squared <= 0.0 {
        return from;
    }
    from + span * ((point - from).dot(span) / squared).clamp(0.0, 1.0)
}

/// An edge key that does not care which way round the two faces walk it.
fn seam(one: u32, other: u32) -> (u32, u32) {
    if one <= other {
        (one, other)
    } else {
        (other, one)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Bvh, closest_point};

    /// A cube from the origin to `(size, size, size)`, wound outward.
    fn cube(size: Scalar) -> Mesh {
        let s = size;
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(s, 0.0, 0.0),
                Vec3::new(s, s, 0.0),
                Vec3::new(0.0, s, 0.0),
                Vec3::new(0.0, 0.0, s),
                Vec3::new(s, 0.0, s),
                Vec3::new(s, s, s),
                Vec3::new(0.0, s, s),
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

    fn inside_by(mesh: &Mesh, mode: SignMode, point: Vec3) -> bool {
        let signer = Signer::new(mesh, mode);
        let found = closest_point(mesh, point).expect("the mesh has faces");
        signer.is_inside(mesh, point, &found)
    }

    #[test]
    fn the_pseudonormal_signs_a_closed_cube() {
        let mesh = cube(10.0);
        assert!(inside_by(&mesh, SignMode::Pseudonormal, Vec3::splat(5.0)));
        assert!(!inside_by(
            &mesh,
            SignMode::Pseudonormal,
            Vec3::new(-2.0, 5.0, 5.0)
        ));
    }

    /// A point past a corner is nearest to the corner itself, where only the angle
    /// weighting gives the outward direction the faces disagree about.
    #[test]
    fn a_point_past_a_corner_is_outside() {
        let mesh = cube(10.0);
        assert!(!inside_by(
            &mesh,
            SignMode::Pseudonormal,
            Vec3::new(-1.0, -1.0, -1.0)
        ));
        assert!(!inside_by(
            &mesh,
            SignMode::Pseudonormal,
            Vec3::new(11.0, 11.0, 5.0)
        ));
    }

    #[test]
    fn both_modes_agree_on_a_closed_mesh() {
        let mesh = cube(10.0);
        let bvh = Bvh::build(&mesh);
        for point in [
            Vec3::splat(5.0),
            Vec3::new(0.5, 5.0, 5.0),
            Vec3::new(-0.5, 5.0, 5.0),
            Vec3::new(9.5, 9.5, 9.5),
            Vec3::new(10.5, 5.0, 5.0),
        ] {
            let found = bvh.closest(&mesh, point).expect("the cube has faces");
            let by_normal = Signer::new(&mesh, SignMode::Pseudonormal);
            let by_winding = Signer::new(&mesh, SignMode::Winding);
            assert_eq!(
                by_normal.is_inside(&mesh, point, &found),
                by_winding.is_inside(&mesh, point, &found),
                "the two modes disagree at {point}"
            );
        }
    }

    /// A sliver 24 mm long and a tenth wide lying face down, and a face rising from its
    /// short edge. Its barycentrics lose that edge to cancellation, which signed the point
    /// off it by the sliver's own normal and called it inside.
    #[test]
    fn a_point_off_the_short_edge_of_a_sliver_is_signed_by_the_edge() {
        let mesh = Mesh::new(
            vec![
                Vec3::new(97.21413, 97.78362, 0.0),
                Vec3::new(97.08101, 122.19565, 0.0),
                Vec3::new(97.09066, 121.70752, 0.0),
                Vec3::new(96.0, 122.0, 1.5),
            ],
            vec![[0, 1, 2], [2, 1, 3]],
        );
        let point = Vec3::new(100.0, 122.2, 1.0);
        let found = closest_point(&mesh, point).expect("the mesh has faces");

        assert!(
            !Signer::new(&mesh, SignMode::Pseudonormal).is_inside(&mesh, point, &found),
            "the point stands off the ridge, outside both faces"
        );
    }

    /// A sliver so thin that `f32` puts its determinant below zero, which read as a face
    /// with no area and signed the point by the sliver's own normal.
    #[test]
    fn a_sliver_whose_determinant_cancels_below_zero_is_still_signed_by_its_edge() {
        let mesh = Mesh::new(
            vec![
                Vec3::new(97.21413, 97.78362, 0.0),
                Vec3::new(97.074_936, 122.76993, 0.0),
                Vec3::new(97.078_255, 122.29129, 0.0),
                Vec3::new(96.0, 122.5, 1.5),
            ],
            vec![[0, 1, 2], [2, 1, 3]],
        );
        let point = Vec3::new(100.0, 122.6, 1.0);
        let found = closest_point(&mesh, point).expect("the mesh has faces");

        assert!(
            !Signer::new(&mesh, SignMode::Pseudonormal).is_inside(&mesh, point, &found),
            "the point stands off the ridge, outside both faces"
        );
    }

    #[test]
    fn a_mesh_with_a_hole_falls_back_to_the_winding_number() {
        let mut mesh = cube(10.0);
        mesh.faces.truncate(10);
        assert!(matches!(
            Signer::new(&mesh, SignMode::Auto),
            Signer::Winding(_)
        ));
        assert!(matches!(
            Signer::new(&cube(10.0), SignMode::Auto),
            Signer::Pseudonormal(_)
        ));
    }
}
