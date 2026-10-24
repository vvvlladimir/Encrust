use serde::{Deserialize, Serialize};

/// What outline a raft is drawn to: the part's own footprint and every foot under it,
/// pushed out by `margin_mm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RaftShape {
    /// The convex hull of that footprint. The least resin for the grip.
    #[default]
    Hull,
    /// The smallest rectangle around it. Peels more evenly, costs more resin.
    Rectangle,
}

/// What a raft is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RaftStyle {
    /// A slab under the whole part whose sloped wall carries on up past its top into a
    /// lip around the edge, like a saucer.
    #[default]
    Skates,
    /// A slab under the whole part, its inside a grid of bars with square holes between.
    /// Less resin and less suction on the plate.
    Grid,
    /// A pad under each foot, joined by strips along the shortest network through them.
    Connected,
}

impl RaftStyle {
    /// Whether the style covers an outline, which then has a [`RaftShape`].
    pub fn has_outline(self) -> bool {
        matches!(self, Self::Skates | Self::Grid)
    }
}

/// The slab the supports stand on instead of the plate, spreading the peel force over one
/// footprint and coming off the plate in one piece.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Raft {
    pub enabled: bool,
    #[serde(default)]
    pub style: RaftStyle,
    #[serde(default)]
    pub shape: RaftShape,
    /// How far the raft reaches past the part's footprint, or past a foot's own edge for
    /// a style without an outline, millimetres.
    #[serde(default = "default_raft_margin_mm")]
    pub margin_mm: f32,
    /// How thick the slab is, millimetres.
    pub thickness_mm: f32,
    /// How far its walls lean out on the way down, degrees from vertical. A raft that
    /// meets the plate wider than it ends grips harder and still releases.
    pub slope_deg: f32,
}

impl Default for Raft {
    fn default() -> Self {
        Self {
            enabled: false,
            style: RaftStyle::Skates,
            shape: RaftShape::Hull,
            margin_mm: default_raft_margin_mm(),
            thickness_mm: 1.5,
            slope_deg: 30.0,
        }
    }
}

/// How far a raft wall may lean before it stops being a wall.
pub(super) const MAX_RAFT_SLOPE_DEG: f32 = 75.0;

impl Raft {
    /// How much wider the bottom of the slab is than its top, millimetres.
    pub fn overhang_mm(&self) -> f32 {
        self.thickness_mm
            * self
                .slope_deg
                .clamp(0.0, MAX_RAFT_SLOPE_DEG)
                .to_radians()
                .tan()
    }
}

/// How far a raft reaches past what it stands under; see ADR 0241.
pub(super) fn default_raft_margin_mm() -> f32 {
    2.0
}
