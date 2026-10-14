use serde::{Deserialize, Serialize};

use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::topology::{Components, EdgeUse, edge_groups, edge_uses, sort_runs};
use crate::{Mesh, Scalar, Vec3};

/// What is structurally wrong, or right, with a mesh.
///
/// Run [`crate::weld`] first. On an unwelded mesh every edge looks like a boundary and
/// these numbers mean nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshDiagnostics {
    pub vertices: usize,
    pub faces: usize,
    /// Faces with a repeated vertex or zero area.
    pub degenerate_faces: usize,
    /// Faces laid over others on the same three vertices that would have to go: both of a
    /// pair wound opposite ways, the extras of one wound the same way.
    pub duplicate_faces: usize,
    pub unreferenced_vertices: usize,
    /// Edges used by exactly one face: the mesh is open there.
    pub boundary_edges: usize,
    /// Edges used by three or more faces: the surface branches there.
    pub non_manifold_edges: usize,
    /// Edges more faces walk one way than the other: the surface does not close there.
    pub unbalanced_edges: usize,
    /// Connected groups of faces.
    pub shells: usize,
    /// V - E + F over referenced vertices. A closed sphere-like shell gives 2.
    pub euler_characteristic: i64,
}

impl MeshDiagnostics {
    /// A surface that closes a volume: every edge walked as often one way as the other.
    ///
    /// Branching alone does not open a mesh. Two sheets meeting along a seam use its edge
    /// four times, twice each way, and each sheet still has an inside; see
    /// `docs/design/mesh-repair.md`.
    pub fn is_closed(&self) -> bool {
        self.unbalanced_edges == 0
    }

    /// Nothing that would make slicing produce garbage: no hole, no face without area, none
    /// drawn twice. An edge two faces walk the same way is not one: see ADR 0267.
    pub fn is_sound(&self) -> bool {
        self.boundary_edges == 0 && self.degenerate_faces == 0 && self.duplicate_faces == 0
    }
}

/// Inspects the topology of a mesh without modifying it.
pub fn diagnose(mesh: &Mesh) -> MeshDiagnostics {
    let uses = edge_uses(mesh);
    let degenerate = degenerate_faces(mesh);
    let mut components = Components::new(mesh.faces.len());

    let mut boundary_edges = 0;
    let mut non_manifold_edges = 0;
    let mut unbalanced_edges = 0;
    let mut edges: usize = 0;
    for group in edge_groups(&uses) {
        edges += 1;
        match group.len() {
            1 => boundary_edges += 1,
            2 => {}
            _ => non_manifold_edges += 1,
        }
        if !balanced(group, &degenerate) {
            unbalanced_edges += 1;
        }
        for other in &group[1..] {
            components.union(group[0].face, other.face);
        }
    }

    let shells = if mesh.faces.is_empty() {
        0
    } else {
        components.labels().1
    };
    let referenced = referenced_vertices(mesh);

    MeshDiagnostics {
        vertices: mesh.vertices.len(),
        faces: mesh.faces.len(),
        degenerate_faces: degenerate.iter().filter(|face| **face).count(),
        duplicate_faces: count_duplicates(mesh),
        unreferenced_vertices: mesh.vertices.len() - referenced,
        boundary_edges,
        non_manifold_edges,
        unbalanced_edges,
        shells,
        euler_characteristic: count(referenced) - count(edges) + count(mesh.faces.len()),
    }
}

/// Whether one edge's uses pair off, as many faces walking it one way as the other.
///
/// A face with no area has no side to be on, so it counts as whichever direction the edge
/// is short of: a plane cut leaves one along every edge it splits, wound as it falls.
pub(crate) fn balanced(group: &[EdgeUse], degenerate: &[bool]) -> bool {
    let (mut forward, mut backward, mut either) = (0usize, 0usize, 0usize);
    for edge_use in group {
        match (degenerate[edge_use.face as usize], edge_use.forward) {
            (true, _) => either += 1,
            (false, true) => forward += 1,
            (false, false) => backward += 1,
        }
    }
    let gap = forward.abs_diff(backward);
    gap <= either && (either - gap) % 2 == 0
}

/// Six times the volume enclosed by the mesh, positive when faces wind outwards.
///
/// Meaningful only on a closed mesh; on an open one the result depends on where the
/// hole is. Six times, because dividing once at the end keeps the sum exact for longer.
pub fn signed_volume_x6(mesh: &Mesh) -> Scalar {
    // Summed in f64: two million f32 terms drift by most of a percent.
    let sum: f64 = mesh
        .triangles()
        .map(|t| f64::from(t.a.dot(t.b.cross(t.c))))
        .sum();
    sum as Scalar
}

