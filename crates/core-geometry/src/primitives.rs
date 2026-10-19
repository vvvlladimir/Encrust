use glam::{Vec2, Vec3};

use crate::{FastSet, Mesh, Scalar};

/// A box between two opposite corners, closed and wound outward.
pub fn cuboid(min: Vec3, max: Vec3) -> Mesh {
    let center = (min + max) / 2.0;
    let half = (max - min) / 2.0;
    oriented_box(
        center,
        [Vec3::X * half.x, Vec3::Y * half.y, Vec3::Z * half.z],
    )
}

/// A box standing at `center` along three half-extent vectors, closed and wound outward.
///
/// The three need not be the axes, only square to each other and right-handed.
pub fn oriented_box(center: Vec3, half: [Vec3; 3]) -> Mesh {
    let vertices = (0..8u32)
        .map(|corner| {
            let sign = |bit: u32| if corner & (1 << bit) == 0 { -1.0 } else { 1.0 };
            center + half[0] * sign(0) + half[1] * sign(1) + half[2] * sign(2)
        })
        .collect();
    let quads: [[u32; 4]; 6] = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    let faces = quads
        .iter()
        .flat_map(|[a, b, c, d]| [[*a, *b, *c], [*a, *c, *d]])
        .collect();
    Mesh::new(vertices, faces)
}

/// A convex outline, anticlockwise seen from above, stood upright from `bottom_mm` to
/// `top_mm`.
pub fn prism(outline: &[Vec2], bottom_mm: Scalar, top_mm: Scalar) -> Mesh {
    let fan: Vec<[u32; 3]> = (1..(outline.len() as u32).saturating_sub(1))
        .map(|corner| [0, corner, corner + 1])
        .collect();
    let count = outline.len();
    extrude(
        outline,
        &fan,
        Vec2::extend,
        &vec![bottom_mm; count],
        &vec![top_mm; count],
    )
}

/// A flat region stood up into a slab: `cap`, triangles over `points` wound anticlockwise,
/// at each point's `bottom` and its `top` height, `lift` placing a point at a height, and a
/// wall along every edge only one triangle of the cap walks. Closed and wound outward
/// wherever every top stands above its bottom.
pub fn extrude(
    points: &[Vec2],
    cap: &[[u32; 3]],
    lift: impl Fn(Vec2, Scalar) -> Vec3,
    bottom: &[Scalar],
    top: &[Scalar],
) -> Mesh {
    let count = points.len() as u32;
    let mut vertices: Vec<Vec3> = points
        .iter()
        .zip(bottom)
        .map(|(p, h)| lift(*p, *h))
        .collect();
    vertices.extend(points.iter().zip(top).map(|(p, h)| lift(*p, *h)));
    let sides = |[a, b, c]: [u32; 3]| [(a, b), (b, c), (c, a)];
    let walked: FastSet<(u32, u32)> = cap.iter().flat_map(|face| sides(*face)).collect();
    let mut faces = Vec::with_capacity(4 * cap.len());
    for [a, b, c] in cap {
        faces.push([a + count, b + count, c + count]);
        faces.push([*a, *c, *b]);
    }
    for (from, to) in cap.iter().flat_map(|face| sides(*face)) {
        if !walked.contains(&(to, from)) {
            faces.push([from, to, to + count]);
            faces.push([from, to + count, from + count]);
        }
    }
    Mesh::new(vertices, faces)
}

/// A round bar of `radius` from `from` to `to`, its round side cut into `segments` flats.
pub fn cylinder(from: Vec3, to: Vec3, radius: Scalar, segments: usize) -> Mesh {
    let circle: Vec<Vec2> = (0..segments)
        .map(|step| Vec2::from_angle(std::f32::consts::TAU * step as Scalar / segments as Scalar))
        .collect();
    let length = from.distance(to);
    revolve(from, to - from, &[(0.0, radius), (length, radius)], &circle)
}

/// A solid turned about `axis` from `base`: `outline`, a ring round the origin anticlockwise
/// seen down the axis, set at each `(along, radius)` and scaled by the radius, the rings
/// joined in order and closed at both ends, wound outward. Convex where `outline` is.
pub fn revolve(base: Vec3, axis: Vec3, rings: &[(Scalar, Scalar)], outline: &[Vec2]) -> Mesh {
    let (Some(first), Some(last)) = (rings.first(), rings.last()) else {
        return Mesh::default();
    };
    let axis = axis.normalize_or_zero();
    let side = axis.any_orthonormal_vector();
    let up = axis.cross(side);
    let mut vertices = Vec::with_capacity(rings.len() * outline.len() + 2);
    for (along, radius) in rings {
        let centre = base + axis * *along;
        vertices.extend(
            outline
                .iter()
                .map(|point| centre + (side * point.x + up * point.y) * *radius),
        );
    }
    let (low, high) = (vertices.len() as u32, vertices.len() as u32 + 1);
    vertices.extend([base + axis * first.0, base + axis * last.0]);

    let count = outline.len() as u32;
    let top = (rings.len() as u32 - 1) * count;
    let mut faces = Vec::with_capacity(2 * outline.len() * (rings.len() + 1));
    for ring in 0..rings.len() as u32 - 1 {
        let (this, next_ring) = (ring * count, (ring + 1) * count);
        for step in 0..count {
            let next = (step + 1) % count;
            faces.push([this + step, this + next, next_ring + next]);
            faces.push([this + step, next_ring + next, next_ring + step]);
        }
    }
    for step in 0..count {
        let next = (step + 1) % count;
        faces.push([low, next, step]);
        faces.push([high, top + step, top + next]);
    }
    Mesh::new(vertices, faces)
}

