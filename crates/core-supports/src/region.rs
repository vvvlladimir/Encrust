use core_geometry::{
    Adjacency, Bvh, Mesh, Scalar, Transform, Vec3, point_triangle, transform_mesh,
};

use serde::{Deserialize, Serialize};

use crate::placed::Placed;

/// A transform this close to singular has flattened its model, and a point on the plate
/// cannot be mapped back onto its surface.
const SINGULAR: Scalar = 1e-12;

/// A patch of a model's surface, as the set of faces it covers.
///
/// Faces rather than a texture or a point cloud: a click resolves to a face already, the
/// set survives moving and scaling the model, and a fill reads the same triangles the
/// slicer will. See `docs/decisions/0092`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Region {
    words: Vec<u64>,
    marked: usize,
}

impl Region {
    pub fn is_empty(&self) -> bool {
        self.marked == 0
    }

    /// How many faces are marked.
    pub fn len(&self) -> usize {
        self.marked
    }

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

    pub fn clear(&mut self) {
        self.words.clear();
        self.marked = 0;
    }

    /// Marks every face of the model within `radius_mm` of `at`, given in plate
    /// coordinates, or unmarks them when `marked` is false. Returns how many changed.
    ///
    /// The hierarchy's leaves are taken whole, so each candidate is measured against the
    /// brush before it is taken.
    pub fn brush(&mut self, placed: &Placed, at: Vec3, radius_mm: Scalar, marked: bool) -> usize {
        let matrix = placed.transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return 0;
        }
        // The brush is a radius on the plate, and the model's own space is where the
        // faces live, so the radius travels with the smallest scale the model carries.
        let local = matrix.inverse().transform_point3(at);
        let reach_mm = radius_mm
            / placed
                .transform
                .scale
                .abs()
                .min_element()
                .max(Scalar::MIN_POSITIVE);

        let mut found = Vec::new();
        placed.bvh.faces_within(local, reach_mm, &mut found);
        found
            .into_iter()
            .filter(|face| {
                placed
                    .model
                    .triangle(*face as usize)
                    .is_some_and(|triangle| {
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
}

/// A region the user has forbidden supports, as the question every placement asks it.
///
/// Both halves of that question are answered here: a face is blocked, so no tip and no
/// foot may be put on it, and a point on the plate is over blocked surface, so automatic
/// placement puts nothing there. See `docs/decisions/0093`.
#[derive(Debug)]
pub struct Blocked {
    region: Region,
    patch: Mesh,
    bvh: Bvh,
}

impl Blocked {
    /// Builds the question for `region` marked on `model` standing at `transform`, or
    /// `None` when nothing is marked and every placement is free.
    pub fn new(model: &Mesh, region: &Region, transform: Transform) -> Option<Self> {
        let patch = region.submesh(model, transform, 0.0)?;
        Some(Self {
            region: region.clone(),
            bvh: Bvh::build(&patch),
            patch,
        })
    }

    /// Whether `face` of the model is inside the blocked patch.
    pub fn contains_face(&self, face: usize) -> bool {
        self.region.contains(face)
    }

    /// Whether the blocked patch comes within `radius_mm` of `point`, in plate
    /// coordinates: what keeps an automatic contact out of a painted area.
    pub fn covers(&self, point: Vec3, radius_mm: Scalar) -> bool {
        self.bvh
            .closest(&self.patch, point)
            .is_some_and(|closest| closest.distance <= radius_mm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::box_mesh;

    /// The box every test paints on: 10 mm on a side, standing on the plate.
    fn cube() -> Mesh {
        box_mesh(Vec3::ZERO, Vec3::splat(10.0))
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
            &Placed::new(&model, &bvh, Transform::default()),
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

        region.brush(
            &Placed::new(&model, &bvh, moved),
            Vec3::new(55.0, 5.0, 11.0),
            2.0,
            true,
        );

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
    fn nothing_marked_blocks_nothing() {
        assert!(Blocked::new(&cube(), &Region::default(), Transform::default()).is_none());
    }

    #[test]
    fn a_blocked_patch_answers_for_its_faces_and_for_the_plate_over_it() {
        let model = cube();
        let bvh = Bvh::build(&model);
        let mut region = Region::default();
        region.brush(
            &Placed::new(&model, &bvh, Transform::default()),
            Vec3::new(5.0, 5.0, 11.0),
            2.0,
            true,
        );
        let face = region.faces().next().expect("the top was painted");

        let blocked =
            Blocked::new(&model, &region, Transform::default()).expect("a patch was painted");
        assert!(blocked.contains_face(face));
        assert!(!blocked.contains_face(model.faces.len()));
        assert!(
            blocked.covers(Vec3::new(5.0, 5.0, 10.2), 0.5),
            "a contact a fifth of a millimetre under the painted top is covered"
        );
        assert!(
            !blocked.covers(Vec3::new(5.0, 5.0, 2.0), 0.5),
            "a contact halfway down the side is nowhere near the painted top"
        );
    }
}
