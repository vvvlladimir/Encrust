use crate::{FastMap, Mesh, Plane, Scalar, Vec2, Vec3};

/// A vertex this near the plane belongs to the cap a cut closed the half with, millimetres.
/// The cut puts its points on the plane to within rounding.
const ON_CAP_MM: Scalar = 1e-3;

/// How far into the cap the spots are looked for, across its widest side. A pin is a few
/// millimetres wide, so forty steps finds room on any cap that has it.
const GRID_STEPS: usize = 40;

/// How much wall a pin keeps between itself and the cap's edge, as a share of its radius.
const EDGE_MARGIN: Scalar = 0.5;

/// Sides of a pin. Within a hundredth of a millimetre of round at four millimetres.
const SIDES: usize = 32;

/// Up to `count` points on the cap `half` was closed with, each at least `radius_mm` and
/// a margin from the cap's edge, spread as far from each other as the cap allows.
///
/// `half` is the side of the cut `plane`'s normal points away from, whose cap faces along
/// it. Fewer points come back where the cap has no room for more, none where it has none.
pub fn pin_spots(half: &Mesh, plane: Plane, radius_mm: Scalar, count: usize) -> Vec<Vec3> {
    let cap = Cap::of(half, plane);
    if cap.triangles.is_empty() || count == 0 {
        return Vec::new();
    }
    let reach = radius_mm * (1.0 + EDGE_MARGIN);
    let (min, max) = cap.bounds();
    let step = (max - min).max_element() / GRID_STEPS as Scalar;
    if step <= 0.0 {
        return Vec::new();
    }
    let mut room = Vec::new();
    let mut y = min.y + step / 2.0;
    while y < max.y {
        let mut x = min.x + step / 2.0;
        while x < max.x {
            let point = Vec2::new(x, y);
            if cap.contains(point) && cap.edge_distance(point) >= reach {
                room.push(point);
            }
            x += step;
        }
        y += step;
    }
    spread(&room, count)
        .into_iter()
        .map(|point| cap.lift(point))
        .collect()
}

/// A closed cylinder of `radius_mm` from `base` along `axis` for `length_mm`, wound outward.
pub fn pin(base: Vec3, axis: Vec3, radius_mm: Scalar, length_mm: Scalar) -> Mesh {
    let axis = axis.normalize_or_zero();
    let (right, up) = axis.any_orthonormal_pair();
    let top = base + axis * length_mm;
    let mut vertices = Vec::with_capacity(2 * SIDES + 2);
    for side in 0..SIDES {
        let angle = std::f32::consts::TAU * side as Scalar / SIDES as Scalar;
        let offset = (right * angle.cos() + up * angle.sin()) * radius_mm;
        vertices.push(base + offset);
        vertices.push(top + offset);
    }
    let (low_cap, high_cap) = (vertices.len() as u32, vertices.len() as u32 + 1);
    vertices.extend([base, top]);

    let mut faces = Vec::with_capacity(4 * SIDES);
    for side in 0..SIDES as u32 {
        let next = (side + 1) % SIDES as u32;
        let (low, high, low_next, high_next) = (2 * side, 2 * side + 1, 2 * next, 2 * next + 1);
        faces.extend([
            [low, low_next, high_next],
            [low, high_next, high],
            [low_cap, low_next, low],
            [high_cap, high, high_next],
        ]);
    }
    Mesh::new(vertices, faces)
}

/// The cap of a cut half, flattened onto the plane: its triangles and the edges round it.
struct Cap {
    origin: Vec3,
    u: Vec3,
    v: Vec3,
    triangles: Vec<[Vec2; 3]>,
    edges: Vec<(Vec2, Vec2)>,
}