/// Volume enclosed by a closed mesh, cubic millimetres.
pub fn signed_volume(mesh: &Mesh) -> Scalar {
    signed_volume_x6(mesh) / 6.0
}

/// Centre of mass of the solid a closed mesh encloses, in the mesh's own space.
///
/// A mesh that encloses no volume — a sheet, a shell, a flat fan — has no mass to be at
/// the centre of, and gives the centre of its bounding box instead. `None` only for a
/// mesh with no vertices.
pub fn center_of_mass(mesh: &Mesh) -> Option<Vec3> {
    let aabb = mesh.aabb()?;
    let middle = (aabb.mins + aabb.maxs) / 2.0;

    // Moments cancel against each other far from the origin, which f32 does not survive
    // on a model of a million faces.
    let (moment, weight) = mesh.triangles().fold(
        (glam::DVec3::ZERO, 0.0_f64),
        |(moment, weight), triangle| {
            let six_volume = f64::from(triangle.a.dot(triangle.b.cross(triangle.c)));
            let centre = (triangle.a + triangle.b + triangle.c).as_dvec3() / 4.0;
            (moment + centre * six_volume, weight + six_volume)
        },
    );

    // An enclosed volume under a millionth of the bounding box is noise, not a solid.
    let box_volume = f64::from((aabb.maxs - aabb.mins).element_product());
    if weight.abs() / 6.0 <= box_volume * 1e-6 {
        return Some(middle);
    }
    let centre = moment / weight;
    Some(Vec3::new(
        centre.x as Scalar,
        centre.y as Scalar,
        centre.z as Scalar,
    ))
}

