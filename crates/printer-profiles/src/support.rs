use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ProfileError;

/// What the very tip of a support is shaped like where it meets the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContactShape {
    /// A point driven into the surface. The smallest scar, and the weakest grip.
    #[default]
    Cone,
    /// A ball at the tip. More contact area, so a firmer hold on a heavy overhang, at
    /// the cost of a wider mark to sand off.
    Sphere,
    /// A flat disc pressed against the surface, biting nothing. For a face that must not
    /// be pierced at all.
    Plane,
}

/// The disc that meets the model and whatever bites into it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TipSegment {
    #[serde(default)]
    pub shape: ContactShape,
    /// Diameter of the disc that meets the model, millimetres.
    pub contact_diameter_mm: f32,
    /// How far the tip sinks into the model, millimetres. Without it the support and the
    /// model cure as two touching solids and the support falls off during the print.
    pub contact_depth_mm: f32,
}

/// The frustum between the tip and the pillar, and the only part of a column whose two
/// ends have different widths by design.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TopSegment {
    /// Diameter where the segment leaves the contact, millimetres.
    pub upper_diameter_mm: f32,
    /// Diameter where it meets the pillar, millimetres.
    pub lower_diameter_mm: f32,
    /// Length of the segment along its own axis, millimetres. On a branch that axis
    /// leans, so this is not a height.
    pub length_mm: f32,
}

/// The pillar body, or the trunk once branches have merged into it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MiddleSegment {
    /// Diameter of the pillar body, millimetres.
    pub diameter_mm: f32,
}

/// What the foot that stands on the plate is shaped like.
///
/// A foot is drawn as a prism about the support's own axis, so the shape is the number of
/// sides it has and whether it tapers. `platform_diameter_mm` is the circle those corners
/// sit on, so a cube's sides are that diameter over the square root of two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlatformShape {
    /// A round pad. The most plate contact for its diameter.
    #[default]
    Cylinder,
    /// A pad that narrows towards the support, so a blade can get under its rim.
    Cone,
    /// Six sides.
    Prism,
    /// Four sides.
    Cube,
    /// An elongated pad cocked up at one end, so a blade gets under it. The one foot
    /// that is not a solid of revolution; see `docs/decisions/0043`.
    Skate,
}

/// Sides of a round foot, before the profile's own facet count is taken into account.
const PRISM_SIDES: u32 = 6;
const CUBE_SIDES: u32 = 4;

impl PlatformShape {
    /// How many sides the foot is drawn with, given the profile's facet count.
    pub fn sides(self, facets: u32) -> u32 {
        match self {
            Self::Cylinder | Self::Cone => facets,
            Self::Prism => PRISM_SIDES,
            Self::Cube | Self::Skate => CUBE_SIDES,
        }
    }

    /// Whether the foot is swept as rings about the support's own axis at all.
    pub fn is_round(self) -> bool {
        !matches!(self, Self::Skate)
    }

    /// Whether the foot narrows on the way up to the support it carries.
    pub fn tapers(self) -> bool {
        matches!(self, Self::Cone)
    }
}

/// The foot a column that reaches the plate stands on, and the flare that widens the
/// pillar out into it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BottomSegment {
    #[serde(default)]
    pub shape: PlatformShape,
    /// Diameter of the circle the foot's corners sit on, millimetres.
    pub platform_diameter_mm: f32,
    /// Height of the foot, millimetres.
    pub platform_thickness_mm: f32,
    /// Diameter where the flare leaves the pillar, millimetres.
    #[serde(default = "default_bottom_upper_diameter_mm")]
    pub upper_diameter_mm: f32,
    /// Diameter where the flare meets the platform, millimetres. The flare rises as far
    /// as it widens, so this sets its height as well; see `docs/decisions/0130`.
    #[serde(default = "default_bottom_lower_diameter_mm")]
    pub lower_diameter_mm: f32,
}

/// What a pillar of about a millimetre flares out to where it meets its pad; see ADR 0131.
fn default_bottom_upper_diameter_mm() -> f32 {
    1.0
}

