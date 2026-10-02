use core_geometry::{Mesh, Scalar, Vec3};
use rayon::prelude::*;

/// A face this near the lowest point of the model is resting on the plate. Half a
/// millimetre is ten layers at the height most of these machines print at.
const BOTTOM_BAND_MM: Scalar = 0.5;

/// What an orientation costs the print, each term in its own unit.
///
/// The total is what orientations are ranked by: lower is better. Its terms are made
/// dimensionless before they are weighed against each other, so the number means the same
/// thing for a miniature and for a bust; see `docs/design/orientation.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    /// Downward-facing area past the overhang angle, weighted by how far past it goes,
    /// square millimetres.
    pub overhang_mm2: Scalar,
    /// Area lying on the plate, square millimetres.
    pub bottom_mm2: Scalar,
    pub height_mm: Scalar,
    /// The model's shadow on the plate, square millimetres: what the peel has to pull at
    /// most, and an exact answer for a convex model.
    pub shadow_mm2: Scalar,
    /// The largest cross-section the peel actually has to pull, square millimetres, or
    /// `None` for a candidate that was never sliced.
    pub peak_mm2: Option<Scalar>,
    pub total: Scalar,
}

/// How much each term of [`Score`] weighs. Tuned by eye on the fixtures, not trained;
/// `docs/decisions/0087` says what that costs us.
const W_OVERHANG: Scalar = 1.0;
const W_PEEL: Scalar = 0.6;
const W_HEIGHT: Scalar = 0.2;
const W_BOTTOM: Scalar = 0.3;

/// A face of the model, reduced to what scoring an orientation asks of it.
///
/// Built once and read by every candidate: the areas and normals of a mesh do not turn
/// with it, so nothing here is recomputed a direction.
pub struct Faces<'a> {
    mesh: &'a Mesh,
    normals: Vec<Vec3>,
    areas: Vec<Scalar>,
    centroids: Vec<Vec3>,
    pub total_area_mm2: Scalar,
    /// The model's own scale, from the cube root of the box it fills: what the height and
    /// the cross-section are divided by so that neither depends on how big the model is.
    length_mm: Scalar,
}

impl<'a> Faces<'a> {
    pub fn of(mesh: &'a Mesh) -> Option<Self> {
        let mut normals = Vec::with_capacity(mesh.faces.len());
        let mut areas = Vec::with_capacity(mesh.faces.len());
        let mut centroids = Vec::with_capacity(mesh.faces.len());

        for triangle in mesh.triangles() {
            let scaled = triangle.normal_unnormalized();
            let area = scaled.length() / 2.0;
            if area <= Scalar::EPSILON {
                continue;
            }
            normals.push(scaled / (area * 2.0));
            areas.push(area);
            centroids.push((triangle.a + triangle.b + triangle.c) / 3.0);
        }

        let bounds = mesh.aabb()?;
        let size = bounds.maxs - bounds.mins;
        let length_mm = (size.x * size.y * size.z).cbrt().max(Scalar::EPSILON);
        let total_area_mm2 = areas.iter().sum::<Scalar>().max(Scalar::EPSILON);

        (!areas.is_empty()).then_some(Self {
            mesh,
            normals,
            areas,
            centroids,
            total_area_mm2,
            length_mm,
        })
    }

    /// What the model costs standing with `down` pointing at the plate, without the peel
    /// term, which needs the model sliced.
    ///
    /// Nothing is rotated: an orientation only moves which way is down, and area, normals
    /// and distances along a direction are all read off the model where it lies.
    pub fn score(&self, down: Vec3, max_overhang_deg: Scalar) -> Score {
        let lean = max_overhang_deg.to_radians().sin();
        let floor = self
            .mesh
            .vertices
            .par_iter()
            .map(|vertex| vertex.dot(down))
            .reduce(|| Scalar::NEG_INFINITY, Scalar::max);

        let (overhang_mm2, bottom_mm2, shadow_mm2) = (0..self.areas.len())
            .into_par_iter()
            .map(|face| {
                let facing = self.normals[face].dot(down);
                let area = self.areas[face];
                // A downward face lying on the plate is held by the plate, so it is the
                // footprint rather than an overhang.
                let on_plate = self.centroids[face].dot(down) > floor - BOTTOM_BAND_MM;
                let overhang = if facing > lean && !on_plate {
                    (facing - lean) * area
                } else {
                    0.0
                };
                (
                    overhang,
                    if on_plate && facing > FLAT { area } else { 0.0 },
                    // Half the area a model's faces project onto the plate is its
                    // silhouette, which is the cross-section for anything convex.
                    area * facing.abs() / 2.0,
                )
            })
            .reduce(|| (0.0, 0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2));

        let height_mm = self.extent(down);
        let mut score = Score {
            overhang_mm2,
            bottom_mm2,
            height_mm,
            shadow_mm2,
            peak_mm2: None,
            total: 0.0,
        };
        score.total = self.total(&score);
        score
    }

    /// The same score with the shadow replaced by the section actually measured, once
    /// the candidate has been sliced.
    pub fn with_peak(&self, score: Score, peak_mm2: Scalar) -> Score {
        let mut measured = Score {
            peak_mm2: Some(peak_mm2),
            ..score
        };
        measured.total = self.total(&measured);
        measured
    }

    /// How tall the model stands with `down` pointing at the plate, millimetres.
    pub fn extent(&self, down: Vec3) -> Scalar {
        let (low, high) = self
            .mesh
            .vertices
            .par_iter()
            .map(|vertex| {
                let along = vertex.dot(down);
                (along, along)
            })
            .reduce(
                || (Scalar::INFINITY, Scalar::NEG_INFINITY),
                |a, b| (a.0.min(b.0), a.1.max(b.1)),
            );
        high - low
    }

