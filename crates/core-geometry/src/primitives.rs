use glam::{Vec2, Vec3};

use crate::{Mesh, Scalar};

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
    let sides = outline.len() as u32;
    let mut vertices: Vec<Vec3> = outline
        .iter()
        .map(|corner| corner.extend(bottom_mm))
        .collect();
    vertices.extend(outline.iter().map(|corner| corner.extend(top_mm)));

    let mut faces = Vec::with_capacity(4 * outline.len());
    for side in 0..sides {
        let next = (side + 1) % sides;
        faces.push([side, next, sides + next]);
        faces.push([side, sides + next, sides + side]);
    }
    for corner in 1..sides.saturating_sub(1) {
        faces.push([0, corner + 1, corner]);
        faces.push([sides, sides + corner, sides + corner + 1]);
    }
    Mesh::new(vertices, faces)
}

/// A round bar of `radius` from `from` to `to`, its round side cut into `segments` flats.
pub fn cylinder(from: Vec3, to: Vec3, radius: Scalar, segments: usize) -> Mesh {
    let axis = (to - from).normalize_or_zero();
    let side = axis.any_orthonormal_vector();
    let up = axis.cross(side);
    let ring = |center: Vec3| {
        (0..segments).map(move |step| {
            let angle = std::f32::consts::TAU * step as Scalar / segments as Scalar;
            center + (side * angle.cos() + up * angle.sin()) * radius
        })
    };
    let mut vertices: Vec<Vec3> = ring(from).chain(ring(to)).collect();
    let (count, low, high) = (
        segments as u32,
        2 * segments as u32,
        2 * segments as u32 + 1,
    );
    vertices.extend([from, to]);

    let mut faces = Vec::with_capacity(4 * segments);
    for step in 0..count {
        let next = (step + 1) % count;
        faces.push([step, next, count + next]);
        faces.push([step, count + next, count + step]);
        faces.push([low, next, step]);
        faces.push([high, count + step, count + next]);
    }
    Mesh::new(vertices, faces)
}

/// How many flats a circle of `radius` needs so that no point of it is further than
/// `tolerance` from them, in the same unit.
pub fn segments_for(radius: Scalar, tolerance: Scalar) -> usize {
    if radius <= tolerance {
        return 3;
    }
    let step = 2.0 * (1.0 - tolerance / radius).acos();
    (std::f32::consts::TAU / step).ceil().max(3.0) as usize
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
