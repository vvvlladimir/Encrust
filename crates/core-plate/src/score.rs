use core_analysis::{disc_pull_n, equivalent_disc_mm};
use core_geometry::{Mesh, Scalar, Vec3};
use rayon::prelude::*;

use crate::cut::Cut;
use crate::low_points::LowPoints;
use crate::orient::OrientSettings;

/// A face this near the lowest point of the model is resting on the plate. Half a
/// millimetre is ten layers at the height most of these machines print at.
const BOTTOM_BAND_MM: Scalar = 0.5;

/// What an orientation costs the print, each term in its own unit; lower total ranks better. Terms
/// are made dimensionless before weighing; see `docs/design/orientation.md`.
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
    /// The plan area of every face leaning past the overhang angle, however far past,
    /// square millimetres: what placement spreads its contacts over, one a spacing apart.
    pub under_mm2: Scalar,
    /// The same area times how high each face stands over the plate, cubic millimetres.
    pub under_mm3: Scalar,
    /// Area lying on the plate, square millimetres, and zero for a model on supports.
    pub bottom_mm2: Scalar,
    pub height_mm: Scalar,
    /// The model's shadow on the plate, square millimetres: what the peel has to pull at
    /// most, and an exact answer for a convex model.
    pub shadow_mm2: Scalar,
    /// The largest cross-section, square millimetres, or `None` for a candidate that was
    /// never sliced.
    pub peak_mm2: Option<Scalar>,
    /// The hardest pull on the film of any section, newtons, or `None` for a candidate
    /// that was never sliced.
    pub peak_pull_n: Option<Scalar>,
    /// The widest hole sealed from below, square millimetres, zero for none, or `None`
    /// for a candidate that was never sliced.
    pub cup_mm2: Option<Scalar>,
    /// How far supports stand the pose off the plate, millimetres; zero set on it.
    pub lift_mm: Scalar,
    pub total: Scalar,
}

impl Score {
    /// How tall the stack stands, the model and the lift under it, millimetres: what a
    /// print's layer count and time follow.
    pub fn stack_mm(&self) -> Scalar {
        self.height_mm + self.lift_mm
    }
}

/// What an orientation is searched for: one term of [`Score`] made to weigh more.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OrientGoal {
    /// The least resin in the supports, whatever else the pose costs.
    #[default]
    FewestSupports,
    /// The smallest cross-section pulled off the film at any layer.
    LeastPeel,
    /// The shortest stack, model and lift, which sets the layers and the time.
    ShortestPrint,
}

impl OrientGoal {
    pub const ALL: [Self; 3] = [Self::FewestSupports, Self::LeastPeel, Self::ShortestPrint];

    /// Where this goal's reading sits among every goal's, the order of [`Self::ALL`].
    pub const fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FewestSupports => "Fewest supports",
            Self::LeastPeel => "Least peel force",
            Self::ShortestPrint => "Shortest print",
        }
    }

    /// Each goal's own weights, set so that its own term decides between most
    /// orientations; height above all is a tie-break for every goal but the shortest print.
    fn weights(self) -> Weights {
        let base = Weights {
            support: 1.0,
            volume: 0.0,
            peel: 0.6,
            height: 0.05,
            bottom: 0.3,
        };
        match self {
            Self::FewestSupports => Weights {
                support: 0.0,
                volume: 1.0,
                peel: 0.0,
                ..base
            },
            Self::LeastPeel => Weights {
                support: 0.5,
                peel: 2.0,
                ..base
            },
            Self::ShortestPrint => Weights {
                support: 0.3,
                peel: 0.0,
                height: 1.5,
                ..base
            },
        }
    }
}

/// What auto-orient is asked for: one goal, or every goal weighed against the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrientPreset {
    /// One goal, ranked by its own policy alone.
    Goal(OrientGoal),
    /// Every goal at once, each giving up as small and as even a share of its best as the
    /// model allows. A goal added to [`OrientGoal::ALL`] is weighed with the rest.
    Auto,
}

impl Default for OrientPreset {
    fn default() -> Self {
        Self::Goal(OrientGoal::default())
    }
}

impl From<OrientGoal> for OrientPreset {
    fn from(goal: OrientGoal) -> Self {
        Self::Goal(goal)
    }
}

impl OrientPreset {
    /// Every preset: each goal, then the balance of them all.
    pub fn all() -> impl Iterator<Item = Self> {
        OrientGoal::ALL
            .into_iter()
            .map(Self::Goal)
            .chain(std::iter::once(Self::Auto))
    }

