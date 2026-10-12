//! Closed solids for the test prints, from primitives that need no boolean operation: parts
//! overlap, and the slicer fills by positive winding, so what overlaps is united.

use core_geometry::{Mesh, Vec2, Vec3};

/// How far a part sinks into what it stands on, mm, so that the two share a volume rather
/// than only a face.
pub const EMBED: f32 = 0.2;

/// A mesh put together from closed parts, each wound outwards.
#[derive(Default)]
pub struct Solid {
    vertices: Vec<Vec3>,
    faces: Vec<[u32; 3]>,
}

impl Solid {
    pub fn into_mesh(self) -> Mesh {
        Mesh::new(self.vertices, self.faces)
    }

    /// A box from `min` to `max`, mm.
    pub fn cuboid(&mut self, min: Vec3, max: Vec3) {
        let outline = [
            Vec2::new(min.x, min.y),
            Vec2::new(max.x, min.y),
            Vec2::new(max.x, max.y),
            Vec2::new(min.x, max.y),
        ];
        self.prism(&outline, min.z, max.z);
    }

    /// A simple polygon, counter-clockwise seen from above, extruded from `bottom` to `top`.
    pub fn prism(&mut self, outline: &[Vec2], bottom: f32, top: f32) {
        let (lower, upper) = self.rings(outline, bottom, top);
        for [a, b, c] in ear_clip(outline) {
            self.faces.push([lower + a, lower + c, lower + b]);
            self.faces.push([upper + a, upper + b, upper + c]);
        }
        self.wall(lower, upper, outline.len() as u32, false);
    }

    /// A polygon counter-clockwise in (x, z), extruded along Y from `y0` to `y1`.
    pub fn side_prism(&mut self, outline: &[Vec2], y0: f32, y1: f32) {
        let start = self.vertices.len();
        self.prism(outline, -y1, -y0);
        // A quarter turn about X, which keeps every face wound outwards.
        for vertex in &mut self.vertices[start..] {
            *vertex = Vec3::new(vertex.x, -vertex.z, vertex.y);
        }
    }

    /// The ring between `outer` and `inner`, counter-clockwise from above, extruded from
    /// `bottom` to `top`. The i-th point of one faces the i-th of the other.
    pub fn tube(&mut self, outer: &[Vec2], inner: &[Vec2], bottom: f32, top: f32) {
        let n = outer.len() as u32;
        let (outer_low, outer_high) = self.rings(outer, bottom, top);
        let (inner_low, inner_high) = self.rings(inner, bottom, top);
        self.wall(outer_low, outer_high, n, false);
        self.wall(inner_low, inner_high, n, true);
        for i in 0..n {
            let j = (i + 1) % n;
            let (oh, ih) = (outer_high, inner_high);
            self.faces.push([oh + i, oh + j, ih + j]);
            self.faces.push([oh + i, ih + j, ih + i]);
            let (ol, il) = (outer_low, inner_low);
            self.faces.push([ol + i, il + j, ol + j]);
            self.faces.push([ol + i, il + i, il + j]);
        }
    }

    /// A closed profile in (radius, z), counter-clockwise, turned about the vertical
    /// through `centre` in `segments` steps. A point at radius zero stays one vertex.
    pub fn revolve(&mut self, centre: Vec2, profile: &[Vec2], segments: u32) {
        let rings: Vec<Vec<u32>> = profile
            .iter()
            .map(|point| self.ring(centre, *point, segments))
            .collect();
        for (k, a) in rings.iter().enumerate() {
            let b = &rings[(k + 1) % rings.len()];
            for i in 0..segments as usize {
                let j = (i + 1) % segments as usize;
                let (ai, aj) = (a[i % a.len()], a[j % a.len()]);
                let (bi, bj) = (b[i % b.len()], b[j % b.len()]);
                if ai != aj {
                    self.faces.push([ai, aj, bj]);
                }
                if bi != bj {
                    self.faces.push([ai, bj, bi]);
                }
            }
        }
    }

