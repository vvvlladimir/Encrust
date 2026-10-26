use serde::{Deserialize, Serialize};

use crate::{
    Adjacency, Bvh, Components, FastMap, Mesh, Scalar, Transform, Vec3, point_triangle,
    transform_mesh,
};

/// A patch of a model's surface, as the set of faces it covers. Faces survive moving and scaling
/// and match what the slicer reads (ADR 0092, ADR 0318).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "Stored")]
pub struct Region {
    words: Vec<u64>,
    marked: usize,
}

/// A region as a file holds it, its count taken from the bits rather than trusted.
#[derive(Deserialize)]
struct Stored {
    words: Vec<u64>,
}

impl From<Stored> for Region {
    fn from(stored: Stored) -> Self {
        let marked = stored
            .words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum();
        Self {
            words: stored.words,
            marked,
        }
    }
}

impl FromIterator<usize> for Region {
    fn from_iter<I: IntoIterator<Item = usize>>(faces: I) -> Self {
        let mut region = Self::default();
        for face in faces {
            region.set(face, true);
        }
        region
    }
}

impl Region {
    /// Whether no face is marked.
    pub fn is_empty(&self) -> bool {
        self.marked == 0
    }

    /// How many faces are marked.
    pub fn len(&self) -> usize {
        self.marked
    }

    /// Whether `face` is marked.
    pub fn contains(&self, face: usize) -> bool {
        self.words
            .get(face / 64)
            .is_some_and(|word| word & (1 << (face % 64)) != 0)
    }

    /// Marks or unmarks one face. Returns whether anything changed.
    pub fn set(&mut self, face: usize, marked: bool) -> bool {
        if marked == self.contains(face) {
            return false;
        }
        if marked && face / 64 >= self.words.len() {
            self.words.resize(face / 64 + 1, 0);
        }
        self.words[face / 64] ^= 1 << (face % 64);
        self.marked = if marked {
            self.marked + 1
        } else {
            self.marked - 1
        };
        true
    }

