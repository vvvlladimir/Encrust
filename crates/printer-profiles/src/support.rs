use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ProfileError;

mod bracing;
mod branching;
mod raft;
mod segments;
mod validate;
pub use bracing::{BracePattern, BraceZone, Bracing, BracingPreset, Grid, IslandSpacing};
pub use branching::Branching;
use branching::TRUNK_SLENDERNESS;
pub use raft::{Raft, RaftShape, RaftStyle};
pub use segments::{
    BottomSegment, ContactShape, MiddleSegment, PlatformShape, SmallPillar, TipSegment, TopSegment,
};

/// Shape of one support, from the tip on the model down to the foot on the plate, and how several
/// merge on the way down. See `docs/design/supports.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportProfile {
    #[serde(default)]
    pub name: String,
    /// Sides of the polygon a pillar is drawn as. Fewer is cheaper to slice, more is
    /// rounder.
    #[serde(default = "default_facets")]
    pub facets: u32,
    /// How many supports automatic placement puts down, against what it reckons is
    /// enough. One is that reckoning, two is twice as many, a half is half as many.
    #[serde(default = "default_density")]
    pub density: f32,
    /// How far a surface may lean before it needs holding up, degrees from vertical.
    /// Zero is a wall, ninety a ceiling; anything leaning further than this is supported.
    #[serde(default = "default_max_overhang_deg")]
    pub max_overhang_deg: f32,
    /// How far apart automatic placement puts contacts under a face looking straight
    /// down, millimetres, before `density` scales it. See `docs/decisions/0131`.
    #[serde(default = "default_contact_spacing_mm")]
    pub contact_spacing_mm: f32,
    /// How far apart they go under a face leaning just past `max_overhang_deg`, mm; between, it
    /// follows the face's angle, never under `contact_spacing_mm` (`docs/decisions/0324`).
    #[serde(default = "default_steep_contact_spacing_mm")]
    pub steep_contact_spacing_mm: f32,
    /// How much clear air a support's body keeps between itself and the model,
    /// millimetres. Anything closer than this cures onto the part.
    #[serde(default = "default_clearance_mm")]
    pub clearance_mm: f32,
    #[serde(default)]
    pub tip: TipSegment,
    #[serde(default)]
    pub top: TopSegment,
    #[serde(default)]
    pub middle: MiddleSegment,
    #[serde(default)]
    pub bottom: BottomSegment,
    /// Whether a support may stand on the model at all. Off, one with the model under it
    /// leans aside for the plate instead of resting on the part.
    #[serde(default = "default_land_on_model")]
    pub land_on_model: bool,
    #[serde(default)]
    pub small_pillar: SmallPillar,
    #[serde(default)]
    pub branching: Branching,
    #[serde(default)]
    pub raft: Raft,
    #[serde(default)]
    pub bracing: Bracing,
    #[serde(default)]
    pub grid: Grid,
    #[serde(default)]
    pub islands: IslandSpacing,
    /// How far the lowest point of a part should stand clear of the plate, mm: supports need room
    /// under the model, and a part printed straight onto the plate sticks hardest.
    #[serde(default = "default_z_lift_mm")]
    pub z_lift_mm: f32,
}

/// How far a part is lifted off the plate before it is supported; see ADR 0131.
fn default_z_lift_mm() -> f32 {
    2.0
}

/// A support standing on the part is what reaches an undercut no column can, and its
/// contact is as small as a tip's, so it is on by default.
fn default_land_on_model() -> bool {
    true
}

/// How much clear air a support's body keeps between itself and the model; see ADR 0131.
fn default_clearance_mm() -> f32 {
    0.6
}

/// Enough supports, by the measurements the spacings are derived from.
fn default_density() -> f32 {
    1.0
}

/// How far apart a lattice of contacts is spaced; see ADR 0131.
fn default_contact_spacing_mm() -> f32 {
    4.0
}

/// How far apart contacts go under a face that barely needs them; see ADR 0324.
fn default_steep_contact_spacing_mm() -> f32 {
    3.3
}

/// A ceiling, in the degrees-from-vertical the overhang angle is measured in.
const RIGHT_ANGLE_DEG: f32 = 90.0;

/// The angle resin holds up on its own before a surface starts to sag, the default every
/// slicer ships.
fn default_max_overhang_deg() -> f32 {
    45.0
}

/// Twenty-four sides put the flats about 0.013 mm inside the circle of the heaviest
/// preset's 3 mm pillar, and read as round on the screen; see ADR 0343.
fn default_facets() -> u32 {
    24
}

/// The fewest sides that still enclose an area.
const MIN_FACETS: u32 = 3;