fn default_bottom_lower_diameter_mm() -> f32 {
    2.2
}

/// The thin strut used where a support runs from one part of the model to another rather
/// than down to the plate.
///
/// Both of its ends sink into a surface, so it needs neither a foot nor the width a
/// column standing on the plate needs. What MSLA slicers call a small pillar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SmallPillar {
    /// Whether a support that lands on the model is thinned down at all.
    pub enabled: bool,
    /// Diameter of the strut, millimetres.
    pub diameter_mm: f32,
    /// How far its upper end sinks into the surface it holds, millimetres.
    pub upper_depth_mm: f32,
    /// How far its lower end sinks into the surface it stands on, millimetres.
    pub lower_depth_mm: f32,
}

impl Default for SmallPillar {
    fn default() -> Self {
        Self {
            enabled: true,
            diameter_mm: 0.5,
            upper_depth_mm: 0.2,
            lower_depth_mm: 0.2,
        }
    }
}

impl SmallPillar {
    pub fn radius_mm(&self) -> f32 {
        self.diameter_mm / 2.0
    }
}

/// How far a strut may lean from vertical, degrees. Past a right angle it climbs rather
/// than descends and the merge solve stops having an answer.
const MAX_BRANCH_ANGLE_DEG: f32 = 80.0;

/// Below this a branch is vertical and nothing ever merges.
const MIN_BRANCH_ANGLE_DEG: f32 = 1.0;

/// How the tips of nearby supports merge into shared trunks.
///
/// The measurements of *Clever Support: Efficient Support Structure Generation for
/// Digital Fabrication*; see `docs/decisions/0040`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Branching {
    /// Whether tips merge at all. Off is the vertical column of step 7a.
    pub enabled: bool,
    /// How far a strut may lean from vertical, degrees. This is the one knob that lets a
    /// support be anything other than a vertical column.
    pub max_angle_deg: f32,
    /// How far apart two tips may be and still be worth merging, millimetres.
    pub max_merge_distance_mm: f32,
    /// How a trunk thickens as it takes on another branch: the radii of what it carries
    /// are combined as the root of the sum of their powers. Two conserves cross-section,
    /// three conserves volume; between them is what tree supports are usually drawn with.
    pub trunk_exponent: f32,
    /// However much a trunk carries, it never grows past this diameter, millimetres.
    pub max_trunk_diameter_mm: f32,
}

impl Default for Branching {
    fn default() -> Self {
        Self {
            enabled: true,
            max_angle_deg: 45.0,
            max_merge_distance_mm: 12.0,
            trunk_exponent: 2.5,
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

/// What footprint a raft covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RaftShape {
    /// The convex hull of the feet standing on it. The least resin for the grip.
    #[default]
    Hull,
    /// The bounding rectangle of those feet. Peels more evenly, costs more resin.
    Rectangle,
}

/// The slab the supports stand on instead of standing on the plate.
///
/// A raft spreads the peel force over one footprint rather than over every foot, and
/// comes off the plate in one piece.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Raft {
    pub enabled: bool,
    #[serde(default)]
    pub shape: RaftShape,
    /// How much wider than the feet it covers the raft is drawn, as a proportion of
    /// their own footprint. One is the footprint itself.
    pub area_ratio: f32,
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
            shape: RaftShape::Hull,
            area_ratio: 1.2,
            thickness_mm: 1.5,
            slope_deg: 30.0,
        }
    }
}

/// How far a raft wall may lean before it stops being a wall.
const MAX_RAFT_SLOPE_DEG: f32 = 75.0;

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

    /// How far out the footprint is pushed to reach `area_ratio` of its own area.
    pub fn spread(&self) -> f32 {
        self.area_ratio.max(1.0).sqrt()
    }
}

/// Struts tying tall trunks to each other, so that neither can sway on the peel.
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
}

impl Default for Bracing {
    fn default() -> Self {
        Self {
            enabled: false,
            diameter_mm: 0.8,
            max_spacing_mm: 12.0,
            start_height_mm: 8.0,
            rise_mm: 10.0,
        }
    }
}