    /// The name a person picks the preset by.
    pub fn label(self) -> &'static str {
        match self {
            Self::Goal(goal) => goal.label(),
            Self::Auto => "Auto",
        }
    }

    /// The search's weights: the goal's own, or the mean of every goal's.
    fn weights(self) -> Weights {
        match self {
            Self::Goal(goal) => goal.weights(),
            Self::Auto => {
                let count = OrientGoal::ALL.len() as Scalar;
                let sum = OrientGoal::ALL
                    .map(OrientGoal::weights)
                    .into_iter()
                    .fold(Weights::default(), Weights::plus);
                sum.times(1.0 / count)
            }
        }
    }
}

/// How much each term of [`Score`] weighs for one goal, set by eye and checked on
/// `orient-lab`. `support` weighs everything a support is put under: the overhang, the
/// tips and the height they stand at.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Weights {
    support: Scalar,
    volume: Scalar,
    peel: Scalar,
    height: Scalar,
    bottom: Scalar,
}

impl Weights {
    fn plus(self, other: Self) -> Self {
        Self {
            support: self.support + other.support,
            volume: self.volume + other.volume,
            peel: self.peel + other.peel,
            height: self.height + other.height,
            bottom: self.bottom + other.bottom,
        }
    }

    fn times(self, factor: Scalar) -> Self {
        Self {
            support: self.support * factor,
            volume: self.volume * factor,
            peel: self.peel * factor,
            height: self.height * factor,
            bottom: self.bottom * factor,
        }
    }
}

/// What a tip costs, in flat patches one contact holds: the whole model can hang off it,
/// and `orient-lab` found the search leaning on tips at any less (ADR 0275).
const TIP_PATCHES: Scalar = 3.0;
/// How near the overhang angle a face is held as past it, degrees: placement reads the
/// angle off pixel steps, and holds much of a face that meets it exactly.
const AT_THE_ANGLE_DEG: Scalar = 1.0;
/// Support standing a model's own length tall costs half again the overhang it holds.
const W_TALL: Scalar = 0.5;

/// A face looks up in its file once its normal is within sixty degrees of +Z: the side its
/// author meant to be seen, as every stage reads it.
pub const LOOKS_UP: Scalar = 0.5;

