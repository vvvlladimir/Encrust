use core_geometry::{FastMap, Mesh, Scalar, Vec3};

/// How near two directions have to be before they are the same candidate, in degrees.
/// Under this, two orientations differ by less than the wall of one layer.
const SAME_DEG: Scalar = 4.0;

/// How finely face normals are binned before their clusters are ranked by area. Sixteen
/// bins an axis puts a cube's six faces in six bins and keeps a sphere from filling
/// memory with one bin a triangle.
const BINS: Scalar = 16.0;

/// Flat faces kept as candidates. Past this, a cluster is a facet of a curve rather than
/// a face a model would rest on.
const FLATS: usize = 24;

/// Directions worth trying as "down", in the model's own space.
///
/// A model rests on its own flat faces, so the largest of them come first and a candidate
/// from the sphere that lands on one is dropped rather than the other way round. The
/// sphere is what covers a model that has no flat face at all; see
/// `docs/design/orientation.md`.
pub fn directions(mesh: &Mesh, samples: usize) -> Vec<Vec3> {
    let mut chosen: Vec<Vec3> = Vec::with_capacity(FLATS + samples);
    for direction in flats(mesh).into_iter().chain(sphere(samples)) {
        if chosen.iter().all(|kept| !is_same(*kept, direction)) {
            chosen.push(direction);
        }
    }
    chosen
}

/// Two directions a print cannot tell apart.
fn is_same(left: Vec3, right: Vec3) -> bool {
    left.dot(right) > SAME_DEG.to_radians().cos()
}

/// The outward normals of the model's flattest, largest surfaces, biggest first.
///
/// Normals are binned rather than clustered properly: a bin is a hash lookup a face,
/// where clustering would be a comparison against every cluster found so far.
fn flats(mesh: &Mesh) -> Vec<Vec3> {
    let mut bins: FastMap<[i32; 3], (Vec3, Scalar)> = FastMap::default();
    for triangle in mesh.triangles() {
        let normal = triangle.normal_unnormalized();
        let area = normal.length() / 2.0;
        if area <= Scalar::EPSILON {
            continue;
        }
        let unit = normal / (area * 2.0);
        let key = [
            (unit.x * BINS).round() as i32,
            (unit.y * BINS).round() as i32,
            (unit.z * BINS).round() as i32,
        ];
        let bin = bins.entry(key).or_insert((Vec3::ZERO, 0.0));
        bin.0 += unit * area;
        bin.1 += area;
    }

    let mut ranked: Vec<(Vec3, Scalar)> = bins.into_values().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked
        .into_iter()
        .take(FLATS)
        .filter_map(|(sum, _)| sum.try_normalize())
        .collect()
}

/// `count` directions spread evenly over the sphere by the Fibonacci lattice, which needs
/// no triangulation and has no pole to clump around.
fn sphere(count: usize) -> Vec<Vec3> {
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

    /// Axis-aligned cube spanning 0..1 on every axis, twelve triangles.
    fn unit_cube() -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
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

    #[test]
    fn a_cube_offers_its_own_six_faces_exactly() {
        let flats = flats(&unit_cube());
        assert_eq!(flats.len(), 6, "a cube has six distinct normals");
        for axis in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, -Vec3::Y, -Vec3::Z] {
            assert!(
                flats.iter().any(|normal| normal.abs_diff_eq(axis, 1e-5)),
                "the {axis} face is missing"
            );
        }
    }

    #[test]
    fn the_sphere_is_spread_and_on_the_unit_sphere() {
        let directions = sphere(200);
        assert_eq!(directions.len(), 200);
        for direction in &directions {
            assert!((direction.length() - 1.0).abs() < 1e-4);
        }

        // No two samples of a 200-point lattice land within 4 degrees of each other.
        for (index, direction) in directions.iter().enumerate() {
            for other in &directions[index + 1..] {
                assert!(!is_same(*direction, *other), "{direction} and {other}");
            }
        }
    }

    #[test]
    fn a_flat_face_wins_over_the_sphere_sample_beside_it() {
        let directions = directions(&unit_cube(), 200);
        assert!(
            directions
                .iter()
                .any(|direction| direction.abs_diff_eq(Vec3::NEG_Z, 1e-5)),
            "the face the cube would stand on is kept exactly"
        );
        assert!(
            directions
                .iter()
                .filter(|direction| is_same(**direction, Vec3::NEG_Z))
                .count()
                == 1,
            "and nothing beside it survives to be tried twice"
        );
    }

    #[test]
    fn an_empty_mesh_offers_only_the_sphere() {
        assert_eq!(directions(&Mesh::default(), 12).len(), 12);
    }
}
