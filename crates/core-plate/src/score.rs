use core_geometry::{Mesh, Scalar, Vec3};
use rayon::prelude::*;

use crate::cut::Cut;
use crate::low_points::LowPoints;
use crate::orient::OrientSettings;
use crate::section::Section;

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
    /// Tips lower than everything around them, off the plate: each needs a support of its
    /// own, however little area it has.
    pub low_points: usize,
    /// The overhang times how high it stands over the plate, cubic millimetres: how much
    /// support stands under it, at most.
    pub support_mm3: Scalar,
    /// Area lying on the plate, square millimetres, and zero for a model on supports.
    pub bottom_mm2: Scalar,
    pub height_mm: Scalar,
    /// The model's shadow on the plate, square millimetres: what the peel has to pull at
    /// most, and an exact answer for a convex model.
    pub shadow_mm2: Scalar,
    /// The largest cross-section, square millimetres, or `None` for a candidate that was
    /// never sliced.
    pub peak_mm2: Option<Scalar>,
    /// The hardest pull on the film of any section, as Stefan's torsion constant, mm^4,
    /// or `None` for a candidate that was never sliced.
    pub peak_pull_mm4: Option<Scalar>,
    /// The part of `overhang_mm2` on faces that look up in the model's own file, which
    /// is the side its author meant to be seen.
    pub top_overhang_mm2: Scalar,
    /// The widest hole sealed from below, square millimetres, zero for none, or `None`
    /// for a candidate that was never sliced.
    pub cup_mm2: Option<Scalar>,
    pub total: Scalar,
}

/// What an orientation is searched for: one term of [`Score`] made to weigh more.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OrientGoal {
    /// The least surface hanging over nothing, so the fewest supports.
    #[default]
    FewestSupports,
    /// The smallest cross-section pulled off the film at any layer.
    LeastPeel,
    /// The fewest layers.
    ShortestPrint,
    /// No support on the faces that look up in the file.
    CleanTop,
}

impl OrientGoal {
    pub const ALL: [Self; 4] = [
        Self::FewestSupports,
        Self::LeastPeel,
        Self::ShortestPrint,
        Self::CleanTop,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::FewestSupports => "Fewest supports",
            Self::LeastPeel => "Least peel force",
            Self::ShortestPrint => "Shortest print",
            Self::CleanTop => "Clean top surface",
        }
    }

    /// Each goal's own weights, set so that its own term decides between most
    /// orientations; height above all is a tie-break for every goal but the shortest print.
    fn weights(self) -> Weights {
        let base = Weights {
            support: 1.0,
            peel: 0.6,
            height: 0.05,
            bottom: 0.3,
            top: 0.0,
        };
        match self {
            Self::FewestSupports => base,
            Self::LeastPeel => Weights {
                support: 0.5,
                peel: 2.0,
                ..base
            },
            Self::ShortestPrint => Weights {
                support: 0.3,
                peel: 0.3,
                height: 1.5,
                ..base
            },
            Self::CleanTop => Weights { top: 3.0, ..base },
        }
    }
}

/// How much each term of [`Score`] weighs for one goal, set by eye and checked on
/// `orient-lab`. `support` weighs everything a support is put under: the overhang, the
/// tips and the height they stand at. The top weighs on top of the overhang it already is.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Weights {
    support: Scalar,
    peel: Scalar,
    height: Scalar,
    bottom: Scalar,
    top: Scalar,
}

/// What a tip costs, in flat patches one contact holds: the whole model can hang off it,
/// and `orient-lab` found the search leaning on tips at any less (ADR 0275).
const TIP_PATCHES: Scalar = 3.0;
/// Support standing a model's own length tall costs half again the overhang it holds.
const W_TALL: Scalar = 0.5;

/// A face looks up in its file once its normal is within sixty degrees of +Z.
const LOOKS_UP: Scalar = 0.5;