    /// The marked faces, in index order.
    pub fn faces(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(at, word)| {
            (0..64)
                .filter(move |bit| word & (1 << bit) != 0)
                .map(move |bit| at * 64 + bit)
        })
    }

    /// Unmarks every face.
    pub fn clear(&mut self) {
        self.words.clear();
        self.marked = 0;
    }

    /// Marks (or unmarks) every face of `model` at `transform` within `radius_mm` of `at`, in plate
    /// space. Returns the count changed; `bvh` leaves are taken whole then measured.
    pub fn brush(
        &mut self,
        (model, bvh, transform): (&Mesh, &Bvh, Transform),
        at: Vec3,
        radius_mm: Scalar,
        marked: bool,
    ) -> usize {
        let Some(inverse) = transform.inverse_matrix() else {
            return 0;
        };
        // The brush is a radius on the plate, and the model's own space is where the
        // faces live, so the radius travels with the least scale the model carries.
        let local = inverse.transform_point3(at);
        let reach_mm = radius_mm / transform.least_scale();

        let mut found = Vec::new();
        bvh.faces_within(local, reach_mm, &mut found);
        found
            .into_iter()
            .filter(|face| {
                model.triangle(*face as usize).is_some_and(|triangle| {
                    point_triangle(local, &triangle).distance(local) <= reach_mm
                })
            })
            .filter(|face| self.set(*face as usize, marked))
            .count()
    }

    /// Marks every face reachable from `seed` across shared edges whose normal stays
    /// within `max_angle_deg` of the seed's, or unmarks them. Returns how many changed.
    ///
    /// This is what filling a picked face means: a flat panel comes whole, a curved one
    /// stops where it has turned too far to be the same surface.
    pub fn flood(
        &mut self,
        model: &Mesh,
        adjacency: &Adjacency,
        seed: usize,
        max_angle_deg: Scalar,
        marked: bool,
    ) -> usize {
        let Some(normal) = model
            .triangle(seed)
            .map(|triangle| triangle.normal_unnormalized().normalize_or_zero())
            .filter(|normal| *normal != Vec3::ZERO)
        else {
            return 0;
        };
        let limit = max_angle_deg.to_radians().cos();

        let mut changed = 0;
        let mut seen = Region::default();
        let mut queue = vec![seed];
        seen.set(seed, true);
        while let Some(face) = queue.pop() {
            let within = model.triangle(face).is_some_and(|triangle| {
                triangle
                    .normal_unnormalized()
                    .normalize_or_zero()
                    .dot(normal)
                    >= limit
            });
            if !within {
                continue;
            }
            changed += usize::from(self.set(face, marked));
            for next in adjacency.neighbours(face) {
                if seen.set(*next as usize, true) {
                    queue.push(*next as usize);
                }
            }
        }
        changed
    }

    /// The marked faces as a mesh of their own, in plate coordinates, or `None` when
    /// nothing is marked.
    ///
    /// `lift_mm` stands each triangle off along its own normal, which is what keeps a
    /// patch drawn over the model from fighting the surface it lies on.
    pub fn submesh(&self, model: &Mesh, transform: Transform, lift_mm: Scalar) -> Option<Mesh> {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for face in self.faces() {
            let Some(corners) = model.faces.get(face) else {
                continue;
            };
            let at = vertices.len() as u32;
            vertices.extend(corners.map(|corner| model.vertices[corner as usize]));
            faces.push([at, at + 1, at + 2]);
        }
        if faces.is_empty() {
            return None;
        }

        let mut patch = transform_mesh(&Mesh::new(vertices, faces), transform);
        if lift_mm != 0.0 {
            for face in 0..patch.faces.len() {
                let Some(normal) = patch
                    .triangle(face)
                    .map(|triangle| triangle.normal_unnormalized().normalize_or_zero())
                else {
                    continue;
                };
                for corner in patch.faces[face] {
                    patch.vertices[corner as usize] += normal * lift_mm;
                }
            }
        }
        Some(patch)
    }

    /// The marked faces of `model` in pieces, two faces in one piece when they share a
    /// vertex; the pieces in the order of their first face.
    pub fn parts(&self, model: &Mesh) -> Vec<Vec<usize>> {
        let mut joined = Components::new(model.vertices.len());
        let marked: Vec<usize> = self
            .faces()
            .filter(|&face| face < model.faces.len())
            .collect();
        for &face in &marked {
            let [a, b, c] = model.faces[face];
            joined.union(a, b);
            joined.union(b, c);
        }
        let mut part_of: FastMap<u32, usize> = FastMap::default();
        let mut parts: Vec<Vec<usize>> = Vec::new();
        for face in marked {
            let at = *part_of
                .entry(joined.find(model.faces[face][0]))
                .or_insert_with(|| {
                    parts.push(Vec::new());
                    parts.len() - 1
                });
            parts[at].push(face);
        }
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cuboid;

    /// The box every test paints on: 10 mm on a side, standing on the plate.
    fn cube() -> Mesh {
        cuboid(Vec3::ZERO, Vec3::splat(10.0))
    }

    #[test]
    fn a_fresh_region_holds_nothing() {
        let region = Region::default();
        assert!(region.is_empty());
        assert!(!region.contains(0));
        assert_eq!(region.faces().count(), 0);
    }

    #[test]
    fn marking_a_face_twice_changes_it_once() {
        let mut region = Region::default();
        assert!(region.set(70, true));
        assert!(!region.set(70, true));
        assert_eq!(region.len(), 1);
        assert_eq!(region.faces().collect::<Vec<_>>(), vec![70]);

        assert!(region.set(70, false));
        assert!(region.is_empty());
    }

    #[test]
    fn a_brush_marks_only_what_it_reaches() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let mut region = Region::default();

        // A millimetre above the middle of the top face, with a brush too small to reach
        // any of the four walls.
        let marked = region.brush(
            (&model, &bvh, Transform::default()),
            Vec3::new(5.0, 5.0, 11.0),
            2.0,
            true,
        );

        assert_eq!(marked, region.len());
        assert!(!region.is_empty(), "the top face is within 2 mm");
        for face in region.faces() {
            let triangle = model.triangle(face).expect("a marked face exists");
            assert!(
                triangle.normal_unnormalized().normalize().z > 0.9,
                "only the top of the cube is within reach of the brush"
            );
        }
    }

    #[test]
    fn a_brush_travels_with_the_model_it_paints() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let moved = Transform::from_translation(Vec3::new(50.0, 0.0, 0.0));
        let mut region = Region::default();

        region.brush((&model, &bvh, moved), Vec3::new(55.0, 5.0, 11.0), 2.0, true);

        assert!(!region.is_empty(), "the brush was aimed at the moved model");
    }

    #[test]
    fn a_flood_stops_at_the_edge_it_turns_over() {
        let model = cube();
        let adjacency = Adjacency::of(&model);
        let top = (0..model.faces.len())
            .find(|face| {
                model
                    .triangle(*face)
                    .expect("in range")
                    .normal_unnormalized()
                    .normalize()
                    .z
                    > 0.9
            })
            .expect("a cube has a top");

        let mut region = Region::default();
        region.flood(&model, &adjacency, top, 30.0, true);

        assert_eq!(region.len(), 2, "the top of a cube is two triangles");
    }

    #[test]
    fn a_flood_over_a_right_angle_stops_at_the_face_that_has_turned_right_round() {
        let model = cube();
        let adjacency = Adjacency::of(&model);
        let mut region = Region::default();
        region.flood(&model, &adjacency, 0, 95.0, true);

        assert_eq!(
            region.len(),
            model.faces.len() - 2,
            "the bottom and the four walls are within 95 degrees of the bottom; the top is 180 from it"
        );
    }

    #[test]
    fn faces_of_two_cubes_far_apart_come_in_two_parts() {
        let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE);
        mesh.append(&cuboid(Vec3::splat(5.0), Vec3::splat(6.0)));
        let mut region = Region::default();
        for face in [0, 1, 12, 13] {
            region.set(face, true);
        }

        assert_eq!(region.parts(&mesh), vec![vec![0, 1], vec![12, 13]]);
    }
}