impl Cap {
    fn of(half: &Mesh, plane: Plane) -> Self {
        let (u, v) = plane.normal.any_orthonormal_pair();
        let flat = |point: Vec3| {
            let along = point - plane.point;
            Vec2::new(along.dot(u), along.dot(v))
        };
        let on_cap = |index: u32| {
            let point = half.vertices[index as usize];
            ((point - plane.point).dot(plane.normal)).abs() <= ON_CAP_MM
        };
        let mut triangles = Vec::new();
        let mut uses: FastMap<(u32, u32), usize> = FastMap::default();
        for (index, face) in half.faces.iter().enumerate() {
            let facing = half
                .triangle(index)
                .is_some_and(|triangle| triangle.normal_unnormalized().dot(plane.normal) > 0.0);
            if !facing || !face.iter().all(|corner| on_cap(*corner)) {
                continue;
            }
            triangles.push(face.map(|corner| flat(half.vertices[corner as usize])));
            for side in 0..3 {
                let (a, b) = (face[side], face[(side + 1) % 3]);
                *uses.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let edges = uses
            .into_iter()
            .filter(|(_, count)| *count == 1)
            .map(|((a, b), _)| {
                (
                    flat(half.vertices[a as usize]),
                    flat(half.vertices[b as usize]),
                )
            })
            .collect();
        Self {
            origin: plane.point,
            u,
            v,
            triangles,
            edges,
        }
    }

    fn bounds(&self) -> (Vec2, Vec2) {
        let corners = self.triangles.iter().flatten();
        let min = corners
            .clone()
            .fold(Vec2::splat(Scalar::MAX), |a, b| a.min(*b));
        let max = corners.fold(Vec2::splat(Scalar::MIN), |a, b| a.max(*b));
        (min, max)
    }

    fn contains(&self, point: Vec2) -> bool {
        self.triangles.iter().any(|[a, b, c]| {
            let side = |p: Vec2, q: Vec2| (q - p).perp_dot(point - p);
            let (ab, bc, ca) = (side(*a, *b), side(*b, *c), side(*c, *a));
            (ab >= 0.0 && bc >= 0.0 && ca >= 0.0) || (ab <= 0.0 && bc <= 0.0 && ca <= 0.0)
        })
    }

    fn edge_distance(&self, point: Vec2) -> Scalar {
        self.edges
            .iter()
            .map(|(a, b)| {
                let along = *b - *a;
                let t = ((point - *a).dot(along) / along.length_squared().max(Scalar::EPSILON))
                    .clamp(0.0, 1.0);
                (*a + along * t).distance(point)
            })
            .fold(Scalar::MAX, Scalar::min)
    }

    fn lift(&self, point: Vec2) -> Vec3 {
        self.origin + self.u * point.x + self.v * point.y
    }
}

/// `count` of `points`, each as far from those already taken as it can be: the first the
/// one furthest from the middle, so two pins stand at the cap's two ends.
fn spread(points: &[Vec2], count: usize) -> Vec<Vec2> {
    let Some(middle) =
        (!points.is_empty()).then(|| points.iter().copied().sum::<Vec2>() / points.len() as Scalar)
    else {
        return Vec::new();
    };
    let furthest = |from: &dyn Fn(Vec2) -> Scalar| {
        points
            .iter()
            .copied()
            .max_by(|a, b| from(*a).total_cmp(&from(*b)))
    };
    let mut taken: Vec<Vec2> = furthest(&|point| point.distance(middle))
        .into_iter()
        .collect();
    while taken.len() < count.min(points.len()) {
        let gap = |point: Vec2| {
            taken
                .iter()
                .map(|other| other.distance(point))
                .fold(Scalar::MAX, Scalar::min)
        };
        match furthest(&gap) {
            Some(next) if gap(next) > 0.0 => taken.push(next),
            _ => break,
        }
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cut, signed_volume};

    /// Axis-aligned box spanning 0..size, twelve triangles.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
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

    fn lower_half(size: Vec3, height_mm: Scalar) -> (Mesh, Plane) {
        let plane = Plane::at(Vec3::Z, height_mm).expect("+Z is a direction");
        (cut(&cuboid(size), plane).below, plane)
    }

    #[test]
    fn two_pins_stand_at_the_two_ends_of_a_long_cap() {
        let (half, plane) = lower_half(Vec3::new(40.0, 10.0, 20.0), 10.0);
        let spots = pin_spots(&half, plane, 2.0, 2);
        assert_eq!(spots.len(), 2);
        for spot in &spots {
            assert!((spot.z - 10.0).abs() < 1e-3, "on the cut, got {spot}");
            let clear = spot.x.min(40.0 - spot.x).min(spot.y).min(10.0 - spot.y);
            assert!(
                clear >= 3.0 - 1e-3,
                "radius 2 and a 1 mm margin from the edge, got {spot}"
            );
        }
        let apart = (spots[0] - spots[1]).length();
        assert!(
            apart > 25.0,
            "spread along the 40 mm side, {apart} mm apart"
        );
    }

    #[test]
    fn a_cap_narrower_than_a_pin_takes_none() {
        let (half, plane) = lower_half(Vec3::new(40.0, 4.0, 20.0), 10.0);
        assert!(
            pin_spots(&half, plane, 2.0, 2).is_empty(),
            "4 mm cannot hold a 4 mm pin with a margin"
        );
    }

    #[test]
    fn a_pin_is_a_closed_cylinder_of_its_own_volume() {
        let mesh = pin(Vec3::ZERO, Vec3::Z, 2.0, 5.0);
        let volume = signed_volume(&mesh);
        // A 32-gon of circumradius 2 is 32 / 2 * 4 * sin(2pi / 32) mm2, times 5 mm.
        let expected = 16.0 * 4.0 * (std::f32::consts::TAU / 32.0).sin() * 5.0;
        assert!(
            (volume - expected).abs() < 1e-2,
            "wound outward, {volume} against {expected}"
        );
    }
}
