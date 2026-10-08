use crate::triangulate::triangulate;
use crate::{FastMap, Mesh, Scalar, Vec2, Vec3};

/// A vertex within this distance of the plane is on it, in millimetres. Closer than a
/// tenth of a layer, and splitting the edge would give a triangle no rasteriser can see.
const ON_PLANE_MM: Scalar = 1e-4;

/// An infinite plane, as a point on it and a unit normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub point: Vec3,
    /// Unit normal. The half the normal points into is `above`.
    pub normal: Vec3,
}

impl Plane {
    /// A plane at `height_mm` along `normal`, measured from the origin.
    pub fn at(normal: Vec3, height_mm: Scalar) -> Option<Self> {
        let normal = normal.try_normalize()?;
        Some(Self {
            point: normal * height_mm,
            normal,
        })
    }

    fn distance(&self, point: Vec3) -> Scalar {
        (point - self.point).dot(self.normal)
    }

    /// Two axes across the plane, with `u × v` pointing along the normal.
    fn basis(&self) -> (Vec3, Vec3) {
        let u = self.normal.any_orthonormal_vector();
        (u, self.normal.cross(u))
    }
}

/// A mesh cut in two, each half closed over the cut.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cut {
    /// The half on the side the normal points away from.
    pub below: Mesh,
    pub above: Mesh,
    /// Cut loops that did not close, and so were left uncapped. A closed mesh gives none.
    pub open_loops: usize,
}

/// Cuts `mesh` with `plane` and closes both halves over the cut.
///
/// The cut is exact: triangles that straddle the plane are split on it, and the section
/// is triangulated and added as a cap to each half, wound so that each half stays solid.
/// See `docs/design/cutting.md`.
pub fn cut(mesh: &Mesh, plane: Plane) -> Cut {
    let distances: Vec<Scalar> = mesh
        .vertices
        .iter()
        .map(|vertex| plane.distance(*vertex))
        .collect();

    let mut below = Half::default();
    let mut above = Half::default();
    let mut section = Section::default();
    let mut on_plane = OnPlaneEdges::default();

    for face in &mesh.faces {
        let signs = face.map(|index| side(distances[index as usize]));
        // A face lying in the plane is the face the lower half already ends with, so it
        // is tested for first and stays there.
        if signs.iter().all(|sign| *sign <= 0) {
            below.face(mesh, face);
            on_plane.face(face, signs, -1);
            continue;
        }
        if signs.iter().all(|sign| *sign >= 0) {
            above.face(mesh, face);
            on_plane.face(face, signs, 1);
            continue;
        }

        let lower = polygon(mesh, face, signs, &distances, plane, -1, &mut section);
        let upper = polygon(mesh, face, signs, &distances, plane, 1, &mut section);
        below.polygon(mesh, &lower, &section);
        above.polygon(mesh, &upper, &section);
        section.edge(&lower);
    }
    on_plane.into_section(mesh, &mut section);

    let (cap, open_loops) = section.cap(plane);
    below.cap(mesh, &cap, &section, plane.normal);
    above.cap(mesh, &cap, &section, -plane.normal);

    Cut {
        below: below.mesh,
        above: above.mesh,
        open_loops,
    }
}

/// Which side of the plane a distance is, with the band around it counted as on it.
fn side(distance: Scalar) -> i8 {
    if distance > ON_PLANE_MM {
        1
    } else if distance < -ON_PLANE_MM {
        -1
    } else {
        0
    }
}

/// A point of a clipped triangle: either one of the model's own vertices, or a point the
/// cut made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corner {
    Model(u32),
    OnCut(u32),
}

/// The points the cut made, and the segments joining them.
///
/// A point is keyed by the edge it was cut from, with the lower vertex first, so the two
/// triangles sharing that edge produce one point rather than two a hair apart.
#[derive(Debug, Default)]
struct Section {
    points: Vec<Vec3>,
    by_edge: FastMap<(u32, u32), u32>,
    /// The model vertex a point is, for the points the plane passed exactly through.
    from_vertex: FastMap<u32, u32>,
    /// Directed, as the material below the plane runs: what makes the loops wind.
    segments: Vec<(u32, u32)>,
}

