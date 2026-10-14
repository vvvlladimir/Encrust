use core_geometry::{FastMap, Mesh, Scalar, Vec3};
use rayon::prelude::*;

/// How much lower than every neighbour a vertex has to be to be a point of its own,
/// millimetres: a fifth of a layer, under which a flat face tilted by rounding is still flat.
const LOWER_MM: Scalar = 0.01;

/// The surface's vertices and which of them share an edge, by position, so a mesh that
/// repeats a vertex for every triangle is read as the surface it describes.
pub struct LowPoints {
    positions: Vec<Vec3>,
    /// Where each position's neighbours start in `neighbours`, one past the last at the end.
    starts: Vec<u32>,
    neighbours: Vec<u32>,
}

impl LowPoints {
    pub fn of(mesh: &Mesh) -> Self {
        let mut ids: FastMap<[u32; 3], u32> = FastMap::default();
        let mut positions = Vec::new();
        let id_of: Vec<u32> = mesh
            .vertices
            .iter()
            .map(|vertex| {
                *ids.entry(vertex.to_array().map(f32::to_bits))
                    .or_insert_with(|| {
                        positions.push(*vertex);
                        (positions.len() - 1) as u32
                    })
            })
            .collect();

        let corners = |face: &[u32; 3]| face.map(|index| id_of[index as usize]);
        let mut starts = vec![0u32; positions.len() + 1];
        for face in &mesh.faces {
            for id in corners(face) {
                starts[id as usize + 1] += 2;
            }
        }
        for at in 1..starts.len() {
            starts[at] += starts[at - 1];
        }
        let mut filled = starts.clone();
        let mut neighbours = vec![0u32; starts[positions.len()] as usize];
        for face in &mesh.faces {
            let [a, b, c] = corners(face);
            for (id, others) in [(a, [b, c]), (b, [c, a]), (c, [a, b])] {
                let at = &mut filled[id as usize];
                neighbours[*at as usize..*at as usize + 2].copy_from_slice(&others);
                *at += 2;
            }
        }
        Self {
            positions,
            starts,
            neighbours,
        }
    }

    /// Vertices lower than every vertex they share an edge with when `down` points at the
    /// plate: tips, which need a support however little area they have. `floor` is how far
    /// along `down` the plate is, when the model stands on it; a tip there is held by it.
    pub fn count(&self, down: Vec3, floor: Option<Scalar>, band_mm: Scalar) -> usize {
        let depth: Vec<Scalar> = self.positions.par_iter().map(|p| p.dot(down)).collect();
        (0..self.positions.len())
            .into_par_iter()
            .filter(|&id| {
                let around =
                    &self.neighbours[self.starts[id] as usize..self.starts[id + 1] as usize];
                !around.is_empty()
                    && around
                        .iter()
                        .all(|&other| depth[id] > depth[other as usize] + LOWER_MM)
                    && floor.is_none_or(|floor| depth[id] < floor - band_mm)
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square pyramid with its apex at the origin and its base 20 mm above it.
    fn pyramid() -> Mesh {
        Mesh::new(
            vec![
                Vec3::ZERO,
                Vec3::new(-10.0, -10.0, 20.0),
                Vec3::new(10.0, -10.0, 20.0),
                Vec3::new(10.0, 10.0, 20.0),
                Vec3::new(-10.0, 10.0, 20.0),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [0, 4, 3],
                [0, 1, 4],
                [1, 2, 3],
                [1, 3, 4],
            ],
        )
    }

    #[test]
    fn a_pyramid_on_its_apex_has_one_tip_and_on_its_base_none() {
        let points = LowPoints::of(&pyramid());
        assert_eq!(points.count(Vec3::NEG_Z, None, 0.5), 1, "the apex");
        assert_eq!(
            points.count(Vec3::Z, None, 0.5),
            0,
            "a flat base is not a tip"
        );
    }

    #[test]
    fn a_tip_resting_on_the_plate_is_held_by_it() {
        let points = LowPoints::of(&pyramid());
        // Along -Z the apex is at depth 0, and the plate is there too.
        assert_eq!(points.count(Vec3::NEG_Z, Some(0.0), 0.5), 0);
    }

    #[test]
    fn a_triangle_soup_is_read_as_the_surface_it_describes() {
        let welded = pyramid();
        let mut soup = Mesh::default();
        for face in &welded.faces {
            let base = soup.vertices.len() as u32;
            soup.vertices
                .extend(face.map(|index| welded.vertices[index as usize]));
            soup.faces.push([base, base + 1, base + 2]);
        }
        assert_eq!(
            LowPoints::of(&soup).count(Vec3::NEG_Z, None, 0.5),
            1,
            "six copies of the apex are one tip, not six"
        );
    }
}