    /// The peel term takes the measured section where there is one and the shadow where
    /// there is not, so the two passes rank on the same quantity.
    fn total(&self, score: &Score) -> Scalar {
        let area = self.total_area_mm2;
        let peel = score.peak_mm2.unwrap_or(score.shadow_mm2) / (self.length_mm * self.length_mm);
        W_OVERHANG * (score.overhang_mm2 / area)
            + W_PEEL * peel
            + W_HEIGHT * (score.height_mm / self.length_mm)
            - W_BOTTOM * (score.bottom_mm2 / area)
    }
}

/// A face whose normal is within a few degrees of straight down is lying on the plate
/// rather than leaning on it.
const FLAT: Scalar = 0.996;

#[cfg(test)]
mod tests {
    use super::*;

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
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

    #[test]
    fn a_cube_standing_on_a_face_has_no_overhang_and_a_whole_face_down() {
        let cube = cuboid(Vec3::splat(10.0));
        let faces = Faces::of(&cube).expect("a cube has faces");
        let score = faces.score(Vec3::NEG_Z, 45.0);

        assert!(score.overhang_mm2 < 1e-3, "every wall is vertical");
        assert!(
            (score.bottom_mm2 - 100.0).abs() < 1e-3,
            "the whole 10x10 face rests on the plate, got {}",
            score.bottom_mm2
        );
        assert!((score.height_mm - 10.0).abs() < 1e-3);
    }

    #[test]
    fn a_cube_on_its_corner_rests_on_nothing_and_scores_worse() {
        let cube = cuboid(Vec3::splat(10.0));
        let faces = Faces::of(&cube).expect("a cube has faces");
        let down = Vec3::splat(-1.0).normalize();
        let score = faces.score(down, 45.0);

        assert!(score.bottom_mm2 < 1e-3, "no face is flat against the plate");
        assert!(
            score.total > faces.score(Vec3::NEG_Z, 45.0).total,
            "standing on a corner is worse than standing on a face"
        );
        // The three lower faces tilt 35 degrees from vertical, which resin bridges on its
        // own; at 20 degrees they stop being self-supporting.
        assert!(score.overhang_mm2 < 1e-3);
        assert!(faces.score(down, 20.0).overhang_mm2 > 0.0);
    }

    #[test]
    fn a_ceiling_above_the_plate_is_an_overhang_and_the_footprint_is_not() {
        // A stem with a wider cap on top: the cap's underside is what needs holding up.
        let mut mushroom = cuboid(Vec3::new(2.0, 2.0, 10.0));
        let cap = cuboid(Vec3::new(10.0, 10.0, 2.0));
        let offset = mushroom.vertices.len() as u32;
        mushroom.vertices.extend(
            cap.vertices
                .iter()
                .map(|v| *v + Vec3::new(-4.0, -4.0, 10.0)),
        );
        mushroom.faces.extend(
            cap.faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );

        let faces = Faces::of(&mushroom).expect("the mushroom has faces");
        let score = faces.score(Vec3::NEG_Z, 45.0);
        assert!(
            (score.bottom_mm2 - 4.0).abs() < 1e-3,
            "only the 2 by 2 stem rests on the plate, got {}",
            score.bottom_mm2
        );
        // The cap's whole underside is horizontal: 100 mm2 weighted by 1 - sin(45).
        let expected = 100.0 * (1.0 - std::f32::consts::FRAC_PI_4.sin());
        assert!(
            (score.overhang_mm2 - expected).abs() < 1e-2,
            "expected {expected}, got {}",
            score.overhang_mm2
        );
    }

    #[test]
    fn a_tall_box_is_scored_shorter_lying_down() {
        let box_mesh = cuboid(Vec3::new(10.0, 10.0, 60.0));
        let faces = Faces::of(&box_mesh).expect("a box has faces");
        assert!((faces.score(Vec3::NEG_Z, 45.0).height_mm - 60.0).abs() < 1e-3);
        assert!((faces.score(Vec3::NEG_X, 45.0).height_mm - 10.0).abs() < 1e-3);
    }

    #[test]
    fn the_shadow_stands_in_for_the_section_until_one_is_measured() {
        let cube = cuboid(Vec3::splat(10.0));
        let faces = Faces::of(&cube).expect("a cube has faces");
        let score = faces.score(Vec3::NEG_Z, 45.0);

        assert_eq!(score.peak_mm2, None);
        assert!(
            (score.shadow_mm2 - 100.0).abs() < 1e-3,
            "a cube's shadow is one face, got {}",
            score.shadow_mm2
        );

        // A hollow model's section is smaller than its shadow, and scores better for it.
        let measured = faces.with_peak(score, 40.0);
        assert_eq!(measured.peak_mm2, Some(40.0));
        assert!(measured.total < score.total);
    }

    #[test]
    fn a_thin_slab_casts_a_smaller_shadow_on_its_edge_than_on_its_face() {
        let slab = cuboid(Vec3::new(40.0, 40.0, 4.0));
        let faces = Faces::of(&slab).expect("a slab has faces");

        let flat = faces.score(Vec3::NEG_Z, 45.0);
        let edge = faces.score(Vec3::NEG_X, 45.0);
        assert!((flat.shadow_mm2 - 1600.0).abs() < 1e-2);
        assert!((edge.shadow_mm2 - 160.0).abs() < 1e-2);
        assert!(edge.total < flat.total, "the peel is what a slab fails on");
    }

    #[test]
    fn a_mesh_with_no_area_has_nothing_to_score() {
        let flat = Mesh::new(vec![Vec3::ZERO, Vec3::X, Vec3::X * 2.0], vec![[0, 1, 2]]);
        assert!(Faces::of(&flat).is_none());
        assert!(Faces::of(&Mesh::default()).is_none());
    }
}