impl Section {
    fn on_vertex(&mut self, mesh: &Mesh, index: u32) -> u32 {
        let point = self.point((index, index), mesh.vertices[index as usize]);
        self.from_vertex.insert(point, index);
        point
    }

    /// Which model vertex a point is, when the plane passed exactly through one.
    fn model_of(&self, point: u32) -> Option<u32> {
        self.from_vertex.get(&point).copied()
    }

    fn on_edge(&mut self, mesh: &Mesh, from: u32, to: u32, distances: &[Scalar]) -> u32 {
        let (low, high) = (from.min(to), from.max(to));
        let (a, b) = (mesh.vertices[low as usize], mesh.vertices[high as usize]);
        let (da, db) = (distances[low as usize], distances[high as usize]);
        let t = da / (da - db);
        self.point((low, high), a.lerp(b, t))
    }

    fn point(&mut self, key: (u32, u32), at: Vec3) -> u32 {
        if let Some(found) = self.by_edge.get(&key) {
            return *found;
        }
        let index = self.points.len() as u32;
        self.points.push(at);
        self.by_edge.insert(key, index);
        index
    }

    /// Records the segment this clipped polygon contributes to the section: the two cut
    /// points that are neighbours in it.
    fn edge(&mut self, polygon: &[Corner]) {
        for index in 0..polygon.len() {
            let (Corner::OnCut(from), Corner::OnCut(to)) =
                (polygon[index], polygon[(index + 1) % polygon.len()])
            else {
                continue;
            };
            if from != to {
                self.segments.push((from, to));
            }
        }
    }

    /// The section triangulated, as triangles of cut-point indices, and how many loops
    /// would not close.
    fn cap(&self, plane: Plane) -> (Vec<[u32; 3]>, usize) {
        let (loops, open) = self.loops();
        if loops.is_empty() {
            return (Vec::new(), open);
        }

        let (u, v) = plane.basis();
        let flat: Vec<Vec2> = self
            .points
            .iter()
            .map(|point| Vec2::new(point.dot(u), point.dot(v)))
            .collect();

        let mut rings: Vec<(Vec<u32>, Scalar)> = loops
            .into_iter()
            .map(|ring| {
                let area = signed_area(&ring, &flat);
                (ring, area)
            })
            .collect();
        // The outside of the section winds one way and its holes the other. Which way
        // that is comes from the mesh, so it is read off the largest ring rather than
        // assumed.
        let outward = rings
            .iter()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .is_none_or(|(_, area)| *area >= 0.0);
        rings.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));

        let (outers, holes): (Vec<_>, Vec<_>) = rings
            .into_iter()
            .partition(|(_, area)| (*area >= 0.0) == outward);

        let mut triangles = Vec::new();
        for (outer, _) in &outers {
            let inner: Vec<&Vec<u32>> = holes
                .iter()
                .filter(|(hole, _)| {
                    hole.first()
                        .is_some_and(|point| contains(outer, &flat, flat[*point as usize]))
                })
                .map(|(hole, _)| hole)
                .collect();
            triangles.extend(triangulate(outer, &inner, &flat));
        }
        (triangles, open)
    }

    /// The segments walked into closed rings. A chain that never comes back to where it
    /// started is counted and dropped: a hole in the model is not a cap to be guessed.
    fn loops(&self) -> (Vec<Vec<u32>>, usize) {
        let mut next: FastMap<u32, Vec<u32>> = FastMap::default();
        for (from, to) in &self.segments {
            next.entry(*from).or_default().push(*to);
        }

        let mut rings = Vec::new();
        let mut open = 0;
        while let Some(start) = next.keys().copied().next() {
            let mut ring = vec![start];
            let mut at = start;
            loop {
                let Some(step) = next.get_mut(&at).and_then(Vec::pop) else {
                    open += 1;
                    ring.clear();
                    break;
                };
                if next.get(&at).is_some_and(Vec::is_empty) {
                    next.remove(&at);
                }
                if step == start {
                    break;
                }
                ring.push(step);
                at = step;
            }
            if ring.len() >= 3 {
                rings.push(ring);
            }
        }
        (rings, open)
    }
}