/// A face of the model, reduced to what scoring an orientation asks of it.
///
/// Built once and read by every candidate: the areas and normals of a mesh do not turn
/// with it, so nothing here is recomputed a direction.
pub struct Faces<'a> {
    mesh: &'a Mesh,
    normals: Vec<Vec3>,
    areas: Vec<Scalar>,
    centroids: Vec<Vec3>,
    low_points: LowPoints,
    pub total_area_mm2: Scalar,
    /// The model's own scale, from the cube root of the box it fills: what the height and
    /// the cross-section are divided by so that neither depends on how big the model is.
    length_mm: Scalar,
    goal: OrientGoal,
    weights: Weights,
    on_supports: bool,
    /// The sine of the overhang angle: how far a face may face down before it hangs.
    lean: Scalar,
    /// What one contact holds up, square millimetres: what a tip costs is a few of them.
    patch_mm2: Scalar,
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

        let faces = (!areas.is_empty()).then(|| Self {
            mesh,
            normals,
            areas,
            centroids,
            low_points: LowPoints::of(mesh),
            total_area_mm2,
            length_mm,
            goal: OrientGoal::default(),
            weights: OrientGoal::default().weights(),
            on_supports: false,
            lean: 0.0,
            patch_mm2: 0.0,
        })?;
        Some(faces.for_settings(&OrientSettings::default()))
    }

    /// The same faces, ranked for the goal of `settings` and held the way it says: on
    /// supports or on the plate, past its overhang angle, by contacts its spacing apart.
    pub fn for_settings(self, settings: &OrientSettings) -> Self {
        Self {
            goal: settings.goal,
            weights: settings.goal.weights(),
            on_supports: settings.on_supports,
            lean: settings.max_overhang_deg.to_radians().sin(),
            patch_mm2: settings.contact_spacing_mm * settings.contact_spacing_mm,
            ..self
        }
    }

    /// What the model costs standing with `down` pointing at the plate, without the peel
    /// term, which needs the model sliced.
    ///
    /// Nothing is rotated: an orientation only moves which way is down, and area, normals
    /// and distances along a direction are all read off the model where it lies.
    pub fn score(&self, down: Vec3) -> Score {
        self.score_held(down, self.on_supports)
    }

    /// The same for the model set straight on the plate, whatever the settings hold it by.
    pub fn score_on_plate(&self, down: Vec3) -> Score {
        self.score_held(down, false)
    }

    fn score_held(&self, down: Vec3, on_supports: bool) -> Score {
        let floor = self
            .mesh
            .vertices
            .par_iter()
            .map(|vertex| vertex.dot(down))
            .reduce(|| Scalar::NEG_INFINITY, Scalar::max);

        let [
            overhang_mm2,
            support_mm3,
            bottom_mm2,
            shadow_mm2,
            top_overhang_mm2,
        ] = (0..self.areas.len())
            .into_par_iter()
            .map(|face| self.face_terms(face, down, floor, on_supports))
            .reduce(|| [0.0; 5], |a, b| std::array::from_fn(|at| a[at] + b[at]));

        let plate = (!on_supports).then_some(floor);
        let mut score = Score {
            overhang_mm2,
            low_points: self.low_points.count(down, plate, BOTTOM_BAND_MM),
            support_mm3,
            bottom_mm2,
            height_mm: self.extent(down),
            shadow_mm2,
            peak_mm2: None,
            peak_pull_mm4: None,
            top_overhang_mm2,
            cup_mm2: None,
            total: 0.0,
        };
        score.total = self.total(&score);
        score
    }

    /// What a support is put under, over the surface: the share of the model that hangs.
    pub fn held(&self, score: &Score) -> Scalar {
        self.terms(score).held
    }

    /// One face's share of overhang, support volume, footprint, shadow and top overhang.
    fn face_terms(&self, face: usize, down: Vec3, floor: Scalar, on_supports: bool) -> [Scalar; 5] {
        let lean = self.lean;
        let facing = self.normals[face].dot(down);
        let area = self.areas[face];
        let above_mm = floor - self.centroids[face].dot(down);
        // A downward face lying on the plate is held by the plate, so it is the footprint
        // rather than an overhang. On supports nothing lies on the plate.
        let on_plate = !on_supports && above_mm < BOTTOM_BAND_MM;
        let overhang = if facing > lean && !on_plate {
            (facing - lean) * area
        } else {
            0.0
        };
        let looks_up = self.normals[face].z > LOOKS_UP;
        [
            overhang,
            overhang * above_mm,
            if on_plate && facing > FLAT { area } else { 0.0 },
            // Half the area a model's faces project onto the plate is its silhouette,
            // which is the cross-section for anything convex.
            area * facing.abs() / 2.0,
            if looks_up { overhang } else { 0.0 },
        ]
    }

    /// The same score with the shadow replaced by what the cut sections measured, once
    /// the candidate has been sliced: the largest of them, the hardest pull of any, and
    /// the widest suction cup.
    pub fn with_cut(&self, score: Score, cut: Cut) -> Score {
        let mut measured = Score {
            peak_mm2: Some(cut.peak_mm2),
            peak_pull_mm4: Some(cut.pull_mm4),
            cup_mm2: Some(cut.cup_mm2),
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

    /// The term `goal` is searched for, unweighted: what the priorities pick by before the
    /// total breaks the tie. See `docs/design/orientation.md`.
    pub fn primary(&self, score: &Score) -> Scalar {
        let terms = self.terms(score);
        match self.goal {
            OrientGoal::FewestSupports => terms.held,
            OrientGoal::LeastPeel => terms.peel,
            OrientGoal::ShortestPrint => terms.height,
            OrientGoal::CleanTop => terms.top,
        }
    }

    fn total(&self, score: &Score) -> Scalar {
        let terms = self.terms(score);
        let w = self.weights;
        w.support * terms.held + w.peel * terms.peel + w.height * terms.height
            - w.bottom * terms.bottom
            + w.top * terms.top
    }

    /// Every term made dimensionless. The peel takes the pull measured where there is one,
    /// as the area of the disc pulling as hard, and the shadow where there is not, which
    /// is never less.
    fn terms(&self, score: &Score) -> Terms {
        let area = self.total_area_mm2;
        let length = self.length_mm;
        let pulled = score.peak_pull_mm4.map_or(score.shadow_mm2, |pull| {
            Section {
                area_mm2: 0.0,
                pull_mm4: pull,
            }
            .equivalent_mm2()
        });
        let tips_mm2 = score.low_points as Scalar * self.patch_mm2 * TIP_PATCHES;
        Terms {
            held: (score.overhang_mm2 + tips_mm2) / area
                + W_TALL * score.support_mm3 / (area * length),
            peel: pulled / (length * length),
            height: score.height_mm / length,
            bottom: score.bottom_mm2 / area,
            top: score.top_overhang_mm2 / area,
        }
    }
}

/// The terms of a [`Score`], each made dimensionless, before they are weighed.
struct Terms {
    held: Scalar,
    peel: Scalar,
    height: Scalar,
    bottom: Scalar,
    top: Scalar,
}

/// A face whose normal is within a few degrees of straight down is lying on the plate
/// rather than leaning on it.
const FLAT: Scalar = 0.996;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_that_looks_up_in_its_file_is_a_top_overhang_once_it_hangs_down() {
        // A 10 by 10 square at z = 10 looking up, and one at z = 20 looking down. Turned
        // upside down, the high square rests on the plate and the low one hangs face down.
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 10.0),
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(0.0, 10.0, 10.0),
                Vec3::new(0.0, 0.0, 20.0),
                Vec3::new(0.0, 10.0, 20.0),
                Vec3::new(10.0, 0.0, 20.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
        );
        let faces = Faces::of(&mesh).expect("two triangles have area");
        let upside_down = faces.score(Vec3::Z);
        // Weighted the way every overhang is: 50 mm2 by 1 - sin(45).
        let expected = 50.0 * (1.0 - 45.0_f32.to_radians().sin());
        assert!(
            (upside_down.top_overhang_mm2 - expected).abs() < 1e-3,
            "the 50 mm2 triangle hangs flat, got {}",
            upside_down.top_overhang_mm2
        );
        let clean = Faces::of(&mesh)
            .expect("two triangles have area")
            .for_settings(&OrientSettings {
                goal: OrientGoal::CleanTop,
                ..OrientSettings::default()
            });
        assert!(
            clean.score(Vec3::Z).total > upside_down.total,
            "a clean top makes the same hang cost more"
        );
    }

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
        let score = faces.score(Vec3::NEG_Z);

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
        let score = faces.score(down);

        assert!(score.bottom_mm2 < 1e-3, "no face is flat against the plate");
        assert!(
            score.total > faces.score(Vec3::NEG_Z).total,
            "standing on a corner is worse than standing on a face"
        );
        // The three lower faces tilt 35 degrees from vertical, which resin bridges on its
        // own; at 20 degrees they stop being self-supporting.
        assert!(score.overhang_mm2 < 1e-3);
        let steep = Faces::of(&cube)
            .expect("a cube has faces")
            .for_settings(&OrientSettings {
                max_overhang_deg: 20.0,
                ..OrientSettings::default()
            });
        assert!(steep.score(down).overhang_mm2 > 0.0);
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
        let score = faces.score(Vec3::NEG_Z);
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

    fn on_supports() -> OrientSettings {
        OrientSettings {
            on_supports: true,
            ..OrientSettings::default()
        }
    }

    #[test]
    fn on_supports_the_underside_hangs_and_earns_no_footprint() {
        let cube = cuboid(Vec3::splat(10.0));
        let lifted = Faces::of(&cube)
            .expect("a cube has faces")
            .for_settings(&on_supports())
            .score(Vec3::NEG_Z);
        // The whole 10 by 10 underside is horizontal: 100 mm2 weighted by 1 - sin(45).
        let expected = 100.0 * (1.0 - std::f32::consts::FRAC_PI_4.sin());
        assert!(
            (lifted.overhang_mm2 - expected).abs() < 1e-2,
            "expected {expected}, got {}",
            lifted.overhang_mm2
        );
        assert!(lifted.bottom_mm2 < 1e-3, "nothing rests on the plate");
    }

    /// A square pyramid with its apex at the origin and its 20 by 20 base 20 mm above.
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
    fn an_apex_needs_a_support_on_supports_and_is_held_by_the_plate_without() {
        let mesh = pyramid();
        let plate = Faces::of(&mesh).expect("a pyramid has faces");
        let held = plate.score(Vec3::NEG_Z);
        assert_eq!(held.low_points, 0, "the apex rests on the plate");

        let lifted = Faces::of(&mesh)
            .expect("a pyramid has faces")
            .for_settings(&on_supports());
        let hanging = lifted.score(Vec3::NEG_Z);
        // Its sides lean 27 degrees from vertical, so nothing else needs holding.
        assert!(hanging.overhang_mm2 < 1e-3);
        assert_eq!(hanging.low_points, 1, "the apex is a point of its own");
        assert!(hanging.total > held.total, "and it costs something");
    }

    #[test]
    fn support_under_a_high_ceiling_is_its_overhang_times_its_height() {
        let mut table = cuboid(Vec3::new(2.0, 2.0, 10.0));
        let top = cuboid(Vec3::new(10.0, 10.0, 2.0));
        let offset = table.vertices.len() as u32;
        table.vertices.extend(
            top.vertices
                .iter()
                .map(|v| *v + Vec3::new(-4.0, -4.0, 10.0)),
        );
        table.faces.extend(
            top.faces
                .iter()
                .map(|[a, b, c]| [a + offset, b + offset, c + offset]),
        );
        let score = Faces::of(&table)
            .expect("the table has faces")
            .score(Vec3::NEG_Z);
        // Every overhang here is the underside of the top, 10 mm over the plate.
        assert!(
            (score.support_mm3 - score.overhang_mm2 * 10.0).abs() < 1e-1,
            "{} against {} x 10 mm",
            score.support_mm3,
            score.overhang_mm2
        );
    }

    #[test]
    fn a_tall_box_is_scored_shorter_lying_down() {
        let box_mesh = cuboid(Vec3::new(10.0, 10.0, 60.0));
        let faces = Faces::of(&box_mesh).expect("a box has faces");
        assert!((faces.score(Vec3::NEG_Z).height_mm - 60.0).abs() < 1e-3);
        assert!((faces.score(Vec3::NEG_X).height_mm - 10.0).abs() < 1e-3);
    }

    #[test]
    fn the_shadow_stands_in_for_the_section_until_one_is_measured() {
        let cube = cuboid(Vec3::splat(10.0));
        let faces = Faces::of(&cube).expect("a cube has faces");
        let score = faces.score(Vec3::NEG_Z);

        assert_eq!(score.peak_mm2, None);
        assert!(
            (score.shadow_mm2 - 100.0).abs() < 1e-3,
            "a cube's shadow is one face, got {}",
            score.shadow_mm2
        );

        // A hollow model's section is smaller than its shadow, and scores better for it.
        let disc_of_40_mm2 = 40.0 * 40.0 / std::f32::consts::TAU;
        let measured = faces.with_cut(
            score,
            Cut {
                peak_mm2: 40.0,
                pull_mm4: disc_of_40_mm2,
                cup_mm2: 0.0,
            },
        );
        assert_eq!(measured.peak_mm2, Some(40.0));
        assert!(measured.total < score.total);
    }

    #[test]
    fn a_thin_slab_casts_a_smaller_shadow_on_its_edge_than_on_its_face() {
        let slab = cuboid(Vec3::new(40.0, 40.0, 4.0));
        let faces = Faces::of(&slab).expect("a slab has faces");

        let flat = faces.score(Vec3::NEG_Z);
        let edge = faces.score(Vec3::NEG_X);
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
