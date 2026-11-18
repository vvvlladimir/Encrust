use crate::{Scalar, Vec3};

/// `count` unit directions spread evenly over the sphere by the Fibonacci lattice, which
/// needs no triangulation and has no pole to clump around.
pub fn sphere_directions(count: usize) -> Vec<Vec3> {
    let golden = std::f32::consts::PI * (3.0 - (5.0 as Scalar).sqrt());
    (0..count)
        .map(|index| {
            let z = 1.0 - 2.0 * (index as Scalar + 0.5) / count as Scalar;
            let radius = (1.0 - z * z).max(0.0).sqrt();
            let angle = golden * index as Scalar;
            Vec3::new(radius * angle.cos(), radius * angle.sin(), z)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lattice_is_unit_directions_no_two_within_four_degrees() {
        let directions = sphere_directions(200);
        assert_eq!(directions.len(), 200);
        assert!(directions.iter().all(|d| (d.length() - 1.0).abs() < 1e-4));
        let apart = 4.0_f32.to_radians().cos();
        for (index, direction) in directions.iter().enumerate() {
            for other in &directions[index + 1..] {
                assert!(direction.dot(*other) <= apart, "{direction} and {other}");
            }
        }
    }

    #[test]
    fn the_lattice_reaches_both_poles_and_balances_about_the_centre() {
        let directions = sphere_directions(64);
        assert!(directions.iter().any(|d| d.z > 0.9));
        assert!(directions.iter().any(|d| d.z < -0.9));
        let mean = directions.iter().sum::<Vec3>() / 64.0;
        assert!(
            mean.length() < 0.02,
            "the mean of an even spread is near zero, got {mean}"
        );
    }
}