/// Twice the signed area of a ring, positive when it runs counter-clockwise in the
/// plane's own basis.
fn signed_area(ring: &[u32], flat: &[Vec2]) -> Scalar {
    ring.iter()
        .enumerate()
        .map(|(index, point)| {
            let a = flat[*point as usize];
            let b = flat[ring[(index + 1) % ring.len()] as usize];
            a.perp_dot(b)
        })
        .sum()
}

/// Whether a ring encloses a point, by the crossing number.
fn contains(ring: &[u32], flat: &[Vec2], point: Vec2) -> bool {
    let mut inside = false;
    for index in 0..ring.len() {
        let a = flat[ring[index] as usize];
        let b = flat[ring[(index + 1) % ring.len()] as usize];
        if (a.y > point.y) != (b.y > point.y) {
            let crossing = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < crossing {
                inside = !inside;
            }
        }
    }
    inside
}

/// The part of a triangle on one side of the plane, as the corners of a polygon.
fn polygon(
    mesh: &Mesh,
    face: &[u32; 3],
    signs: [i8; 3],
    distances: &[Scalar],
    plane: Plane,
    keep: i8,
    section: &mut Section,
) -> Vec<Corner> {
    let _ = plane;
    let mut corners = Vec::with_capacity(4);
    for index in 0..3 {
        let (from, to) = (index, (index + 1) % 3);
        let (here, next) = (signs[from], signs[to]);

        if here == 0 {
            corners.push(Corner::OnCut(section.on_vertex(mesh, face[from])));
        } else if here == keep {
            corners.push(Corner::Model(face[from]));
        }

        if here != 0 && next != 0 && here != next {
            corners.push(Corner::OnCut(
                section.on_edge(mesh, face[from], face[to], distances),
            ));
        }
    }
    corners
}

/// The edges lying in the plane, and which sides the faces sharing them hold material on.
///
/// No face straddles at such an edge, so the clip never makes a point on it; the section
/// still ends there wherever the material swaps sides across it, which is what a lathed
/// model gives at the ring of vertices around its equator.
#[derive(Debug, Default)]
struct OnPlaneEdges {
    /// Keyed by the edge with the lower vertex first.
    edges: FastMap<(u32, u32), Sides>,
}

/// What has been seen of the two faces sharing one edge in the plane.
#[derive(Debug, Default, Clone, Copy)]
struct Sides {
    /// The edge as the face below it runs it, once that face has been seen.
    below: Option<(u32, u32)>,
    above: bool,
}

impl OnPlaneEdges {
    /// Takes a face that did not straddle, `side` being the side it holds material on.
    /// A face lying in the plane holds material on neither and is the cap itself.
    fn face(&mut self, face: &[u32; 3], signs: [i8; 3], side: i8) {
        if !signs.contains(&side) {
            return;
        }
        for index in 0..3 {
            let next = (index + 1) % 3;
            if signs[index] != 0 || signs[next] != 0 {
                continue;
            }
            let (from, to) = (face[index], face[next]);
            let entry = self.edges.entry((from.min(to), from.max(to))).or_default();
            if side < 0 {
                entry.below = Some((from, to));
            } else {
                entry.above = true;
            }
        }
    }

    /// Hands the section the edges with material on both sides of them, wound as the face
    /// below runs them, which is the direction the clipped faces hand their segments in.
    fn into_section(self, mesh: &Mesh, section: &mut Section) {
        for sides in self.edges.into_values() {
            let Some((from, to)) = sides.below.filter(|_| sides.above) else {
                continue;
            };
            let (from, to) = (section.on_vertex(mesh, from), section.on_vertex(mesh, to));
            section.segments.push((from, to));
        }
    }
}

/// One half of the cut, being built.
#[derive(Debug, Default)]
struct Half {
    mesh: Mesh,
    /// Where each of the model's vertices landed in this half, once it was needed.
    from_model: FastMap<u32, u32>,
    from_cut: FastMap<u32, u32>,
}

impl Half {
    /// Takes a triangle that needed no clipping.
    fn face(&mut self, mesh: &Mesh, face: &[u32; 3]) {
        let corners = face.map(|index| self.model_vertex(mesh, index));
        self.mesh.faces.push(corners);
    }