/// A ball of `radius` about `centre`, cut into `rings` bands from pole to pole and
/// `segments` meridians, closed and wound outward.
pub fn ball(centre: Vec3, radius: Scalar, rings: usize, segments: usize) -> Mesh {
    let mut vertices = vec![centre + Vec3::Z * radius];
    for ring in 1..rings {
        let theta = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        let (sin_theta, cos_theta) = theta.sin_cos();
        for segment in 0..segments {
            let phi = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            let (sin_phi, cos_phi) = phi.sin_cos();
            vertices.push(
                centre + radius * Vec3::new(sin_theta * cos_phi, sin_theta * sin_phi, cos_theta),
            );
        }
    }
    let south = vertices.len() as u32;
    vertices.push(centre - Vec3::Z * radius);

    let at = |ring: usize, segment: usize| (1 + (ring - 1) * segments + segment % segments) as u32;
    let mut faces = Vec::with_capacity(2 * segments * rings.saturating_sub(1));
    for segment in 0..segments {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
        faces.push([south, at(rings - 1, segment + 1), at(rings - 1, segment)]);
    }
    for ring in 1..rings.saturating_sub(1) {
        for segment in 0..segments {
            let (upper, lower) = (at(ring, segment), at(ring + 1, segment));
            let (upper_next, lower_next) = (at(ring, segment + 1), at(ring + 1, segment + 1));
            faces.push([upper, lower, lower_next]);
            faces.push([upper, lower_next, upper_next]);
        }
    }
    Mesh::new(vertices, faces)
}

/// The most flats a circle is given, however fine the tolerance asked: a tolerance under
/// f32's precision against the radius would otherwise ask for infinitely many.
const MOST_SEGMENTS: Scalar = 4096.0;

/// How many flats a circle of `radius` needs so that no point of it is further than
/// `tolerance` from them, in the same unit.
pub fn segments_for(radius: Scalar, tolerance: Scalar) -> usize {
    if radius <= tolerance {
        return 3;
    }
    let step = 2.0 * (1.0 - tolerance / radius).acos();
    // A NaN tolerance casts to zero, so the bound is kept after the cast as well.
    ((std::f32::consts::TAU / step)
        .ceil()
        .clamp(3.0, MOST_SEGMENTS) as usize)
        .max(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnose, signed_volume};

    fn closed_with_volume(mesh: &Mesh, expected: Scalar, what: &str) {
        let report = diagnose(mesh);
        assert_eq!(report.boundary_edges, 0, "{what} is open: {report:?}");
        assert_eq!(
            report.unbalanced_edges, 0,
            "{what} is wound unevenly: {report:?}"
        );
        let volume = signed_volume(mesh);
        assert!(
            (volume - expected).abs() <= expected * 1e-3,
            "{what} holds {expected}, wound outward, got {volume}"
        );
    }

    #[test]
    fn a_tolerance_too_fine_to_measure_asks_for_a_bounded_circle() {
        assert_eq!(segments_for(10.0, 0.0), MOST_SEGMENTS as usize);
        assert_eq!(segments_for(1.0e6, 1.0e-3), MOST_SEGMENTS as usize);
    }

    #[test]
    fn a_cuboid_holds_its_sides_multiplied() {
        let mesh = cuboid(Vec3::new(1.0, 2.0, 3.0), Vec3::new(3.0, 5.0, 7.0));
        closed_with_volume(&mesh, 2.0 * 3.0 * 4.0, "a 2 x 3 x 4 box");
    }

    #[test]
    fn an_oriented_box_turned_off_the_axes_keeps_its_volume() {
        let along = Vec3::new(1.0, 1.0, 1.0).normalize();
        let side = along.any_orthonormal_vector();
        let up = along.cross(side);
        let mesh = oriented_box(Vec3::splat(2.0), [along * 3.0, side, up * 0.5]);
        closed_with_volume(&mesh, 6.0 * 2.0 * 1.0, "a 6 x 2 x 1 box on the diagonal");
    }

    #[test]
    fn a_hexagonal_prism_holds_its_area_times_its_height() {
        // A regular hexagon of circumradius 1 covers 3 sqrt(3) / 2.
        let hexagon: Vec<Vec2> = (0..6)
            .map(|step| Vec2::from_angle(std::f32::consts::FRAC_PI_3 * step as Scalar))
            .collect();
        let mesh = prism(&hexagon, -1.0, 2.0);
        let area = 3.0 * (3.0 as Scalar).sqrt() / 2.0;
        closed_with_volume(&mesh, area * 3.0, "a hexagonal prism 3 tall");
    }

    #[test]
    fn a_cylinder_holds_its_polygon_times_its_length() {
        // A regular n-gon of circumradius r covers n r^2 sin(2 pi / n) / 2.
        let (radius, segments) = (1.5, 24);
        let mesh = cylinder(Vec3::ZERO, Vec3::new(0.0, 4.0, 3.0), radius, segments);
        let n = segments as Scalar;
        let area = n * radius * radius * (std::f32::consts::TAU / n).sin() / 2.0;
        closed_with_volume(&mesh, area * 5.0, "a 5 mm bar");
    }

    #[test]
    fn a_finer_tolerance_asks_for_more_flats() {
        assert!(segments_for(1.5, 0.01) > segments_for(1.5, 0.1));
        let coarse = segments_for(10.0, 0.5);
        let step = std::f32::consts::TAU / coarse as Scalar;
        let sag = 10.0 * (1.0 - (step / 2.0).cos());
        assert!(sag <= 0.5, "{coarse} flats sag {sag} mm off a 10 mm circle");
    }
}