impl SupportProfile {
    /// The support profile this build ships under `id`, as its file in `assets/profiles/supports/`
    /// says, whatever the user's copy says; `None` if not shipped.
    pub fn shipped(id: &str) -> Option<Self> {
        crate::catalogue::shipped_support(id)
    }

    /// A profile that says nothing, every field at its own default: what a file of no
    /// fields reads as.
    fn unstated() -> Self {
        Self {
            name: String::new(),
            facets: default_facets(),
            density: default_density(),
            max_overhang_deg: default_max_overhang_deg(),
            contact_spacing_mm: default_contact_spacing_mm(),
            steep_contact_spacing_mm: default_steep_contact_spacing_mm(),
            clearance_mm: default_clearance_mm(),
            tip: TipSegment::default(),
            top: TopSegment::default(),
            middle: MiddleSegment::default(),
            bottom: BottomSegment::default(),
            land_on_model: default_land_on_model(),
            small_pillar: SmallPillar::default(),
            branching: Branching::default(),
            raft: Raft::default(),
            bracing: Bracing::default(),
            grid: Grid::default(),
            islands: IslandSpacing::default(),
            z_lift_mm: default_z_lift_mm(),
        }
    }

    pub fn contact_radius_mm(&self) -> f32 {
        self.tip.contact_diameter_mm / 2.0
    }



    pub fn pillar_radius_mm(&self) -> f32 {
        self.middle.diameter_mm / 2.0
    }

    pub fn base_radius_mm(&self) -> f32 {
        self.bottom.platform_diameter_mm / 2.0
    }


    /// How high a support standing on the plate starts, millimetres: the top of its pad,
    /// or of the raft that takes the pad's place. See `docs/decisions/0266`.
    pub fn footing_height_mm(&self) -> f32 {
        if self.raft.enabled {
            self.raft.thickness_mm
        } else {
            self.bottom.platform_thickness_mm
        }
    }

    /// How far the lowest point of a part stands off the plate, mm: the profile's lift plus a
    /// raft's thickness, so the slab is never printed into the part (`docs/decisions/0238`).
    pub fn lift_mm(&self) -> f32 {
        if self.raft.enabled {
            self.z_lift_mm + self.raft.thickness_mm
        } else {
            self.z_lift_mm
        }
    }

    /// Radius a trunk `height_mm` tall carrying `tips` widens to at its base, mm: as height
    /// and load ask, within the cap and pillar width (`docs/decisions/0242`).
    pub fn trunk_base_radius_mm(&self, height_mm: f32, tips: usize) -> f32 {
        let pillar = self.pillar_radius_mm();
        let carried = (1..tips).fold(pillar, |radius, _| {
            self.branching.merged_radius_mm(radius, pillar)
        });
        let slender = height_mm.max(0.0) / TRUNK_SLENDERNESS / 2.0;
        carried
            .max(slender)
            .min(self.branching.max_trunk_diameter_mm / 2.0)
            .max(pillar)
    }

    pub fn flare_lower_radius_mm(&self) -> f32 {
        self.bottom.lower_diameter_mm / 2.0
    }

    /// How far a support's lower end sinks into whatever it stands on, millimetres.
    pub fn landing_depth_mm(&self) -> f32 {
        if self.small_pillar.enabled {
            self.small_pillar.lower_depth_mm
        } else {
            self.tip.contact_depth_mm
        }
    }

    /// Writes the profile back out as TOML.
    pub fn save(&self, path: &Path) -> Result<(), ProfileError> {
        let source = self.to_toml_string(path)?;
        std::fs::write(path, source).map_err(|source| ProfileError::Io {
            path: path.to_owned(),
            source,
        })
    }

    /// The profile as the TOML it is kept in; `path` is where it is going, for the error.
    pub fn to_toml_string(&self, path: &Path) -> Result<String, ProfileError> {
        toml::to_string_pretty(self).map_err(|source| ProfileError::Serialise {
            path: path.to_owned(),
            source,
        })
    }

    pub fn from_toml_str(source: &str, path: &Path) -> Result<Self, ProfileError> {
        let profile: Self = toml::from_str(source).map_err(|source| ProfileError::Parse {
            path: path.to_owned(),
            source,
        })?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn load(path: &Path) -> Result<Self, ProfileError> {
        let source = std::fs::read_to_string(path).map_err(|source| ProfileError::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::from_toml_str(&source, path)
    }
}

/// The id of the support profile a new object starts with.
pub const STOCK_SUPPORT: &str = "medium";

/// The shipped [`STOCK_SUPPORT`] as its file says; a profile of the fields' own defaults
/// should that file not read, which a test of the shipped files rules out.
impl Default for SupportProfile {
    fn default() -> Self {
        Self::shipped(STOCK_SUPPORT).unwrap_or_else(Self::unstated)
    }
}

#[cfg(test)]
mod tests;