    /// Takes a clipped polygon, as a fan: it has four corners at most, and is convex.
    fn polygon(&mut self, mesh: &Mesh, corners: &[Corner], section: &Section) {
        if corners.len() < 3 {
            return;
        }
        let vertices: Vec<u32> = corners
            .iter()
            .map(|corner| match corner {
                Corner::Model(index) => self.model_vertex(mesh, *index),
                Corner::OnCut(index) => self.cut_vertex(mesh, section, *index),
            })
            .collect();
        for corner in 1..vertices.len() - 1 {
            self.mesh
                .faces
                .push([vertices[0], vertices[corner], vertices[corner + 1]]);
        }
    }

    /// Closes this half over the cut, with every cap triangle facing `outward`.
    fn cap(&mut self, mesh: &Mesh, triangles: &[[u32; 3]], section: &Section, outward: Vec3) {
        for corners in triangles {
            let points = corners.map(|index| section.points[index as usize]);
            let facing = (points[1] - points[0]).cross(points[2] - points[0]);
            let mut face = corners.map(|index| self.cut_vertex(mesh, section, index));
            if facing.dot(outward) < 0.0 {
                face.swap(1, 2);
            }
            self.mesh.faces.push(face);
        }
    }

    fn model_vertex(&mut self, mesh: &Mesh, index: u32) -> u32 {
        *self.from_model.entry(index).or_insert_with(|| {
            self.mesh.vertices.push(mesh.vertices[index as usize]);
            (self.mesh.vertices.len() - 1) as u32
        })
    }

    fn cut_vertex(&mut self, mesh: &Mesh, section: &Section, index: u32) -> u32 {
        // A point the plane passed through is a vertex this half may already hold: a
        // second copy of it would leave the cap and the wall meeting along nothing.
        if let Some(vertex) = section.model_of(index) {
            return self.model_vertex(mesh, vertex);
        }
        *self.from_cut.entry(index).or_insert_with(|| {
            self.mesh.vertices.push(section.points[index as usize]);
            (self.mesh.vertices.len() - 1) as u32
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnose, signed_volume};

    /// Axis-aligned box, twelve triangles, spanning `min`..`max`.
    fn cuboid(min: Vec3, max: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];
        let faces = vec![
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
        ];
        Mesh::new(vertices, faces)
    }

    fn at_height(height_mm: Scalar) -> Plane {
        Plane::at(Vec3::Z, height_mm).expect("Z is a direction")
    }

    #[test]
    fn a_cube_cut_in_half_gives_two_closed_halves() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let halves = cut(&cube, at_height(4.0));

        assert_eq!(halves.open_loops, 0, "a closed cube cuts into closed loops");
        for (name, half, expected) in [
            ("below", &halves.below, 400.0),
            ("above", &halves.above, 600.0),
        ] {
            let diagnostics = diagnose(half);
            assert!(
                diagnostics.is_closed(),
                "the {name} half is closed, got {diagnostics:?}"
            );
            let volume = signed_volume(half);
            assert!(
                (volume - expected).abs() < 1e-2,
                "the {name} half holds {expected} mm3, got {volume}"
            );
        }
    }

