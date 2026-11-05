//! What a condition can read off a model: the variables of `docs/design/recipes.md`.

use core_geometry::{Mat3, Mesh, MeshDiagnostics, Scalar, Transform, Vec3, placed_aabb};
use core_volume::{hollowing_saves_mm3, placed_volume_and_area, raw_saving_mm3};

use crate::condition::{Kind, Value};
use crate::recipe::StepKind;

/// One variable a condition may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    HeightMm,
    WidthMm,
    DepthMm,
    VolumeMl,
    SurfaceCm2,
    Faces,
    Sound,
    Defects,
    Turned,
    Hollow,
    SavesMl,
    SavesShare,
    WorthHollowing,
    Fits,
    Trapped,
    Islands,
    PeelN,
    SupportMl,
}

impl Metric {
    pub const ALL: [Self; 18] = [
        Self::HeightMm,
        Self::WidthMm,
        Self::DepthMm,
        Self::VolumeMl,
        Self::SurfaceCm2,
        Self::Faces,
        Self::Sound,
        Self::Defects,
        Self::Turned,
        Self::Hollow,
        Self::SavesMl,
        Self::SavesShare,
        Self::WorthHollowing,
        Self::Fits,
        Self::Trapped,
        Self::Islands,
        Self::PeelN,
        Self::SupportMl,
    ];

    /// The name a condition reads it by.
    pub fn name(self) -> &'static str {
        match self {
            Self::HeightMm => "height_mm",
            Self::WidthMm => "width_mm",
            Self::DepthMm => "depth_mm",
            Self::VolumeMl => "volume_ml",
            Self::SurfaceCm2 => "surface_cm2",
            Self::Faces => "faces",
            Self::Sound => "sound",
            Self::Defects => "defects",
            Self::Turned => "turned",
            Self::Hollow => "hollow",
            Self::SavesMl => "saves_ml",
            Self::SavesShare => "saves_share",
            Self::WorthHollowing => "worth_hollowing",
            Self::Fits => "fits",
            Self::Trapped => "trapped",
            Self::Islands => "islands",
            Self::PeelN => "peel_n",
            Self::SupportMl => "support_ml",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|metric| metric.name() == name)
    }

    pub fn kind(self) -> Kind {
        match self {
            Self::Sound | Self::Turned | Self::Hollow | Self::WorthHollowing | Self::Fits => {
                Kind::Bool
            }
            _ => Kind::Number,
        }
    }

    /// The steps any one of which, earlier in the recipe, measures it; empty when it is known
    /// from the start.
    pub fn known_after(self) -> &'static [StepKind] {
        match self {
            Self::Trapped => &[StepKind::Drain, StepKind::Check],
            Self::Islands | Self::PeelN | Self::SupportMl => &[StepKind::Check],
            _ => &[],
        }
    }
}

/// The variables measured on one model so far; a step's front end sets what it measured.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metrics {
    values: Vec<(Metric, Value)>,
}

impl Metrics {
    pub fn get(&self, metric: Metric) -> Option<Value> {
        self.values
            .iter()
            .find_map(|(held, value)| (*held == metric).then_some(*value))
    }

    pub fn set(&mut self, metric: Metric, value: Value) {
        match self.values.iter_mut().find(|(held, _)| *held == metric) {
            Some((_, held)) => *held = value,
            None => self.values.push((metric, value)),
        }
    }
}

/// A model as a front end holds it, what `measure` reads.
#[derive(Debug, Clone, Copy)]
pub struct Model<'a> {
    pub mesh: &'a Mesh,
    pub transform: &'a Transform,
    pub diagnostics: &'a MeshDiagnostics,
    /// The user turned it, so an orientation search would undo their choice.
    pub turned: bool,
    pub hollow: bool,
    /// It fits the build volume with what stands under it.
    pub fits: bool,
}

/// Every variable known from the start, `saves_ml` reckoned at `wall_mm`.
pub fn measure(model: &Model, wall_mm: Scalar) -> Metrics {
    let found = model.diagnostics;
    let linear = Mat3::from_mat4(model.transform.to_matrix());
    let (volume_mm3, area_mm2) = placed_volume_and_area(model.mesh, linear);
    let saved_mm3 = raw_saving_mm3(volume_mm3, area_mm2, wall_mm).max(0.0);
    let share = if volume_mm3 > 0.0 {
        saved_mm3 / volume_mm3
    } else {
        0.0
    };
    let worth = found.is_sound()
        && !model.hollow
        && hollowing_saves_mm3(model.mesh, linear, wall_mm).is_some();
    let size = placed_aabb(model.mesh, *model.transform).map_or(Vec3::ZERO, |aabb| aabb.extents());
    let defects =
        found.holes + found.degenerate_faces + found.duplicate_faces + found.non_manifold_edges;

    let mut metrics = Metrics::default();
    let numbers = [
        (Metric::WidthMm, size.x),
        (Metric::DepthMm, size.y),
        (Metric::HeightMm, size.z),
        (Metric::VolumeMl, (volume_mm3 / 1000.0) as Scalar),
        (Metric::SurfaceCm2, (area_mm2 / 100.0) as Scalar),
        (Metric::Faces, model.mesh.faces.len() as Scalar),
        (Metric::Defects, defects as Scalar),
        (Metric::SavesMl, (saved_mm3 / 1000.0) as Scalar),
        (Metric::SavesShare, share as Scalar),
    ];
    for (metric, number) in numbers {
        metrics.set(metric, Value::Number(number));
    }
    let flags = [
        (Metric::Sound, found.is_sound()),
        (Metric::Turned, model.turned),
        (Metric::Hollow, model.hollow),
        (Metric::WorthHollowing, worth),
        (Metric::Fits, model.fits),
    ];
    for (metric, flag) in flags {
        metrics.set(metric, Value::Bool(flag));
    }
    metrics
}

#[cfg(test)]
mod tests;