impl Bracing {
    pub fn radius_mm(&self) -> f32 {
        self.diameter_mm / 2.0
    }
}

/// Shape of one support, from the tip that touches the model down to the foot that stands
/// on the plate, and how several of them merge on the way down.
///
/// The sections follow what MSLA slicers call these measurements, so a user who has tuned
/// numbers in another slicer can carry them across. See `docs/design/supports.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportProfile {
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
    /// How far apart automatic placement puts contacts over an overhang, millimetres,
    /// before `density` scales it. See `docs/decisions/0131`.
    #[serde(default = "default_contact_spacing_mm")]
    pub contact_spacing_mm: f32,
    /// How much clear air a support's body keeps between itself and the model,
    /// millimetres. Anything closer than this cures onto the part.
    #[serde(default = "default_clearance_mm")]
    pub clearance_mm: f32,
    pub tip: TipSegment,
    pub top: TopSegment,
    pub middle: MiddleSegment,
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
    /// How far the lowest point of a part should stand clear of the plate, millimetres.
    /// Supports need room under the model, and the first layers of a part printed
    /// straight onto the plate are the ones that stick to it hardest.
    #[serde(default = "default_z_lift_mm")]
    pub z_lift_mm: f32,
}

/// How far a part is lifted off the plate before it is supported; see ADR 0131.
fn default_z_lift_mm() -> f32 {
    2.0
}

/// The raft every shipped preset carries: off, with the measurements of ADR 0131 when it
/// is turned on.
fn shipped_raft(shape: RaftShape) -> Raft {
    Raft {
        enabled: false,
        shape,
        area_ratio: 1.1,
        thickness_mm: 0.5,
        slope_deg: 30.0,
    }
}

