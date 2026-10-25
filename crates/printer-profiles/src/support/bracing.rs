use serde::{Deserialize, Serialize};

/// How one level of bracing ties a pair of trunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BracePattern {
    /// One brace a level, leaning the other way on the next, so the levels zigzag up.
    Zigzag,
    /// Two braces a level, crossed, so the pair is held both ways.
    #[default]
    Cross,
}

/// The levels at one end of a pair of trunks, all tied the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BraceZone {
    pub pattern: BracePattern,
    /// How many levels the zone takes, at most.
    pub levels: u32,
}

/// Legs propping a tall trunk the bracing leaves free at the top: a strut out and down, then a
/// column to the plate (`docs/decisions/0328`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Buttress {
    pub enabled: bool,
    /// How far a trunk may stand above its highest brace, millimetres, before it gets a
    /// pair of buttresses.
    pub pair_above_mm: f32,
    /// How far it may stand above its second-highest, to a second neighbour, before it
    /// gets one.
    pub one_above_mm: f32,
}

impl Default for Buttress {
    fn default() -> Self {
        Self {
            enabled: false,
            pair_above_mm: 30.0,
            one_above_mm: 60.0,
        }
    }
}

/// Struts tying tall trunks together so neither sways on the peel. A pair's levels fall in three
/// zones, `bottom`, `middle` and `top`; see `docs/decisions/0328`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bracing {
    pub enabled: bool,
    /// Diameter of one brace, millimetres.
    pub diameter_mm: f32,
    /// How far apart two trunks may be and still be worth tying together, millimetres.
    pub max_spacing_mm: f32,
    /// How far up a trunk the first brace goes, millimetres.
    pub start_height_mm: f32,
    /// How far apart the braces up one pair of trunks are, millimetres.
    pub rise_mm: f32,
    #[serde(default)]
    pub bottom: BraceZone,
    #[serde(default)]
    pub middle: BracePattern,
    #[serde(default)]
    pub top: BraceZone,
    #[serde(default)]
    pub buttress: Buttress,
}

impl Default for Bracing {
    fn default() -> Self {
        Self {
            enabled: false,
            diameter_mm: 0.8,
            max_spacing_mm: 12.0,
            start_height_mm: 8.0,
            rise_mm: 10.0,
            bottom: BraceZone::default(),
            middle: BracePattern::default(),
            top: BraceZone::default(),
            buttress: Buttress::default(),
        }
    }
}

impl Bracing {
    pub fn radius_mm(&self) -> f32 {
        self.diameter_mm / 2.0
    }
}

/// Trunks stood on a square grid on the plate, tips leant to their node at 45 degrees. Off, trunks
/// stand where the tips merge. See `docs/decisions/0333`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub enabled: bool,
    /// Distance between two neighbouring nodes, millimetres.
    pub spacing_mm: f32,
}

impl Grid {
    /// The narrowest and widest a grid may be set, millimetres.
    pub const SPACING_MM: std::ops::RangeInclusive<f32> = 1.0..=15.0;
}

/// How closely automatic placement holds an island against the contact spacing, by the
/// island's area: a small one hangs on the film alone, with no edge of a part to steady it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IslandSpacing {
    /// The least island, mm² on the layer it begins on, held at the second share and at
    /// the third.
    pub area_mm2: [f32; 2],
    /// The share of the contact spacing an island of each size is held at, smallest first.
    pub share: [f32; 3],
}

/// What a profile saved before islands were spaced by it is read with; the shipped
/// profiles say it themselves.
impl Default for IslandSpacing {
    fn default() -> Self {
        Self {
            area_mm2: [2.4, 3.2],
            share: [13.0 / 18.0, 15.0 / 18.0, 1.0],
        }
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            enabled: false,
            spacing_mm: 5.0,
        }
    }
}

/// A ready set of bracing numbers to start from. A profile keeps the numbers, never the
/// name, so one is recognised by its numbers alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracingPreset {
    /// Zigzags, crossed once at the top.
    Standard,
    /// Crossed all the way up.
    Strong,
    /// Zigzags between two crossed levels at the top and one at the bottom, the braces
    /// as thick as the trunks allow.
    Tall,
    /// Crossed all the way up, the braces thin.
    Small,
}

impl BracingPreset {
    pub const ALL: [Self; 4] = [Self::Standard, Self::Strong, Self::Tall, Self::Small];

    /// The preset's numbers, switched on.
    pub fn bracing(self) -> Bracing {
        use BracePattern::{Cross, Zigzag};
        let zone = |pattern, levels| BraceZone { pattern, levels };
        let (diameter_mm, start_height_mm, bottom, middle, top) = match self {
            Self::Standard => (0.9, 1.0, zone(Zigzag, 1), Zigzag, zone(Cross, 1)),
            Self::Strong => (1.0, 0.0, zone(Cross, 1), Cross, zone(Cross, 1)),
            Self::Tall => (2.0, 1.0, zone(Cross, 1), Zigzag, zone(Cross, 2)),
            Self::Small => (0.8, 1.0, zone(Cross, 1), Cross, zone(Cross, 1)),
        };
        let (pair_above_mm, one_above_mm) = match self {
            Self::Standard => (30.0, 60.0),
            Self::Strong => (30.0, 50.0),
            Self::Tall => (20.0, 50.0),
            Self::Small => (10.0, 30.0),
        };
        Bracing {
            enabled: true,
            diameter_mm,
            max_spacing_mm: 10.0,
            start_height_mm,
            rise_mm: 5.0,
            bottom,
            middle,
            top,
            buttress: Buttress {
                enabled: true,
                pair_above_mm,
                one_above_mm,
            },
        }
    }

    /// The preset `bracing` holds the numbers of, switched on or not.
    pub fn of(bracing: &Bracing) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| {
            let numbers = Bracing {
                enabled: bracing.enabled,
                ..preset.bracing()
            };
            numbers == *bracing
        })
    }
}
