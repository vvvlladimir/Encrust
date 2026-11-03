//! What `--drain-at auto` chose, located where the model prints, as `slicing.trapped` is
//! (ADR 0396, 0398).

use core_engine::{Drained, HoleRole};
use core_geometry::{Aabb, Scalar, Vec3};
use core_supports::Trapped;
use serde::Serialize;

use super::located::TrappedAt;

/// The pockets the search found and the holes it proved drain them.
#[derive(Debug, Serialize)]
pub struct AutoDrains {
    pub pockets: Vec<PocketDrained>,
    pub holes: Vec<HoleAt>,
    /// How many times the model was sliced and scanned to choose them.
    pub scans: usize,
}

/// A pocket the model trapped before its holes, and whether they empty it.
#[derive(Debug, Serialize)]
pub struct PocketDrained {
    #[serde(flatten)]
    pub pocket: TrappedAt,
    pub drained: bool,
}

/// One hole the search chose.
#[derive(Debug, Serialize)]
pub struct HoleAt {
    /// Which of `pockets` it serves, counted from zero.
    pub pocket: usize,
    pub role: Role,
    /// The middle of its mouth on the surface, plate millimetres.
    pub at_mm: [Scalar; 3],
    /// The unit direction it is drilled in, into the model.
    pub axis: [Scalar; 3],
    pub diameter_mm: Scalar,
    pub depth_mm: Scalar,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Drain,
    Vent,
}

impl AutoDrains {
    /// `drained` moved up by the `lifted_mm` the supports stood the model off the plate.
    pub fn of(drained: &Drained, lifted_mm: Scalar) -> Self {
        let up = Vec3::Z * lifted_mm;
        Self {
            pockets: drained
                .pockets
                .iter()
                .enumerate()
                .map(|(index, pocket)| PocketDrained {
                    pocket: TrappedAt::from(&raised(pocket, up)),
                    drained: drained.is_drained(index),
                })
                .collect(),
            holes: drained
                .holes
                .iter()
                .map(|suggested| HoleAt {
                    pocket: suggested.pocket,
                    role: match suggested.role {
                        HoleRole::Drain => Role::Drain,
                        HoleRole::Vent => Role::Vent,
                    },
                    at_mm: (suggested.hole.at + up).to_array(),
                    axis: suggested.hole.axis.normalize_or_zero().to_array(),
                    diameter_mm: suggested.hole.diameter_mm,
                    depth_mm: suggested.hole.depth_mm,
                })
                .collect(),
            scans: drained.scans,
        }
    }
}

fn raised(pocket: &Trapped, up: Vec3) -> Trapped {
    Trapped {
        at: pocket.at + up,
        bounds: Aabb::new(pocket.bounds.mins + up, pocket.bounds.maxs + up),
        ..*pocket
    }
}
