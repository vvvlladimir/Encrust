//! What a recipe step's resolved settings ask of a tool's job; a key left out takes the
//! engine's default, never what a panel holds (`docs/design/recipes.md`).

use core_engine::{PREPARE_ORIENT, PREPARE_REPAIR};
use core_geometry::{MendMethod, Scalar, Transform};
use core_plate::{ArrangeSettings, Asked};
use core_recipe::resolved::{flag, number, points, text};
use core_recipe::{StepKind, preset_of};
use core_volume::{HoleSize, HollowMode, HollowSettings, InfillSettings, RepairMethod, plate_down};

use super::Step;
use crate::scene::ObjectId;
use crate::supports::Side;

pub(super) type Settings = core_recipe::ResolvedSettings;

/// The job one batch of a step runs, the same for every model in it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Work {
    Orient {
        wanted: Asked,
        max_tilt_deg: Option<Scalar>,
    },
    Repair(RepairMethod),
    Hollow(Hollowed),
    /// Drain's Auto, the proven search.
    DrainAuto {
        size: HoleSize,
        vent: bool,
        plugs: bool,
    },
    /// A hole at each point, `[x, y, z]` fractions of the placed box.
    DrainPoints {
        size: HoleSize,
        points: Vec<[Scalar; 3]>,
        plugs: bool,
    },
    /// Supports to the catalogue profile `profile`, `auto` the automatic one.
    Supports {
        profile: String,
        side: Side,
    },
    /// Drain's Check drainage, which `trapped` is read off.
    Scan,
    /// The stack measured as Check measures it, after `attempt` supportings of its islands.
    Check {
        attempt: u8,
    },
    Arrange {
        spacing_mm: Scalar,
    },
    Save(Saved),
}

/// A save step's settings; a name or folder left out is the window's.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Saved {
    pub folder: Option<String>,
    /// A name template, ADR 0387's tags.
    pub name: Option<String>,
    pub send: bool,
}

/// A hollow step's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Hollowed {
    pub wall_mm: Scalar,
    pub mode: HollowMode,
    pub open_floor: bool,
    pub infill: bool,
    /// The wall stays solid wherever the model is thinner than this, millimetres.
    pub solid_under_mm: Option<Scalar>,
}

/// A step kind or setting this window does not run yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NotYet(pub StepKind);

/// The models of a step that run the same work, and those of them held up inside after.
#[derive(Debug, Clone)]
pub(super) struct Batch {
    pub work: Work,
    pub ids: Vec<ObjectId>,
    pub inside: Vec<ObjectId>,
}

impl Batch {
    /// `work` on `ids`, none of them held up inside after.
    pub(super) fn of(work: Work, ids: Vec<ObjectId>) -> Self {
        Self {
            work,
            ids,
            inside: Vec::new(),
        }
    }
}

impl Work {
    /// What step `kind` asks with `settings`.
    pub(super) fn of(kind: StepKind, settings: &Settings) -> Result<Self, NotYet> {
        Ok(match kind {
            StepKind::Orient => Self::Orient {
                wanted: Asked {
                    preset: text(settings, "goal").map_or(PREPARE_ORIENT, preset_of),
                    clean_surface: flag(settings, "clean_surface").unwrap_or(false),
                },
                max_tilt_deg: number(settings, "max_tilt_deg"),
            },
            StepKind::Repair => {
                Self::Repair(text(settings, "method").map_or(PREPARE_REPAIR, method_of))
            }
            StepKind::Hollow => Self::Hollow(Hollowed::of(settings)),
            StepKind::Drain => drain_of(settings),
            StepKind::Supports => Self::Supports {
                profile: text(settings, "profile").unwrap_or("auto").to_owned(),
                side: Side::Outside,
            },
            StepKind::Check => Self::Check { attempt: 0 },
            StepKind::Arrange => Self::Arrange {
                spacing_mm: number(settings, "spacing_mm")
                    .unwrap_or(ArrangeSettings::default().clearance_mm),
            },
            StepKind::Save => Self::Save(Saved {
                folder: text(settings, "folder").map(str::to_owned),
                name: text(settings, "name").map(str::to_owned),
                send: flag(settings, "send").unwrap_or(false),
            }),
            // A gate and a pause are decided, never run.
            StepKind::Gate | StepKind::Pause => return Err(NotYet(kind)),
        })
    }

