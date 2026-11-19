//! What a recipe step's resolved settings ask of a tool's job; a key left out takes the
//! engine's default, never what a panel holds (`docs/design/recipes.md`).

use core_document::preset_of;
use core_edit::{AUTO, Edit, ObjectId as EditId};
use core_geometry::{Aabb, Scalar, Transform, Vec3};
use core_plate::Asked;
use core_recipe::resolved::{flag, text};
use core_recipe::{StepKind, arrange_of, edits_of};
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
    /// The stack measured as Check measures it, after `attempt` supportings of its islands;
    /// `fixes` are still to be answered once it is read.
    Check {
        attempt: u8,
        fixes: CheckFixes,
    },
    Arrange {
        spacing_mm: Scalar,
        only_in_the_way: bool,
    },
    Save(Saved),
}

/// What a check answers once, before its retry: its suction cups, its weak necks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CheckFixes {
    pub cups: bool,
    pub levers: bool,
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
    /// What step `kind` asks with `settings`, and whether its supports also hold the cavity.
    pub(super) fn of(kind: StepKind, settings: &Settings) -> Result<(Self, bool), NotYet> {
        let (spacing_mm, only_in_the_way) = arrange_of(settings);
        let work = match kind {
            StepKind::Arrange if only_in_the_way => Self::Arrange {
                spacing_mm,
                only_in_the_way,
            },
            StepKind::Check => Self::Check {
                attempt: 0,
                fixes: CheckFixes {
                    cups: flag(settings, "drill_cups").unwrap_or(false),
                    levers: flag(settings, "hold_levers").unwrap_or(false),
                },
            },
            StepKind::Save => Self::Save(Saved {
                folder: text(settings, "folder").map(str::to_owned),
                name: text(settings, "name").map(str::to_owned),
                send: flag(settings, "send").unwrap_or(false),
            }),
            // A gate and a pause are decided, never run.
            StepKind::Gate | StepKind::Pause => return Err(NotYet(kind)),
            _ => {
                let edits = stand_in_edits(kind, settings).ok_or(NotYet(kind))?;
                return Self::of_edits(&edits).ok_or(NotYet(kind));
            }
        };
        Ok((work, false))
    }

    /// The job a step's `edits` ask of a model, and whether its supports hold the cavity.
    fn of_edits(edits: &[Edit]) -> Option<(Self, bool)> {
        let work = match edits.first()? {
            Edit::Orient {
                goal,
                max_tilt_deg,
                clean_surface,
                ..
            } => Self::Orient {
                wanted: Asked {
                    preset: preset_of(*goal),
                    clean_surface: *clean_surface,
                },
                max_tilt_deg: *max_tilt_deg,
            },
            Edit::Repair { method, .. } => Self::Repair(*method),
            Edit::Hollow { settings, .. } => Self::Hollow(Hollowed {
                wall_mm: settings.thickness_mm,
                mode: settings.mode,
                open_floor: settings.open_floor,
                infill: settings.infill_on,
                solid_under_mm: settings.solid_under_mm,
            }),
            Edit::Drain {
                at_mm,
                size,
                auto,
                vent,
                ..
            } => {
                let plugs = edits.len() > 1;
                if *auto {
                    Self::DrainAuto {
                        size: *size,
                        vent: *vent,
                        plugs,
                    }
                } else {
                    // Against the unit box, a point comes back as its fractions.
                    Self::DrainPoints {
                        size: *size,
                        points: at_mm.iter().map(|at| at.to_array()).collect(),
                        plugs,
                    }
                }
            }
            Edit::Supports {
                profile, cavity, ..
            } => {
                let profile = profile.clone().unwrap_or_else(|| AUTO.to_owned());
                let side = Side::Outside;
                return Some((Self::Supports { profile, side }, *cavity));
            }
            Edit::Arrange { clearance_mm, .. } => Self::Arrange {
                spacing_mm: *clearance_mm,
                only_in_the_way: false,
            },
            _ => return None,
        };
        Some((work, false))
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

impl Hollowed {
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

/// The edits a step of `kind` makes of a stand-in model: no object, a unit box and no plate,
/// which `Work` reads back as fractions and ignores.
fn stand_in_edits(kind: StepKind, settings: &Settings) -> Option<Vec<Edit>> {
    let unit = Aabb::new(Vec3::ZERO, Vec3::ONE);
    edits_of(kind, settings, EditId::UNSET, unit, [0.0; 2]).ok()
}