    #[test]
    fn the_two_halves_hold_what_the_whole_held() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let halves = cut(&cube, at_height(2.5));
        let total = signed_volume(&halves.below) + signed_volume(&halves.above);
        assert!((total - signed_volume(&cube)).abs() < 1e-2, "got {total}");
    }

    #[test]
    fn a_cut_through_a_cavity_caps_the_wall_and_not_the_hole() {
        // A hollow box: an outer shell with an inner one wound the other way.
        let mut hollow = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let inner = cuboid(Vec3::splat(2.0), Vec3::splat(8.0));
        let offset = hollow.vertices.len() as u32;
        hollow.vertices.extend(inner.vertices.iter().copied());
        hollow.faces.extend(
            inner
                .faces
                .iter()
                .map(|[a, b, c]| [a + offset, c + offset, b + offset]),
        );

        let halves = cut(&hollow, at_height(5.0));
        assert_eq!(halves.open_loops, 0);

        // 10x10x5 of box less 6x6x3 of cavity: the cap is a frame, not a disc.
        let volume = signed_volume(&halves.below);
        assert!(
            (volume - (500.0 - 108.0)).abs() < 1e-1,
            "expected the wall only, got {volume}"
        );
    }

    #[test]
    fn a_plane_that_misses_the_model_leaves_it_whole() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let halves = cut(&cube, at_height(20.0));

        assert!(halves.above.is_empty(), "nothing is above the plane");
        assert_eq!(halves.below.faces.len(), cube.faces.len());
        assert!((signed_volume(&halves.below) - 1000.0).abs() < 1e-3);
    }

    #[test]
    fn a_plane_along_a_face_cuts_nothing_off() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let halves = cut(&cube, at_height(10.0));
        assert!(halves.above.is_empty(), "the top face is not a half");
        assert!((signed_volume(&halves.below) - 1000.0).abs() < 1e-3);
    }

    #[test]
    fn a_cut_across_the_diagonal_still_closes() {
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let plane = Plane {
            point: Vec3::splat(5.0),
            normal: Vec3::new(1.0, 1.0, 1.0).normalize(),
        };
        let halves = cut(&cube, plane);

        assert_eq!(halves.open_loops, 0);
        assert!(
            diagnose(&halves.below).is_closed(),
            "the low half is closed"
        );
        assert!(
            diagnose(&halves.above).is_closed(),
            "the high half is closed"
        );
        let total = signed_volume(&halves.below) + signed_volume(&halves.above);
        assert!((total - 1000.0).abs() < 1e-1, "got {total}");
    }

    /// Regular octahedron of circumradius `r`, with four of its vertices on z = 0.
    fn octahedron(r: Scalar) -> Mesh {
        let vertices = vec![
            Vec3::new(r, 0.0, 0.0),
            Vec3::new(0.0, r, 0.0),
            Vec3::new(-r, 0.0, 0.0),
            Vec3::new(0.0, -r, 0.0),
            Vec3::new(0.0, 0.0, r),
            Vec3::new(0.0, 0.0, -r),
        ];
        let faces = vec![
            [0, 1, 4],
            [1, 2, 4],
            [2, 3, 4],
            [3, 0, 4],
            [1, 0, 5],
            [2, 1, 5],
            [3, 2, 5],
            [0, 3, 5],
        ];
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_plane_through_a_ring_of_vertices_still_caps_both_halves() {
        // Nothing straddles: every face has an edge on the plane and its third corner
        // off it, which is what a lathed model gives at its equator.
        let halves = cut(&octahedron(10.0), at_height(0.0));

        assert_eq!(halves.open_loops, 0);
        for (name, half) in [("below", &halves.below), ("above", &halves.above)] {
            let diagnostics = diagnose(half);
            assert!(
                diagnostics.is_closed(),
                "the {name} half is closed, got {diagnostics:?}"
            );
            // Each half is a square pyramid of base diagonal 2r and height r:
            // (2r)^2 / 2 * r / 3 = 2 r^3 / 3.
            let volume = signed_volume(half);
            assert!(
                (volume - 2000.0 / 3.0).abs() < 1e-1,
                "the {name} half is half the octahedron, got {volume}"
            );
        }
    }

    #[test]
    fn a_vertex_on_the_plane_is_one_vertex_in_the_half_that_keeps_it() {
        // x + 10z = 100 holds two of the cube's corners exactly and cuts four faces.
        let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let plane = Plane {
            point: Vec3::new(0.0, 0.0, 10.0),
            normal: Vec3::new(1.0, 0.0, 10.0).normalize(),
        };
        let halves = cut(&cube, plane);

        assert_eq!(halves.open_loops, 0);
        for (name, half, expected) in [
            ("below", &halves.below, 950.0),
            ("above", &halves.above, 50.0),
        ] {
            let diagnostics = diagnose(half);
            assert!(
                diagnostics.is_closed(),
                "the {name} half is closed, got {diagnostics:?}"
            );
            // The wedge above is x/10 high over the 10 x 10 top: 50 mm3 of the 1000.
            let volume = signed_volume(half);
            assert!(
                (volume - expected).abs() < 1e-1,
                "the {name} half holds {expected} mm3, got {volume}"
            );
        }
    }

    #[test]
    fn a_plane_with_no_direction_is_not_a_plane() {
        assert!(Plane::at(Vec3::ZERO, 1.0).is_none());
    }
}
