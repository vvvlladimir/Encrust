use crate::triangulate::triangulate;
use crate::{FastMap, Mesh, Scalar, Vec2, Vec3, ring_area, ring_contains};

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
    pub fn basis(&self) -> (Vec3, Vec3) {
        axes_across(self.normal)
    }
}

/// Two unit axes square to the unit `normal` and to each other, `u × v` along `normal`.
pub fn axes_across(normal: Vec3) -> (Vec3, Vec3) {
    let u = normal.any_orthonormal_vector();
    (u, normal.cross(u))
}

/// A mesh cut in two, each half closed over the cut.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cut {
    /// The half on the side the normal points away from.
    pub below: Mesh,
    pub above: Mesh,
    /// Cut loops that did not close, and so were left uncapped. A closed mesh gives none.
    pub open_loops: usize,
    /// How many faces each half's cap has: the last this many of each half's faces.
    pub cap_faces: usize,
}

/// Cuts `mesh` with `plane` and closes both halves over the cut, exactly. Straddling triangles are
/// split on the plane and the section is capped on each half; `docs/design/cutting.md`.
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
        // A face lying in the plane is the surface of the half it looks out of, and the
        // section runs round it like round any face of that half.
        if signs == [0; 3] {
            let side = if facing(mesh, face).dot(plane.normal) >= 0.0 {
                -1
            } else {
                1
            };
            if side < 0 {
                below.face(mesh, face);
            } else {
                above.face(mesh, face);
            }
            on_plane.lying(face, side);
            continue;
        }
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
        cap_faces: cap.len(),
    }
}

/// Twice the area of a face, along its normal.
fn facing(mesh: &Mesh, face: &[u32; 3]) -> Vec3 {
    let [a, b, c] = face.map(|index| mesh.vertices[index as usize]);
    (b - a).cross(c - a)
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
                let area = ring_area(ring.iter().map(|point| flat[*point as usize]));
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

        // A hole is in the smallest outer ring round it: the outers run largest first, and
        // a hole in an island in a cavity lies inside the outside wall too.
        let mut inner: Vec<Vec<&Vec<u32>>> = vec![Vec::new(); outers.len()];
        for (hole, _) in &holes {
            let around = hole.first().and_then(|point| {
                (0..outers.len()).rev().find(|at| {
                    let ring = outers[*at].0.iter().map(|corner| flat[*corner as usize]);
                    ring_contains(ring, flat[*point as usize])
                })
            });
            if let Some(at) = around {
                inner[at].push(hole);
            }
        }
        let mut triangles = Vec::new();
        for ((outer, _), inner) in outers.iter().zip(&inner) {
            triangles.extend(triangulate(outer, inner, &flat));
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

/// The edges lying in the plane, and which sides the faces sharing them hold material on. The clip
/// makes no point there, yet the section ends where material swaps sides, as at a lathed equator.
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
            if signs[index] == 0 && signs[next] == 0 {
                self.edge(face[index], face[next], side);
            }
        }
    }

    /// Takes a face lying in the plane, as a face of the half `side` it looks out of.
    fn lying(&mut self, face: &[u32; 3], side: i8) {
        for index in 0..3 {
            self.edge(face[index], face[(index + 1) % 3], side);
        }
    }

    fn edge(&mut self, from: u32, to: u32, side: i8) {
        let entry = self.edges.entry((from.min(to), from.max(to))).or_default();
        if side < 0 {
            entry.below = Some((from, to));
        } else {
            entry.above = true;
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

    /// Closes this half over the cut, the cap facing `outward`. The cap turns over whole by its
    /// area, not per sliver, which would face against its neighbours.
    fn cap(&mut self, mesh: &Mesh, triangles: &[[u32; 3]], section: &Section, outward: Vec3) {
        let facing: Scalar = triangles
            .iter()
            .map(|corners| {
                let points = corners.map(|index| section.points[index as usize]);
                (points[1] - points[0])
                    .cross(points[2] - points[0])
                    .dot(outward)
            })
            .sum();
        for corners in triangles {
            let mut face = corners.map(|index| self.cut_vertex(mesh, section, index));
            if facing < 0.0 {
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
mod tests;
