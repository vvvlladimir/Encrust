//! What `--drain-at auto` chose, located where the model prints, as `slicing.trapped` is
//! (ADR 0400, 0401).

use core_engine::HoleRole;
use core_geometry::{Aabb, Mat4, Scalar, Transform, Vec3};
use core_supports::Trapped;
use serde::Serialize;

use super::located::TrappedAt;
use crate::stage::Searched;

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
    /// `searched`, carried from where the model stood when it searched to where it is
    /// `placed` now, then up by the `lifted_mm` the supports stood it off the plate.
    pub fn of(searched: &Searched, placed: Option<Transform>, lifted_mm: Scalar) -> Self {
        let back = searched.stood.inverse_matrix().unwrap_or(Mat4::IDENTITY);
        let now = placed.map_or(Mat4::IDENTITY, |placed| placed.to_matrix() * back);
        let moved = Mat4::from_translation(Vec3::Z * lifted_mm) * now;
        let drained = &searched.drained;
        Self {
            pockets: drained
                .pockets
                .iter()
                .enumerate()
                .map(|(index, pocket)| PocketDrained {
                    pocket: TrappedAt::from(&carried(pocket, moved)),
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
                    at_mm: moved.transform_point3(suggested.hole.at).to_array(),
                    axis: (moved.transform_vector3(suggested.hole.axis))
                        .normalize_or_zero()
                        .to_array(),
                    diameter_mm: suggested.hole.diameter_mm,
                    depth_mm: suggested.hole.depth_mm,
                })
                .collect(),
            scans: drained.scans,
        }
    }
}

fn carried(pocket: &Trapped, moved: Mat4) -> Trapped {
    let corners = pocket
        .bounds
        .vertices()
        .map(|corner| moved.transform_point3(corner));
    let (mins, maxs) = (corners.iter()).fold(
        (
            Vec3::splat(Scalar::INFINITY),
            Vec3::splat(Scalar::NEG_INFINITY),
        ),
        |(mins, maxs), corner| (mins.min(*corner), maxs.max(*corner)),
    );
    Trapped {
        at: moved.transform_point3(pocket.at),
        bounds: Aabb::new(mins, maxs),
        ..*pocket
    }
}