    /// The job the bar names it by.
    pub(super) fn step(&self) -> Step {
        match self {
            Self::Orient { .. } => Step::Orient,
            Self::Repair(_) => Step::Repair,
            Self::Hollow(_) => Step::Hollow,
            Self::DrainAuto { .. } | Self::DrainPoints { .. } => Step::Drain,
            Self::Supports {
                side: Side::Outside,
                ..
            } => Step::Supports,
            Self::Supports {
                side: Side::Inside, ..
            } => Step::Cavity,
            Self::Scan => Step::Scan,
            Self::Check { .. } => Step::Check,
            Self::Arrange { .. } => Step::Arrange,
            Self::Save(_) => Step::Save,
        }
    }

    /// Whether it is a drain whose holes are left with no plug.
    pub(super) fn unplugged(&self) -> bool {
        matches!(
            self,
            Self::DrainAuto { plugs: false, .. } | Self::DrainPoints { plugs: false, .. }
        )
    }

    /// Whether it works on the plate as a whole, once a step whatever its models.
    pub(super) fn whole_plate(&self) -> bool {
        matches!(
            self,
            Self::Check { .. } | Self::Arrange { .. } | Self::Save(_)
        )
    }
}

/// Whether a supports step holds the model up inside its cavity as well.
pub(super) fn cavity(settings: &Settings) -> bool {
    flag(settings, "cavity").unwrap_or(false)
}

impl Hollowed {
    fn of(settings: &Settings) -> Self {
        let default = HollowSettings::default();
        let (mode, open_floor) = match text(settings, "mode") {
            Some("outside") => (HollowMode::External, false),
            Some("open-floor") => (HollowMode::Internal, true),
            _ => (default.mode, false),
        };
        Self {
            wall_mm: number(settings, "wall_mm").unwrap_or(default.thickness_mm),
            mode,
            open_floor,
            infill: flag(settings, "infill").unwrap_or(false),
            solid_under_mm: number(settings, "keep_solid_under_mm"),
        }
    }

    /// Hollow's settings for a model placed by `transform`, its floor opened toward the plate.
    pub(super) fn settings_for(self, transform: Transform) -> HollowSettings {
        HollowSettings {
            thickness_mm: self.wall_mm,
            mode: self.mode,
            infill: self.infill.then(InfillSettings::default),
            open_toward: self.open_floor.then(|| plate_down(transform)),
            solid_under_mm: self.solid_under_mm,
            ..HollowSettings::default()
        }
    }
}

fn drain_of(settings: &Settings) -> Work {
    // Left out, a recipe lays no plug, as the command line lays none without the flag.
    let plugs = flag(settings, "plugs").unwrap_or(false);
    let default = HoleSize::default();
    let size = HoleSize {
        diameter_mm: number(settings, "diameter_mm").unwrap_or(default.diameter_mm),
        depth_mm: number(settings, "depth_mm").unwrap_or(default.depth_mm),
        ..default
    };
    match text(settings, "at") {
        Some("points") => Work::DrainPoints {
            size,
            points: points(settings, "points").to_vec(),
            plugs,
        },
        _ => Work::DrainAuto {
            size,
            vent: flag(settings, "vent").unwrap_or(true),
            plugs,
        },
    }
}

fn method_of(method: &str) -> RepairMethod {
    match method {
        "close-holes" => RepairMethod::Mend(MendMethod::CloseHoles),
        "wrap" => RepairMethod::Wrap,
        "deep" => RepairMethod::Deep,
        _ => RepairMethod::Mend(MendMethod::Repair),
    }
}