/// Widens a count for the Euler arithmetic, which is the only signed value here.
fn count(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn referenced_vertices(mesh: &Mesh) -> usize {
    let mut seen = vec![false; mesh.vertices.len()];
    for face in &mesh.faces {
        for index in face {
            if let Some(slot) = seen.get_mut(*index as usize) {
                *slot = true;
            }
        }
    }
    seen.iter().filter(|s| **s).count()
}

/// Which faces have a repeated vertex or no area, one flag per face.
///
/// A sliver with a tiny but non-zero area is not one: choosing that threshold is a slicing
/// concern, see docs/design/slicing.md when step 2 lands.
pub(crate) fn degenerate_faces(mesh: &Mesh) -> Vec<bool> {
    mesh.faces
        .iter()
        .enumerate()
        .map(|(index, face)| {
            face[0] == face[1]
                || face[1] == face[2]
                || face[0] == face[2]
                || mesh
                    .triangle(index)
                    .is_none_or(|t| t.normal_unnormalized().length_squared() == 0.0)
        })
        .collect()
}

fn count_duplicates(mesh: &Mesh) -> usize {
    duplicate_faces(mesh).len()
}

/// The faces to drop where several cover the same three vertices, ascending.
///
/// Two of them wound opposite ways enclose nothing between them, so each such pair goes
/// whole; of what is left, the first face stays and the rest go. What remains walks every
/// edge as the majority did, so dropping these never unbalances one.
pub(crate) fn duplicate_faces(mesh: &Mesh) -> Vec<usize> {
    let keyed = faces_by_corners(mesh);
    let mut dropped = Vec::new();
    for group in keyed
        .chunk_by(|a, b| a.0 == b.0)
        .filter(|group| group.len() > 1)
    {
        let forward = group.iter().filter(|face| face.1).count();
        let backward = group.len() - forward;
        let keep = match forward.cmp(&backward) {
            std::cmp::Ordering::Greater => group.iter().find(|face| face.1),
            std::cmp::Ordering::Less => group.iter().find(|face| !face.1),
            std::cmp::Ordering::Equal => None,
        }
        .map(|face| face.2);
        dropped.extend(
            group
                .iter()
                .map(|face| face.2)
                .filter(|index| Some(*index) != keep),
        );
    }
    dropped.sort_unstable();
    dropped
}

/// Every face under its corners in ascending order, whether it winds the way that order
/// does, and its index; sorted, so faces over the same corners are adjacent.
///
/// Counted out by lowest corner and sorted only within each run, the same order a full
/// sort gives at linear cost, as `edge_uses` does.
fn faces_by_corners(mesh: &Mesh) -> Vec<([u32; 3], bool, usize)> {
    let keyed: Vec<([u32; 3], bool, usize)> = mesh
        .faces
        .par_iter()
        .enumerate()
        .map(|(index, face)| {
            let mut key = *face;
            key.sort_unstable();
            let ascending = [key, [key[1], key[2], key[0]], [key[2], key[0], key[1]]];
            (key, ascending.contains(face), index)
        })
        .collect();
    let buckets = keyed
        .iter()
        .map(|face| face.0[0] as usize + 1)
        .max()
        .unwrap_or(0);
    let mut starts = vec![0usize; buckets + 1];
    for face in &keyed {
        starts[face.0[0] as usize + 1] += 1;
    }
    for at in 1..starts.len() {
        starts[at] += starts[at - 1];
    }
    let mut sorted = vec![([0u32; 3], false, 0usize); keyed.len()];
    let mut cursor = starts.clone();
    for face in keyed {
        let slot = &mut cursor[face.0[0] as usize];
        sorted[*slot] = face;
        *slot += 1;
    }
    sort_runs(&mut sorted, &starts);
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec3;

    /// A cube of side `side` with every face cut into `n` by `n` squares, wound outward.
    fn tessellated_cube(n: u32, side: Scalar) -> Mesh {
        let mut mesh = Mesh::default();
        let faces = [
            (Vec3::ZERO, Vec3::Y, Vec3::X),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::ZERO, Vec3::X, Vec3::Z),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (Vec3::ZERO, Vec3::Z, Vec3::Y),
            (Vec3::X, Vec3::Y, Vec3::Z),
        ];
        for (origin, u, v) in faces {
            let base = mesh.vertices.len() as u32;
            for j in 0..=n {
                for i in 0..=n {
                    let (s, t) = (i as Scalar / n as Scalar, j as Scalar / n as Scalar);
                    mesh.vertices.push((origin + u * s + v * t) * side);
                }
            }
            for j in 0..n {
                for i in 0..n {
                    let a = base + j * (n + 1) + i;
                    let c = a + n + 1;
                    mesh.faces.push([a, a + 1, c + 1]);
                    mesh.faces.push([a, c + 1, c]);
                }
            }
        }
        mesh
    }

    #[test]
    fn the_volume_of_a_dense_mesh_does_not_drift() {
        // 1.9 million faces, as a detailed resin model has; summed in f32 this came to
        // 1008.8 mm^3.
        let cube = tessellated_cube(400, 10.0);
        let volume = signed_volume(&cube);
        assert!(
            (volume - 1000.0).abs() < 0.1,
            "a 10 mm cube holds 1000 mm^3, got {volume}"
        );
    }

    fn two_triangles() -> Mesh {
        Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)],
            vec![[0, 1, 2], [1, 3, 2]],
        )
    }

    /// An axis-aligned box from `mins` to `maxs`, twelve triangles wound outward.
    fn box_mesh(mins: Vec3, maxs: Vec3) -> Mesh {
        let corner = |x: bool, y: bool, z: bool| {
            Vec3::new(
                if x { maxs.x } else { mins.x },
                if y { maxs.y } else { mins.y },
                if z { maxs.z } else { mins.z },
            )
        };
        let vertices = vec![
            corner(false, false, false),
            corner(true, false, false),
            corner(true, true, false),
            corner(false, true, false),
            corner(false, false, true),
            corner(true, false, true),
            corner(true, true, true),
            corner(false, true, true),
        ];
        Mesh::new(
            vertices,
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

    #[test]
    fn a_box_has_its_centre_of_mass_in_the_middle() {
        let mesh = box_mesh(Vec3::new(10.0, -4.0, 0.0), Vec3::new(14.0, 2.0, 8.0));
        let centre = center_of_mass(&mesh).expect("a box has vertices");
        // A solid box is symmetric about all three midplanes.
        assert!(
            centre.abs_diff_eq(Vec3::new(12.0, -1.0, 4.0), 1e-4),
            "expected the middle of the box, got {centre:?}"
        );
    }

    #[test]
    fn a_tetrahedron_has_its_centre_of_mass_at_the_average_of_its_corners() {
        let mesh = Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(3.0, 0.0, 0.0),
                Vec3::new(0.0, 3.0, 0.0),
                Vec3::new(0.0, 0.0, 3.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let centre = center_of_mass(&mesh).expect("a tetrahedron has vertices");
        // The centroid of a solid tetrahedron is the mean of its four vertices.
        assert!(
            centre.abs_diff_eq(Vec3::splat(0.75), 1e-5),
            "expected (0.75, 0.75, 0.75), got {centre:?}"
        );
    }

    #[test]
    fn a_flat_sheet_falls_back_to_the_middle_of_its_bounding_box() {
        let centre = center_of_mass(&two_triangles()).expect("the sheet has vertices");
        assert!(
            centre.abs_diff_eq(Vec3::new(0.5, 0.5, 0.0), 1e-6),
            "a sheet encloses nothing, so the box decides; got {centre:?}"
        );
    }

    #[test]
    fn a_mesh_with_no_vertices_has_no_centre_of_mass() {
        assert_eq!(center_of_mass(&Mesh::new(Vec::new(), Vec::new())), None);
    }

    #[test]
    fn the_centre_of_mass_is_not_the_middle_of_the_bounding_box() {
        // Two boxes of the same 2 mm side, one at the origin, one eight times further
        // out: mass is shared evenly, so the centroid sits between them, not in the
        // middle of the pair's bounding box.
        let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
        let far = box_mesh(Vec3::new(16.0, 0.0, 0.0), Vec3::new(18.0, 2.0, 2.0));
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend(far.vertices);
        mesh.faces
            .extend(far.faces.iter().map(|f| f.map(|i| i + offset)));

        let centre = center_of_mass(&mesh).expect("two boxes have vertices");
        assert!(
            (centre.x - 9.0).abs() < 1e-3,
            "equal masses at x = 1 and x = 17 average to 9, got {centre:?}"
        );
    }

    #[test]
    fn an_open_sheet_reports_its_boundary() {
        let diagnostics = diagnose(&two_triangles());
        assert_eq!(diagnostics.boundary_edges, 4);
        assert_eq!(diagnostics.non_manifold_edges, 0);
        assert_eq!(diagnostics.shells, 1);
        assert!(!diagnostics.is_closed());
    }

    /// Two boxes meeting along one vertical edge: the four faces at it pair off, two
    /// walking it each way, so each box still has an inside.
    #[test]
    fn two_boxes_sharing_an_edge_branch_there_and_still_close() {
        let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
        let other = box_mesh(Vec3::new(2.0, 2.0, 0.0), Vec3::new(4.0, 4.0, 2.0));
        let offset = mesh.vertices.len() as u32;
        mesh.vertices.extend(other.vertices);
        mesh.faces
            .extend(other.faces.iter().map(|f| f.map(|i| i + offset)));
        let mesh = crate::weld(&mesh, crate::DEFAULT_WELD_TOLERANCE).mesh;

        let diagnostics = diagnose(&mesh);
        assert_eq!(
            diagnostics.non_manifold_edges, 1,
            "the shared edge branches"
        );
        assert_eq!(diagnostics.unbalanced_edges, 0);
        assert!(diagnostics.is_closed(), "two solids, no hole in either");
    }

    /// One face turned round leaves its three edges walked twice the same way, which is
    /// where a surface stops having one inside.
    #[test]
    fn a_face_turned_round_leaves_its_edges_unbalanced() {
        let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
        mesh.faces[0].swap(1, 2);

        let diagnostics = diagnose(&mesh);
        assert_eq!(diagnostics.boundary_edges, 0);
        assert_eq!(diagnostics.non_manifold_edges, 0);
        assert_eq!(diagnostics.unbalanced_edges, 3);
        assert!(!diagnostics.is_closed());
        assert!(
            diagnostics.is_sound(),
            "the fill rule takes what the box encloses, so it slices as it stands"
        );
    }

    #[test]
    fn a_hole_makes_a_mesh_unsound() {
        let mut mesh = box_mesh(Vec3::ZERO, Vec3::splat(2.0));
        mesh.faces.pop();
        assert!(!diagnose(&mesh).is_sound());
    }

    #[test]
    fn a_third_face_on_one_edge_is_non_manifold() {
        let mut mesh = two_triangles();
        mesh.vertices.push(Vec3::new(0.0, 0.0, 1.0));
        mesh.faces.push([1, 2, 4]);
        assert_eq!(diagnose(&mesh).non_manifold_edges, 1);
    }

    #[test]
    fn a_repeated_face_is_counted_once_as_a_duplicate() {
        let mut mesh = two_triangles();
        mesh.faces.push([2, 0, 1]);
        assert_eq!(diagnose(&mesh).duplicate_faces, 1);
    }

    /// Wound opposite ways the two enclose nothing between them, so both are counted.
    #[test]
    fn a_face_laid_back_to_back_on_another_counts_as_a_pair() {
        let mut mesh = two_triangles();
        mesh.faces.push([0, 2, 1]);
        assert_eq!(duplicate_faces(&mesh), vec![0, 2]);
    }

    #[test]
    fn a_collinear_face_is_degenerate() {
        let mesh = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::new(2.0, 0.0, 0.0)],
            vec![[0, 1, 2]],
        );
        assert_eq!(diagnose(&mesh).degenerate_faces, 1);
    }

    #[test]
    fn a_vertex_no_face_uses_is_reported() {
        let mut mesh = two_triangles();
        mesh.vertices.push(Vec3::new(9.0, 9.0, 9.0));
        assert_eq!(diagnose(&mesh).unreferenced_vertices, 1);
    }

    #[test]
    fn an_empty_mesh_has_no_shells() {
        let diagnostics = diagnose(&Mesh::default());
        assert_eq!(diagnostics.shells, 0);
        assert_eq!(diagnostics.euler_characteristic, 0);
    }
}