/// A face of the model, reduced to what scoring an orientation asks of it. Built once for all
/// candidates: areas and normals do not turn with the mesh.
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
    weights: Weights,
    on_supports: bool,
    /// The sine of the overhang angle: how far a face may face down before it hangs.
    lean: Scalar,
    /// The same short of [`AT_THE_ANGLE_DEG`]: how far a face may face down before it is
    /// held.
    held_from: Scalar,
    /// What one contact holds up, square millimetres: what a tip costs is a few of them.
    patch_mm2: Scalar,
    /// How far supports lift the model off the plate, millimetres: the least a support is tall.
    lift_mm: Scalar,
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
            weights: OrientGoal::default().weights(),
            on_supports: false,
            lean: 0.0,
            held_from: 0.0,
            patch_mm2: 0.0,
            lift_mm: 0.0,
        })?;
        Some(faces.for_settings(&OrientSettings::default()))
    }

    /// The same faces, ranked for the goal of `settings` and held the way it says: on
    /// supports or on the plate, past its overhang angle, by contacts its spacing apart.
    pub fn for_settings(self, settings: &OrientSettings) -> Self {
        Self {
            weights: settings.preset.weights(),
            on_supports: settings.on_supports,
            lean: settings.max_overhang_deg.to_radians().sin(),
            held_from: (settings.max_overhang_deg - AT_THE_ANGLE_DEG)
                .max(0.0)
                .to_radians()
                .sin(),
            patch_mm2: settings.contact_spacing_mm * settings.contact_spacing_mm,
            lift_mm: settings.lift_mm,
            ..self
        }
    }

    /// What the model costs standing with `down` pointing at the plate, without the peel term.
    /// Nothing is rotated: area, normals and distances are read off the model where it lies.
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
            under_mm2,
            under_mm3,
            bottom_mm2,
            shadow_mm2,
        ] = (0..self.areas.len())
            .into_par_iter()
            .map(|face| self.face_terms(face, down, floor, on_supports))
            .reduce(|| [0.0; 6], |a, b| std::array::from_fn(|at| a[at] + b[at]));

        let plate = (!on_supports).then_some(floor);
        let mut score = Score {
            overhang_mm2,
            low_points: self.low_points.count(down, plate, BOTTOM_BAND_MM),
            support_mm3,
            under_mm2,
            under_mm3,
            bottom_mm2,
            height_mm: self.extent(down),
            shadow_mm2,
            peak_mm2: None,
            peak_pull_n: None,
            cup_mm2: None,
            lift_mm: if on_supports { self.lift_mm } else { 0.0 },
            total: 0.0,
        };
        score.total = self.total(&score);
        score
    }

    /// What a support is put under, over the surface: the share of the model that hangs.
    pub fn held(&self, score: &Score) -> Scalar {
        self.terms(score).held
    }

    /// One face's share of overhang, support volume, area under supports and its volume,
    /// footprint and shadow.
    fn face_terms(&self, face: usize, down: Vec3, floor: Scalar, on_supports: bool) -> [Scalar; 6] {
        let lean = self.lean;
        let facing = self.normals[face].dot(down);
        let area = self.areas[face];
        let above_mm = floor - self.centroids[face].dot(down);
        // A downward face lying on the plate is held by the plate, so it is the footprint
        // rather than an overhang. On supports nothing lies on the plate.
        let on_plate = !on_supports && above_mm < BOTTOM_BAND_MM;
        let hangs = facing > self.held_from && !on_plate;
        let overhang = if hangs {
            (facing - lean).max(0.0) * area
        } else {
            0.0
        };
        let under = if hangs { facing * area } else { 0.0 };
        [
            overhang,
            overhang * above_mm,
            under,
            under * above_mm,
            if on_plate && facing > FLAT { area } else { 0.0 },
            // Half the area a model's faces project onto the plate is its silhouette,
            // which is the cross-section for anything convex.
            area * facing.abs() / 2.0,
        ]
    }

    /// The same score with the shadow replaced by what the cut sections measured, once
    /// the candidate has been sliced: the largest of them, the hardest pull of any, and
    /// the widest suction cup.
    pub fn with_cut(&self, score: Score, cut: Cut) -> Score {
        let mut measured = Score {
            peak_mm2: Some(cut.peak_mm2),
            peak_pull_n: Some(cut.pull_n),
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

    /// What `goal` is searched for, whichever goal these faces are ranked for: its term
    /// unweighted, and as the proof reads them the pull, newtons, and the stack, millimetres.
    pub fn reading(&self, goal: OrientGoal, score: &Score) -> Scalar {
        let terms = self.terms(score);
        match goal {
            OrientGoal::FewestSupports => terms.volume,
            OrientGoal::LeastPeel => peak_pull_n(score),
            OrientGoal::ShortestPrint => score.stack_mm(),
        }
    }

    fn total(&self, score: &Score) -> Scalar {
        self.weighed(self.weights, score)
    }

    /// What `score` would total ranked for `preset`, whichever preset these faces weigh.
    pub(crate) fn total_for(&self, preset: OrientPreset, score: &Score) -> Scalar {
        self.weighed(preset.weights(), score)
    }

    fn weighed(&self, w: Weights, score: &Score) -> Scalar {
        let terms = self.terms(score);
        w.support * terms.held
            + w.volume * terms.volume
            + w.peel * terms.peel
            + w.height * terms.height
            - w.bottom * terms.bottom
    }

    /// Every term made dimensionless. The peel is the area of the disc pulling as hard.
    fn terms(&self, score: &Score) -> Terms {
        let area = self.total_area_mm2;
        let length = self.length_mm;
        let pulled = score.peak_pull_n.map_or(score.shadow_mm2, |pull| {
            let across_mm = equivalent_disc_mm(f64::from(pull)) as Scalar;
            std::f32::consts::PI * across_mm * across_mm / 4.0
        });
        let tips_mm2 = score.low_points as Scalar * self.patch_mm2 * TIP_PATCHES;
        Terms {
            held: (score.overhang_mm2 + tips_mm2) / area
                + W_TALL * score.support_mm3 / (area * length),
            volume: volume_mm3(score, tips_mm2) / (area * length),
            peel: pulled / (length * length),
            height: score.stack_mm() / length,
            bottom: score.bottom_mm2 / area,
        }
    }
}

/// The hardest pull measured where the candidate was cut, and the pull of a disc of its
/// shadow where it was not, which no section of it pulls past, newtons.
fn peak_pull_n(score: &Score) -> Scalar {
    score.peak_pull_n.unwrap_or_else(|| {
        let across_mm = 2.0 * f64::from(score.shadow_mm2 / std::f32::consts::PI).sqrt();
        disc_pull_n(across_mm) as Scalar
    })
}

/// About how much support stands under a pose, as the plan area placement holds times the
/// height it is held at, lift included, cubic millimetres; a tip stands half the model's
/// height up.
fn volume_mm3(score: &Score, tips_mm2: Scalar) -> Scalar {
    score.under_mm3
        + (score.under_mm2 + tips_mm2) * score.lift_mm
        + tips_mm2 * score.height_mm / 2.0
}

/// The terms of a [`Score`], each made dimensionless, before they are weighed.
struct Terms {
    held: Scalar,
    volume: Scalar,
    peel: Scalar,
    height: Scalar,
    bottom: Scalar,
}

/// A face whose normal is within a few degrees of straight down is lying on the plate
/// rather than leaning on it.
const FLAT: Scalar = 0.996;

#[cfg(test)]
mod tests;
