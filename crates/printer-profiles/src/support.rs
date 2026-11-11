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
pub(crate) use validate::overhang_within_right_angle;

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

/// How many times its tips' own section a strut or trunk carrying them is built to, so a tip
/// tears at the part before anything under it breaks (`docs/decisions/0424`).
pub const CARRYING_MARGIN: f32 = 1.5;

/// How many times its own width a leaning strut may run: 6 mm at 1.2 mm, the longest strut
/// shipped since fans of longer ones swayed (`docs/decisions/0338`, `0424`).
pub const STRUT_SLENDERNESS: f32 = 5.0;

/// The radius a body `length_mm` long needs to be no slenderer than `slenderness`, mm.
fn slender_radius_mm(length_mm: f32, slenderness: f32) -> f32 {
    length_mm.max(0.0) / slenderness / 2.0
}

/// A ceiling, in the degrees-from-vertical the overhang angle is measured in.
const RIGHT_ANGLE_DEG: f32 = 90.0;

/// The angle resin holds up on its own before a surface starts to sag, the default every
/// slicer ships.
fn default_max_overhang_deg() -> f32 {
    45.0
}

/// Twelve sides, shaded smooth, read as round on the screen, as every shipped preset has;
/// see ADR 0341.
fn default_facets() -> u32 {
    12
}

/// The fewest sides that still enclose an area.
const MIN_FACETS: u32 = 3;

impl SupportProfile {
    /// The support profile this build ships under `id`, as its file in `assets/profiles/supports/`
    /// says, whatever the user's copy says.
    pub fn shipped(id: &str) -> Result<Self, ProfileError> {
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

    /// How far apart supports stand on a flat underside, millimetres, density taken in.
    pub fn spacing_mm(&self) -> f32 {
        self.contact_spacing_mm / self.density.max(1e-3).sqrt()
    }

    /// The tip's radius where it crosses the model's surface, millimetres: what the peel
    /// pulls through, wider than the contact where a plane tip bites in (ADR 0243).
    pub fn surface_radius_mm(&self) -> f32 {
        let contact = self.contact_radius_mm();
        match self.tip.shape {
            ContactShape::Plane => {
                let top = self.top.length_mm.max(1e-3);
                let bite = self.tip.contact_depth_mm.min(top);
                contact + (self.pillar_radius_mm() - contact) * bite / top
            }
            ContactShape::Sphere => contact,
        }
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

    /// Radius a trunk `height_mm` tall carrying `tips` widens to at its base, mm.
    pub fn trunk_base_radius_mm(&self, height_mm: f32, tips: usize) -> f32 {
        self.carrying_radius_mm(height_mm, tips)
    }

    /// Radius a trunk `height_mm` tall carrying `tips` of this profile's tips is built to, mm:
    /// so the tip tears first, no slenderer than a column may stand, within the pillar and
    /// the widest trunk (`docs/decisions/0424`).
    pub fn carrying_radius_mm(&self, height_mm: f32, tips: usize) -> f32 {
        self.within(
            self.strong_radius_mm(tips)
                .max(slender_radius_mm(height_mm, TRUNK_SLENDERNESS)),
        )
    }

    /// Radius a leaning strut `length_mm` long carrying `tips` is built to, mm: as a trunk,
    /// but no slenderer than a strut that does not sway on the peel (`docs/decisions/0424`).
    pub fn strut_radius_mm(&self, length_mm: f32, tips: usize) -> f32 {
        self.within(self.strut_needs_mm(length_mm, tips))
    }

    /// Whether a leaning strut `length_mm` long carrying `tips` needs no more than the widest
    /// trunk: one that would is better stood on a trunk of its own.
    pub fn can_strut(&self, length_mm: f32, tips: usize) -> bool {
        self.strut_needs_mm(length_mm, tips) <= self.widest_radius_mm()
    }

    /// The most tips one trunk carries before it would outgrow the widest trunk.
    pub fn most_tips(&self) -> usize {
        let contact = self.contact_radius_mm().max(1e-3);
        let most = (self.widest_radius_mm() / contact).powi(2) / CARRYING_MARGIN;
        (most.floor() as usize).max(1)
    }

    /// The widest a trunk or strut is built, mm: the cap, never under the pillar.
    pub fn widest_radius_mm(&self) -> f32 {
        (self.branching.max_trunk_diameter_mm / 2.0).max(self.pillar_radius_mm())
    }

    /// The section `tips` ask for on their own: half again theirs, mm of radius.
    fn strong_radius_mm(&self, tips: usize) -> f32 {
        self.contact_radius_mm() * (CARRYING_MARGIN * tips as f32).sqrt()
    }

    fn strut_needs_mm(&self, length_mm: f32, tips: usize) -> f32 {
        self.strong_radius_mm(tips)
            .max(slender_radius_mm(length_mm, STRUT_SLENDERNESS))
    }

    /// `radius_mm` no thinner than the pillar and no wider than the widest trunk.
    fn within(&self, radius_mm: f32) -> f32 {
        radius_mm
            .max(self.pillar_radius_mm())
            .min(self.widest_radius_mm())
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
        Self::shipped(STOCK_SUPPORT).unwrap_or_else(|_| Self::unstated())
    }
}

#[cfg(test)]
mod tests;
