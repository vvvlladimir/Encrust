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
    /// How a trunk's base widens with its branches: radii combine as the root of the sum of their
    /// powers. Two conserves cross-section, three volume.
    pub trunk_exponent: f32,
    /// The widest a trunk's base grows, however tall it stands or much it carries,
    /// millimetres. See `docs/decisions/0242`.
    pub max_trunk_diameter_mm: f32,
    /// The longest a leaning strut of a merge runs, millimetres: a longer one sways on the
    /// peel, so its tips stand on trunks of their own. See `docs/decisions/0338`.
    #[serde(default = "default_max_strut_length_mm")]
    pub max_strut_length_mm: f32,
    /// The most tips one trunk carries before its neighbours get trunks of their own.
    #[serde(default = "default_max_tips_per_trunk")]
    pub max_tips_per_trunk: u32,
}

pub(super) fn default_max_strut_length_mm() -> f32 {
    6.0
}

pub(super) fn default_max_tips_per_trunk() -> u32 {
    6
}

impl Default for Branching {
    fn default() -> Self {
        Self {
            enabled: true,
            max_angle_deg: 45.0,
            max_merge_distance_mm: 12.0,
            trunk_exponent: 2.5,
            max_trunk_diameter_mm: 3.0,
            max_strut_length_mm: default_max_strut_length_mm(),
            max_tips_per_trunk: default_max_tips_per_trunk(),
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

    /// Radius of a trunk that takes on `a` and `b`, millimetres.
    pub fn merged_radius_mm(&self, a: f32, b: f32) -> f32 {
        let power = self.trunk_exponent.max(1.0);
        let merged = a.powf(power) + b.powf(power);
        merged
            .powf(1.0 / power)
            .min(self.max_trunk_diameter_mm / 2.0)
            .max(a.max(b))
    }
}