    /// The vertices of one profile point turned round, or the one vertex it is on the axis.
    fn ring(&mut self, centre: Vec2, point: Vec2, segments: u32) -> Vec<u32> {
        let start = self.vertices.len() as u32;
        // An arc computed down to the axis lands a hair off it; that is still the axis.
        if point.x <= 1e-4 {
            self.vertices.push(centre.extend(point.y));
            return vec![start];
        }
        for step in 0..segments {
            let around = Vec2::from_angle(std::f32::consts::TAU * step as f32 / segments as f32);
            self.vertices
                .push((centre + around * point.x).extend(point.y));
        }
        (start..start + segments).collect()
    }

    /// The outline at `bottom` and again at `top`, answering where each copy starts.
    fn rings(&mut self, outline: &[Vec2], bottom: f32, top: f32) -> (u32, u32) {
        let lower = self.vertices.len() as u32;
        for z in [bottom, top] {
            self.vertices
                .extend(outline.iter().map(|point| point.extend(z)));
        }
        (lower, lower + outline.len() as u32)
    }

    /// The side between a loop of `n` vertices and the copy above it, facing out of the
    /// loop, or into it where the loop is a hole.
    fn wall(&mut self, lower: u32, upper: u32, n: u32, hole: bool) {
        for i in 0..n {
            let j = (i + 1) % n;
            let quad = [lower + i, lower + j, upper + j, upper + i];
            let [a, b, c, d] = if hole {
                [quad[0], quad[3], quad[2], quad[1]]
            } else {
                quad
            };
            self.faces.push([a, b, c]);
            self.faces.push([a, c, d]);
        }
    }
}

/// The rectangle from `min` to `max` and the round hole through it, matched point for point
/// for [`Solid::tube`]: both are sampled at the same angles about the hole's centre, the
/// rectangle's corners among them.
pub fn holed_rectangle(
    min: Vec2,
    max: Vec2,
    centre: Vec2,
    radius: f32,
    segments: u32,
) -> (Vec<Vec2>, Vec<Vec2>) {
    let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
    let mut angles: Vec<f32> = (0..segments)
        .map(|step| std::f32::consts::TAU * step as f32 / segments as f32)
        .chain(corners.iter().map(|corner| {
            let to = *corner - centre;
            to.y.atan2(to.x).rem_euclid(std::f32::consts::TAU)
        }))
        .collect();
    angles.sort_by(f32::total_cmp);
    angles.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    let outer = angles
        .iter()
        .map(|angle| {
            let along = Vec2::from_angle(*angle);
            let reach = |low: f32, high: f32, from: f32, step: f32| match step {
                s if s > 1e-6 => (high - from) / s,
                s if s < -1e-6 => (low - from) / s,
                _ => f32::INFINITY,
            };
            let t =
                reach(min.x, max.x, centre.x, along.x).min(reach(min.y, max.y, centre.y, along.y));
            centre + along * t
        })
        .collect();
    let inner = angles
        .iter()
        .map(|angle| centre + Vec2::from_angle(*angle) * radius)
        .collect();
    (outer, inner)
}

/// A straight rod's profile for [`Solid::revolve`], `radius` mm, from `bottom` to `top`.
pub fn rod(radius: f32, bottom: f32, top: f32) -> [Vec2; 4] {
    [
        Vec2::new(0.0, bottom),
        Vec2::new(radius, bottom),
        Vec2::new(radius, top),
        Vec2::new(0.0, top),
    ]
}

