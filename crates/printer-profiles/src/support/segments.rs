use serde::{Deserialize, Serialize};

/// What the very tip of a support is shaped like where it meets the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContactShape {
    /// A ball at the tip. More contact area, so a firmer hold on a heavy overhang, at
    /// the cost of a wider mark to sand off.
    Sphere,
    /// A flat disc, the whole tip pushed its bite into the surface; see
    /// `docs/decisions/0243`.
    #[default]
    Plane,
}

/// The disc that meets the model and whatever bites into it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TipSegment {
    #[serde(default)]
    pub shape: ContactShape,
    /// Diameter of the disc that meets the model, millimetres.
    pub contact_diameter_mm: f32,
    /// How far the tip sinks into the model, millimetres. Zero meets the surface and
    /// relies on the layer above curing into it; see `docs/decisions/0323`.
    pub contact_depth_mm: f32,
}

/// What a profile that names no tip meets the model with: the field's 0.4 mm plane
/// (ADR 0131).
impl Default for TipSegment {
    fn default() -> Self {
        Self {
            shape: ContactShape::Plane,
            contact_diameter_mm: 0.4,
            contact_depth_mm: 0.2,
        }
    }
}

/// The cone between tip and pillar: as wide as the contact at the model and the pillar where it
/// meets it, so it has only a length. See `docs/decisions/0238`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TopSegment {
    /// Length of the segment along its own axis, millimetres. On a branch that axis
    /// leans, so this is not a height.
    pub length_mm: f32,
}

impl Default for TopSegment {
    fn default() -> Self {
        Self { length_mm: 2.0 }
    }
}

/// The pillar body, or the trunk once branches have merged into it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MiddleSegment {
    /// Diameter of the pillar body, millimetres.
    pub diameter_mm: f32,
}

impl Default for MiddleSegment {
    fn default() -> Self {
        Self { diameter_mm: 1.2 }
    }
}

/// What the foot on the plate is shaped like: a prism about the support's axis with a number of
/// sides and optional taper; `platform_diameter_mm` is the circle its corners sit on.
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
pub(super) const PRISM_SIDES: u32 = 6;

pub(super) const CUBE_SIDES: u32 = 4;

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
    /// Diameter where the flare meets the platform, mm. It leaves the pillar at its width, so
    /// this sets its height too; see `docs/decisions/0130`.
    #[serde(default = "default_bottom_lower_diameter_mm")]
    pub lower_diameter_mm: f32,
}

impl Default for BottomSegment {
    fn default() -> Self {
        Self {
            shape: PlatformShape::Cylinder,
            platform_diameter_mm: 8.0,
            platform_thickness_mm: 1.0,
            lower_diameter_mm: default_bottom_lower_diameter_mm(),
        }
    }
}

/// What a pillar of about a millimetre flares out to where it meets its pad; see ADR 0131.
pub(super) fn default_bottom_lower_diameter_mm() -> f32 {
    2.2
}

/// The thin strut where a support runs from one part of the model to another, both ends sunk in
/// a surface, so it needs no foot or column width.
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
    /// The longest run from contact to landing that is thinned down, millimetres: a
    /// longer one keeps the regular pillar's width, since a long thin strut bends.
    #[serde(default = "default_small_pillar_max_length_mm")]
    pub max_length_mm: f32,
}

impl Default for SmallPillar {
    fn default() -> Self {
        Self {
            enabled: true,
            diameter_mm: 0.5,
            upper_depth_mm: 0.2,
            lower_depth_mm: 0.2,
            max_length_mm: default_small_pillar_max_length_mm(),
        }
    }
}

/// Long enough for the gap under a ledge or between two limbs, short of a strut that
/// would sway on the peel.
pub(super) fn default_small_pillar_max_length_mm() -> f32 {
    10.0
}

impl SmallPillar {
    pub fn radius_mm(&self) -> f32 {
        self.diameter_mm / 2.0
    }
}