/// The cross bracing every shipped preset carries, left on; see ADR 0131.
fn shipped_bracing() -> Bracing {
    Bracing {
        enabled: true,
        diameter_mm: 0.8,
        max_spacing_mm: 30.0,
        start_height_mm: 3.0,
        rise_mm: 2.0,
    }
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

/// A ceiling, in the degrees-from-vertical the overhang angle is measured in.
const RIGHT_ANGLE_DEG: f32 = 90.0;

/// The angle resin holds up on its own before a surface starts to sag, the default every
/// slicer ships.
fn default_max_overhang_deg() -> f32 {
    45.0
}

/// Twelve sides put the flats about 0.026 mm inside the circle for the thickest preset's
/// 1.5 mm pillar, which is finer than one pixel of a 9K panel.
fn default_facets() -> u32 {
    12
}

/// The fewest sides that still enclose an area.
const MIN_FACETS: u32 = 3;

impl SupportProfile {
    /// Thin supports for small, well-anchored parts. Easy to cut off, easy to snap.
    pub fn light() -> Self {
        Self {
            name: "Light".to_owned(),
            facets: default_facets(),
            density: 0.6,
            max_overhang_deg: 55.0,
            contact_spacing_mm: default_contact_spacing_mm(),
            clearance_mm: default_clearance_mm(),
            tip: TipSegment {
                shape: ContactShape::Cone,
                contact_diameter_mm: 0.5,
                contact_depth_mm: 0.3,
            },
            // The pad, the flare and the top segment together stand inside the 2 mm
            // lift this profile asks for; see `docs/design/supports.md`.
            top: TopSegment {
                upper_diameter_mm: 0.5,
                lower_diameter_mm: 0.9,
                length_mm: 1.0,
            },
            middle: MiddleSegment { diameter_mm: 0.9 },
            bottom: BottomSegment {
                shape: PlatformShape::Cylinder,
                platform_diameter_mm: 7.0,
                platform_thickness_mm: 0.5,
                upper_diameter_mm: 0.9,
                lower_diameter_mm: 1.5,
            },
            land_on_model: default_land_on_model(),
            small_pillar: SmallPillar {
                enabled: true,
                diameter_mm: 0.3,
                upper_depth_mm: 0.15,
                lower_depth_mm: 0.2,
            },
            branching: Branching {
                max_angle_deg: 50.0,
                max_merge_distance_mm: 14.0,
                max_trunk_diameter_mm: 1.0,
                ..Branching::default()
            },
            raft: shipped_raft(RaftShape::Hull),
            bracing: Bracing {
                enabled: true,
                diameter_mm: 0.8,
                max_spacing_mm: 8.0,
                start_height_mm: 8.0,
                rise_mm: 8.0,
            },
            z_lift_mm: default_z_lift_mm(),
        }
    }

    /// The everyday choice, and what a new object starts with.
    pub fn medium() -> Self {
        Self {
            name: "Medium".to_owned(),
            facets: default_facets(),
            density: default_density(),
            max_overhang_deg: default_max_overhang_deg(),
            contact_spacing_mm: default_contact_spacing_mm(),
            clearance_mm: default_clearance_mm(),
            tip: TipSegment {
                shape: ContactShape::Cone,
                contact_diameter_mm: 0.8,
                contact_depth_mm: 0.4,
            },
            top: TopSegment {
                upper_diameter_mm: 0.55,
                lower_diameter_mm: 1.2,
                length_mm: 2.0,
            },
            middle: MiddleSegment { diameter_mm: 1.2 },
            bottom: BottomSegment {
                shape: PlatformShape::Cylinder,
                platform_diameter_mm: 12.0,
                platform_thickness_mm: 1.0,
                upper_diameter_mm: 1.2,
                lower_diameter_mm: 2.2,
            },
            land_on_model: default_land_on_model(),
            small_pillar: SmallPillar {
                enabled: true,
                diameter_mm: 0.4,
                upper_depth_mm: 0.2,
                lower_depth_mm: 0.2,
            },
            branching: Branching {
                max_angle_deg: 45.0,
                max_merge_distance_mm: 8.0,
                max_trunk_diameter_mm: 2.0,
                ..Branching::default()
            },
            raft: shipped_raft(RaftShape::Hull),
            bracing: Bracing {
                enabled: false,
                ..shipped_bracing()
            },
            z_lift_mm: default_z_lift_mm(),
        }
    }

    /// Thick supports for heavy parts and large flat overhangs, at the cost of scarring.
    pub fn heavy() -> Self {
        Self {
            name: "Heavy".to_owned(),
            facets: default_facets(),
            density: 2.5,
            max_overhang_deg: 35.0,
            contact_spacing_mm: default_contact_spacing_mm(),
            clearance_mm: default_clearance_mm(),
            tip: TipSegment {
                shape: ContactShape::Sphere,
                contact_diameter_mm: 1.0,
                contact_depth_mm: 0.6,
            },
            top: TopSegment {
                upper_diameter_mm: 0.6,
                lower_diameter_mm: 1.5,
                length_mm: 3.0,
            },
            middle: MiddleSegment { diameter_mm: 1.5 },
            bottom: BottomSegment {
                shape: PlatformShape::Cylinder,
                platform_diameter_mm: 12.0,
                platform_thickness_mm: 1.0,
                upper_diameter_mm: 1.5,
                lower_diameter_mm: 2.7,
            },
            land_on_model: default_land_on_model(),
            small_pillar: SmallPillar {
                enabled: true,
                diameter_mm: 0.5,
                upper_depth_mm: 0.2,
                lower_depth_mm: 0.2,
            },
            branching: Branching {
                max_angle_deg: 70.0,
                max_merge_distance_mm: 10.0,
                max_trunk_diameter_mm: 2.5,
                ..Branching::default()
            },
            raft: shipped_raft(RaftShape::Rectangle),
            bracing: shipped_bracing(),
            z_lift_mm: default_z_lift_mm(),
        }
    }

    pub fn contact_radius_mm(&self) -> f32 {
        self.tip.contact_diameter_mm / 2.0
    }

    pub fn contact_depth_mm(&self) -> f32 {
        self.tip.contact_depth_mm
    }

    pub fn top_upper_radius_mm(&self) -> f32 {
        self.top.upper_diameter_mm / 2.0
    }

    pub fn top_lower_radius_mm(&self) -> f32 {
        self.top.lower_diameter_mm / 2.0
    }

    pub fn top_length_mm(&self) -> f32 {
        self.top.length_mm
    }

    pub fn pillar_radius_mm(&self) -> f32 {
        self.middle.diameter_mm / 2.0
    }

    pub fn base_radius_mm(&self) -> f32 {
        self.bottom.platform_diameter_mm / 2.0
    }

    pub fn base_height_mm(&self) -> f32 {
        self.bottom.platform_thickness_mm
    }

    pub fn flare_upper_radius_mm(&self) -> f32 {
        self.bottom.upper_diameter_mm / 2.0
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

    fn validate(&self) -> Result<(), ProfileError> {
        self.validate_signs()?;
        // A surface that leans past a right angle is not a surface a print can have.
        if self.max_overhang_deg > RIGHT_ANGLE_DEG {
            return Err(ProfileError::OutOfOrder {
                smaller: "max_overhang_deg",
                smaller_value: self.max_overhang_deg,
                larger: "a right angle",
                larger_value: RIGHT_ANGLE_DEG,
            });
        }
        if self.facets < MIN_FACETS {
            return Err(ProfileError::TooFew {
                field: "facets",
                minimum: MIN_FACETS,
                value: self.facets,
            });
        }
        self.validate_widths()
    }

    /// Sizes that must be above zero, then those that may be zero but not below it.
    fn validate_signs(&self) -> Result<(), ProfileError> {
        let positive = [
            ("density", self.density),
            ("max_overhang_deg", self.max_overhang_deg),
            ("contact_spacing_mm", self.contact_spacing_mm),
            ("contact_diameter_mm", self.tip.contact_diameter_mm),
            ("contact_depth_mm", self.tip.contact_depth_mm),
            ("upper_diameter_mm", self.top.upper_diameter_mm),
            ("lower_diameter_mm", self.top.lower_diameter_mm),
            ("length_mm", self.top.length_mm),
            ("diameter_mm", self.middle.diameter_mm),
            ("platform_diameter_mm", self.bottom.platform_diameter_mm),
            ("platform_thickness_mm", self.bottom.platform_thickness_mm),
            ("bottom upper_diameter_mm", self.bottom.upper_diameter_mm),
            ("bottom lower_diameter_mm", self.bottom.lower_diameter_mm),
            (
                "max_trunk_diameter_mm",
                self.branching.max_trunk_diameter_mm,
            ),
            (
                "max_merge_distance_mm",
                self.branching.max_merge_distance_mm,
            ),
        ];
        if let Some((field, value)) = positive.into_iter().find(|(_, value)| *value <= 0.0) {
            return Err(ProfileError::NonPositive { field, value });
        }

        let non_negative = [
            ("raft slope_deg", self.raft.slope_deg),
            ("bracing start_height_mm", self.bracing.start_height_mm),
            ("z_lift_mm", self.z_lift_mm),
            ("clearance_mm", self.clearance_mm),
        ];
        match non_negative.into_iter().find(|(_, value)| *value < 0.0) {
            Some((field, value)) => Err(ProfileError::NonPositive { field, value }),
            None => Ok(()),
        }
    }

    /// Every ring of a column is at least as wide as the one above it. A column that
    /// narrows downwards turns itself inside out where the two rings cross.
    fn validate_widths(&self) -> Result<(), ProfileError> {
        // The contact is not in this chain: a head wider than the neck under it is the
        // nail every slicer drives into a face, not an inversion. See
        // `docs/decisions/0131`.
        let rings = [
            ("upper_diameter_mm", self.top.upper_diameter_mm),
            ("lower_diameter_mm", self.top.lower_diameter_mm),
            ("diameter_mm", self.middle.diameter_mm),
            (
                "max_trunk_diameter_mm",
                self.branching.max_trunk_diameter_mm,
            ),
        ];
        for pair in rings.windows(2) {
            order(pair[0].0, pair[0].1, pair[1].0, pair[1].1)?;
        }
        // A thinned pillar that is thicker than the one it replaces is not a thinning.
        order(
            "small_pillar diameter_mm",
            self.small_pillar.diameter_mm,
            "diameter_mm",
            self.middle.diameter_mm,
        )?;
        order(
            "max_trunk_diameter_mm",
            self.branching.max_trunk_diameter_mm,
            "platform_diameter_mm",
            self.bottom.platform_diameter_mm,
        )?;
        // The flare stands between the pillar and the pad, so it is no narrower than the
        // one and no wider than the other.
        order(
            "bottom upper_diameter_mm",
            self.bottom.upper_diameter_mm,
            "bottom lower_diameter_mm",
            self.bottom.lower_diameter_mm,
        )?;
        order(
            "bottom lower_diameter_mm",
            self.bottom.lower_diameter_mm,
            "platform_diameter_mm",
            self.bottom.platform_diameter_mm,
        )
    }
}

fn order(
    smaller: &'static str,
    smaller_value: f32,
    larger: &'static str,
    larger_value: f32,
) -> Result<(), ProfileError> {
    if smaller_value > larger_value {
        return Err(ProfileError::OutOfOrder {
            smaller,
            smaller_value,
            larger,
            larger_value,
        });
    }
    Ok(())
}

impl Default for SupportProfile {
    fn default() -> Self {
        Self::medium()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> &'static Path {
        Path::new("inline.toml")
    }

    fn toml_of(profile: &SupportProfile) -> String {
        toml::to_string(profile).expect("serialises")
    }

    #[test]
    fn every_preset_is_valid_and_they_grow_in_order() {
        let presets = [
            SupportProfile::light(),
            SupportProfile::medium(),
            SupportProfile::heavy(),
        ];
        for preset in &presets {
            preset.validate().expect("a shipped preset is valid");
        }
        for pair in presets.windows(2) {
            assert!(
                pair[0].middle.diameter_mm < pair[1].middle.diameter_mm,
                "{} must be thinner than {}",
                pair[0].name,
                pair[1].name
            );
            assert!(pair[0].tip.contact_diameter_mm < pair[1].tip.contact_diameter_mm);
        }
    }

    #[test]
    fn default_survives_a_toml_round_trip() {
        let original = SupportProfile::default();
        let parsed: SupportProfile = toml::from_str(&toml_of(&original)).expect("parses back");
        assert_eq!(original, parsed);
    }

    #[test]
    fn a_profile_without_facets_takes_the_default() {
        let profile = SupportProfile::from_toml_str(
            r#"
name = "No facets"

[tip]
contact_diameter_mm = 0.4
contact_depth_mm = 0.2

[top]
upper_diameter_mm = 0.4
lower_diameter_mm = 2.0
length_mm = 2.0

[middle]
diameter_mm = 2.0

[bottom]
platform_diameter_mm = 8.0
platform_thickness_mm = 1.0
"#,
            path(),
        )
        .expect("valid profile");
        assert_eq!(profile.facets, 12);
        assert!((profile.density - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_profile_without_branching_still_branches_by_default() {
        let profile = SupportProfile::from_toml_str(
            r#"
name = "No branching"

[tip]
contact_diameter_mm = 0.4
contact_depth_mm = 0.2

[top]
upper_diameter_mm = 0.4
lower_diameter_mm = 2.0
length_mm = 2.0

[middle]
diameter_mm = 2.0

[bottom]
platform_diameter_mm = 8.0
platform_thickness_mm = 1.0
"#,
            path(),
        )
        .expect("valid profile");
        assert_eq!(profile.branching, Branching::default());
        assert!(profile.branching.enabled);
    }

    #[test]
    fn a_zero_contact_is_rejected() {
        let mut broken = SupportProfile::medium();
        broken.tip.contact_diameter_mm = 0.0;
        let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::NonPositive {
                field: "contact_diameter_mm",
                ..
            }
        ));
    }

    #[test]
    fn a_density_of_zero_is_rejected() {
        let mut broken = SupportProfile::medium();
        broken.density = 0.0;
        let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::NonPositive {
                field: "density",
                ..
            }
        ));
    }

    #[test]
    fn an_overhang_angle_past_a_right_angle_is_rejected() {
        let mut broken = SupportProfile::medium();
        broken.max_overhang_deg = 95.0;
        let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::OutOfOrder {
                smaller: "max_overhang_deg",
                ..
            }
        ));
    }

    #[test]
    fn a_two_sided_pillar_is_rejected() {
        let mut broken = SupportProfile::medium();
        broken.facets = 2;
        let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::TooFew {
                field: "facets",
                ..
            }
        ));
    }

    #[test]
    fn a_contact_wider_than_the_neck_under_it_is_a_nail_head_not_an_error() {
        let mut nailed = SupportProfile::medium();
        nailed.tip.contact_diameter_mm = nailed.top.upper_diameter_mm + 1.0;
        SupportProfile::from_toml_str(&toml_of(&nailed), path())
            .expect("a head wider than its neck is what every slicer drives into a face");
    }

    #[test]
    fn a_neck_wider_than_the_segment_under_it_is_rejected() {
        let mut broken = SupportProfile::medium();
        broken.top.upper_diameter_mm = broken.top.lower_diameter_mm + 1.0;
        let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::OutOfOrder {
                smaller: "upper_diameter_mm",
                ..
            }
        ));
    }

    #[test]
    fn a_flare_wider_than_the_pad_it_meets_is_rejected() {
        let mut broken = SupportProfile::medium();
        broken.bottom.lower_diameter_mm = broken.bottom.platform_diameter_mm + 1.0;
        let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::OutOfOrder {
                smaller: "bottom lower_diameter_mm",
                ..
            }
        ));
    }

    #[test]
    fn a_foot_narrower_than_the_thickest_trunk_is_rejected() {
        let mut broken = SupportProfile::medium();
        broken.bottom.platform_diameter_mm = broken.branching.max_trunk_diameter_mm - 0.5;
        let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::OutOfOrder {
                larger: "platform_diameter_mm",
                ..
            }
        ));
    }

    #[test]
    fn a_trunk_thinner_than_the_pillar_it_carries_is_rejected() {
        let mut broken = SupportProfile::medium();
        broken.branching.max_trunk_diameter_mm = broken.middle.diameter_mm - 0.1;
        let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::OutOfOrder {
                smaller: "diameter_mm",
                ..
            }
        ));
    }

    #[test]
    fn malformed_toml_reports_its_path() {
        let err = SupportProfile::from_toml_str("name = ", path()).unwrap_err();
        assert!(matches!(err, ProfileError::Parse { .. }));
    }

    #[test]
    fn the_radii_are_half_the_diameters() {
        let profile = SupportProfile::medium();
        assert!((profile.contact_radius_mm() - 0.4).abs() < f32::EPSILON);
        assert!((profile.pillar_radius_mm() - 0.6).abs() < f32::EPSILON);
        assert!((profile.base_radius_mm() - 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_trunk_is_thicker_than_either_branch_but_thinner_than_their_sum() {
        let branching = Branching::default();
        let merged = branching.merged_radius_mm(0.6, 0.6);
        assert!(
            merged > 0.6 && merged < 1.2,
            "a trunk carrying two 0.6 mm branches is between one of them and both, got \
             {merged}"
        );
    }

    #[test]
    fn a_trunk_never_grows_past_its_cap() {
        let branching = Branching::default();
        let mut radius = 0.6;
        for _ in 0..20 {
            radius = branching.merged_radius_mm(radius, 0.6);
        }
        assert!(
            (radius - branching.max_trunk_diameter_mm / 2.0).abs() < 1e-5,
            "twenty merges must stop at the cap, got {radius}"
        );
    }

    #[test]
    fn a_forty_five_degree_branch_covers_as_much_ground_as_it_drops() {
        let branching = Branching {
            max_angle_deg: 45.0,
            ..Branching::default()
        };
        assert!((branching.reach_per_drop() - 1.0).abs() < 1e-5);
    }
}
