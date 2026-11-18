//! Where each finding of a report is, in plate millimetres from the plate's front left
//! corner after placement: the frame `--drain-at` takes (ADR 0400).

use core_analysis::{Measured, Risk, RiskKind};
use core_geometry::{Aabb, Scalar};
use core_raster::RasterSettings;
use core_slicer::LayerPlan;
use core_supports::Trapped;
use serde::Serialize;

/// One risk the masks show.
#[derive(Debug, Serialize)]
pub struct RiskAt {
    /// Counted from one, as a printer's screen does.
    pub layer: usize,
    /// Plate millimetres; z is the bottom of the layer, where it cures onto what is under it.
    pub at_mm: [Scalar; 3],
    #[serde(flatten)]
    pub kind: Kind,
}

/// What a risk is, with its own readings.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Kind {
    Island {
        area_mm2: f32,
    },
    Floating {
        area_mm2: f32,
    },
    Lever {
        stress_mpa: f32,
        lever_mm: f32,
        neck_mm2: f32,
    },
    Peel {
        force_n: f32,
    },
    SuctionCup {
        area_mm2: f32,
        volume_mm3: f32,
    },
    Thin {
        width_mm: f32,
    },
}

impl From<RiskKind> for Kind {
    fn from(kind: RiskKind) -> Self {
        match kind {
            RiskKind::Island { area_mm2 } => Self::Island { area_mm2 },
            RiskKind::Floating { area_mm2 } => Self::Floating { area_mm2 },
            RiskKind::Lever {
                stress_mpa,
                lever_mm,
                neck_mm2,
                ..
            } => Self::Lever {
                stress_mpa,
                lever_mm,
                neck_mm2,
            },
            RiskKind::Peel { force_n } => Self::Peel { force_n },
            RiskKind::SuctionCup {
                area_mm2,
                volume_mm3,
            } => Self::SuctionCup {
                area_mm2,
                volume_mm3,
            },
            RiskKind::Thin { width_mm } => Self::Thin { width_mm },
        }
    }
}

/// A pocket of resin with no way out.
#[derive(Debug, Serialize)]
pub struct TrappedAt {
    /// A point certainly inside the pocket, on its lowest layer: where a hole would go.
    pub at_mm: [Scalar; 3],
    pub volume_mm3: Scalar,
    pub bounds_mm: Bounds,
}

/// A box, plate millimetres.
#[derive(Debug, Serialize)]
pub struct Bounds {
    pub min: [Scalar; 3],
    pub max: [Scalar; 3],
}

impl From<&Trapped> for TrappedAt {
    fn from(pocket: &Trapped) -> Self {
        Self {
            at_mm: pocket.at.to_array(),
            volume_mm3: pocket.volume_mm3,
            bounds_mm: Bounds::of(&pocket.bounds),
        }
    }
}

impl Bounds {
    fn of(aabb: &Aabb) -> Self {
        Self {
            min: aabb.mins.to_array(),
            max: aabb.maxs.to_array(),
        }
    }
}

/// What places the findings of a stack of masks on the plate: the panel they were drawn
/// for and the layers they were drawn at.
#[derive(Clone, Copy)]
pub struct Frame<'a> {
    pub panel: &'a RasterSettings,
    pub plan: &'a LayerPlan,
}

impl Frame<'_> {
    /// Every risk `measured` names, in print order.
    pub fn risks(self, measured: &Measured) -> Vec<RiskAt> {
        measured
            .risks()
            .iter()
            .map(|risk| self.risk(risk))
            .collect()
    }

    fn risk(self, risk: &Risk) -> RiskAt {
        let at = self.panel.plate_mm(risk.at_mm);
        RiskAt {
            layer: risk.layer + 1,
            at_mm: at.extend(self.floor_mm(risk.layer)).to_array(),
            kind: risk.kind.into(),
        }
    }

    /// The bottom of `layer`. The masks are drawn from the plan, so every layer is in it.
    fn floor_mm(self, layer: usize) -> Scalar {
        self.plan.band_of(layer).map_or_else(
            || self.plan.nominal_thickness() * layer as Scalar,
            |band| band.0,
        )
    }
}