/// The mesh as a binary STL, the form a dropped file arrives in.
pub fn stl(mesh: &Mesh) -> Vec<u8> {
    let mut bytes = vec![0; 80];
    bytes.extend_from_slice(&(mesh.faces.len() as u32).to_le_bytes());
    for face in &mesh.faces {
        // Readers take the winding, not the normal, so the normal is left zero.
        bytes.extend_from_slice(&[0; 12]);
        for index in face {
            let vertex = mesh.vertices[*index as usize];
            for coordinate in vertex.to_array() {
                bytes.extend_from_slice(&coordinate.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0; 2]);
    }
    bytes
}

/// Triangles covering a simple polygon wound counter-clockwise, as indices into it.
fn ear_clip(outline: &[Vec2]) -> Vec<[u32; 3]> {
    let mut left: Vec<u32> = (0..outline.len() as u32).collect();
    let mut triangles = Vec::with_capacity(outline.len().saturating_sub(2));
    while left.len() > 3 {
        let n = left.len();
        let corner = |k: usize| [left[(k + n - 1) % n], left[k], left[(k + 1) % n]];
        let Some(ear) = (0..n).find(|&k| is_ear(outline, &left, corner(k))) else {
            break;
        };
        triangles.push(corner(ear));
        left.remove(ear);
    }
    if let [a, b, c] = left[..] {
        triangles.push([a, b, c]);
    }
    triangles
}

/// Whether the corner turns left and no other vertex of the polygon lies within it.
fn is_ear(outline: &[Vec2], left: &[u32], [a, b, c]: [u32; 3]) -> bool {
    let at = |index: u32| outline[index as usize];
    let turn = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
    if turn(at(a), at(b), at(c)) <= 0.0 {
        return false;
    }
    left.iter().filter(|p| ![a, b, c].contains(p)).all(|&p| {
        let point = at(p);
        turn(at(a), at(b), point) < 0.0
            || turn(at(b), at(c), point) < 0.0
            || turn(at(c), at(a), point) < 0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{diagnose, signed_volume};

    fn solid(build: impl FnOnce(&mut Solid)) -> Mesh {
        let mut solid = Solid::default();
        build(&mut solid);
        solid.into_mesh()
    }

    #[test]
    fn a_cuboid_is_closed_and_holds_its_volume() {
        let mesh = solid(|s| s.cuboid(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0)));
        assert!(diagnose(&mesh).is_sound());
        let volume = signed_volume(&mesh);
        assert!((volume - 24.0).abs() < 1e-4, "2 × 3 × 4 mm, got {volume}");
    }

    #[test]
    fn a_concave_prism_is_closed_and_holds_its_area_times_height() {
        // An L of three unit squares.
        let outline = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ]
        .map(|(x, y)| Vec2::new(x, y));
        let mesh = solid(|s| s.prism(&outline, 0.0, 2.0));
        assert!(diagnose(&mesh).is_sound());
        let volume = signed_volume(&mesh);
        assert!((volume - 6.0).abs() < 1e-4, "3 mm² × 2 mm, got {volume}");
    }

    #[test]
    fn a_side_prism_stays_wound_outwards() {
        let outline = [(0.0, 0.0), (3.0, 0.0), (0.0, 2.0)].map(|(x, y)| Vec2::new(x, y));
        let mesh = solid(|s| s.side_prism(&outline, 1.0, 5.0));
        let volume = signed_volume(&mesh);
        assert!((volume - 12.0).abs() < 1e-4, "3 mm² × 4 mm, got {volume}");
    }

    #[test]
    fn a_holed_rectangle_loses_its_hole() {
        let (outer, inner) = holed_rectangle(
            Vec2::ZERO,
            Vec2::new(10.0, 6.0),
            Vec2::new(3.0, 3.0),
            1.0,
            256,
        );
        let mesh = solid(|s| s.tube(&outer, &inner, 0.0, 1.0));
        assert!(diagnose(&mesh).is_sound());
        let expected = 60.0 - std::f32::consts::PI;
        let volume = signed_volume(&mesh);
        assert!(
            (volume - expected).abs() < 1e-2,
            "60 mm² less a unit circle, got {volume}"
        );
    }

    #[test]
    fn a_revolved_rod_is_a_cylinder() {
        let mesh = solid(|s| s.revolve(Vec2::ZERO, &rod(1.0, 0.0, 2.0), 512));
        assert!(diagnose(&mesh).is_sound());
        let expected = 2.0 * std::f32::consts::PI;
        let volume = signed_volume(&mesh);
        assert!(
            (volume - expected).abs() < 1e-3,
            "π r² h for r 1, h 2, got {volume}"
        );
    }
}
