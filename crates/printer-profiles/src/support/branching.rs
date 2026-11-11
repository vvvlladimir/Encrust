use serde::{Deserialize, Serialize};

/// How many times its own base diameter a trunk may stand tall: a slenderer column buckles
/// on the peel. See `docs/decisions/0242`.
pub(super) const TRUNK_SLENDERNESS: f32 = 15.0;

/// How far a strut may lean from vertical, degrees. Past a right angle it climbs rather
/// than descends and the merge solve stops having an answer.
pub(super) const MAX_BRANCH_ANGLE_DEG: f32 = 80.0;

/// Below this a branch is vertical and nothing ever merges.
pub(super) const MIN_BRANCH_ANGLE_DEG: f32 = 1.0;

/// How the tips of nearby supports merge into shared trunks, after the measurements of *Clever
/// Support: Efficient Support Structure Generation for Digital Fabrication*; `docs/decisions/0040`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Branching {
    /// Whether tips merge at all. Off is the vertical column of step 7a.
    pub enabled: bool,
    /// How far a strut may lean from vertical, degrees. This is the one knob that lets a
    /// support be anything other than a vertical column.
    pub max_angle_deg: f32,
    /// How far apart two tips may be and still be worth merging, millimetres.
    pub max_merge_distance_mm: f32,
    /// The widest a trunk or strut grows, however tall it stands or much it carries,
    /// millimetres: what bounds the tips one trunk takes. See `docs/decisions/0242`, `0424`.
    pub max_trunk_diameter_mm: f32,
}

impl Default for Branching {
    fn default() -> Self {
        Self {
            enabled: true,
            max_angle_deg: 45.0,
            max_merge_distance_mm: 12.0,
            max_trunk_diameter_mm: 3.0,
        }
    }
}

impl Branching {
    /// The lean, clamped to what the merge solve can answer, in radians.
    pub fn max_angle_rad(&self) -> f32 {
        self.max_angle_deg
            .clamp(MIN_BRANCH_ANGLE_DEG, MAX_BRANCH_ANGLE_DEG)
            .to_radians()
    }

    /// How much ground a strut covers per millimetre it descends.
    pub fn reach_per_drop(&self) -> f32 {
        self.max_angle_rad().tan()
    }
}
