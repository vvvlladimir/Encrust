//! A plate step's resolved settings as the edits that carry it out (`docs/design/recipes.md`).

use core_edit::{AUTO, Edit, ObjectId, OrientAim};
use core_engine::PREPARE_REPAIR;
use core_engine::project::HollowState;
use core_geometry::{Aabb, MendMethod, Scalar, Vec3};
use core_plate::ArrangeSettings;
use core_volume::{HoleKind, HoleSize, HollowMode, PLUG_GAP_MM, RepairMethod};
use thiserror::Error;

use crate::recipe::StepKind;
use crate::resolved::{ResolvedSettings, flag, number, points, text};

/// A step that is no edit of a plate, or a setting of one no edit holds yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("the {} step is not an edit of a plate", .0.name())]
pub struct NotAnEdit(pub StepKind);

/// The edits step `kind` makes of `object` with `settings`, a key left out at its default, on
/// the plate's usable `area_mm`; a plug gap has the resin's `plug_drift_mm` taken off.
pub fn edits_of(
    kind: StepKind,
    settings: &ResolvedSettings,
    object: ObjectId,
    placed: Aabb,
    area_mm: ([Scalar; 2], [Scalar; 2]),
    plug_drift_mm: Scalar,
) -> Result<Vec<Edit>, NotAnEdit> {
    Ok(match kind {
        StepKind::Repair => vec![Edit::Repair {
            object,
            method: text(settings, "method").map_or(PREPARE_REPAIR, method_of),
            seam_tolerance_mm: None,
            smallest_shell_mm2: None,
        }],
        StepKind::Orient => vec![Edit::Orient {
            object,
            goal: text(settings, "goal").map_or(OrientAim::Auto, OrientAim::named),
            max_tilt_deg: number(settings, "max_tilt_deg"),
            clean_surface: flag(settings, "clean_surface").unwrap_or(false),
            pick: 0,
            plate_mm: Some(area_mm.1),
            low_mm: area_mm.0,
            printer: None,
            resin: None,
            keep_clear_mm: Vec::new(),
            keep_clear_radius_mm: None,
            keep_clear_seen_deg: None,
            lift_mm: None,
        }],
        StepKind::Hollow => vec![Edit::Hollow {
            object,
            settings: hollow_of(settings),
            keep_solid: Vec::new(),
            keep_solid_reach_mm: None,
        }],
        StepKind::Drain => drain_of(settings, (object, placed), (area_mm, plug_drift_mm)),
        StepKind::Supports => vec![Edit::Supports {
            object,
            profile: Some(text(settings, "profile").unwrap_or(AUTO).to_owned()),
            at_mm: Vec::new(),
            only_at: false,
            cavity: flag(settings, "cavity").unwrap_or(false),
            shown_at: Vec::new(),
            clean_surface: false,
        }],
        // Sliding clear only what is in the way is the window's, with no edit yet.
        StepKind::Arrange => {
            let (clearance_mm, only_in_the_way) = arrange_of(settings);
            if only_in_the_way {
                return Err(NotAnEdit(kind));
            }
            let (low_mm, plate_mm) = area_mm;
            vec![Edit::Arrange {
                low_mm,
                plate_mm,
                clearance_mm,
            }]
        }
        StepKind::Check | StepKind::Save | StepKind::Gate | StepKind::Pause => {
            return Err(NotAnEdit(kind));
        }
    })
}

/// The arrange step's spacing in mm and whether it clears only what is in the way.
pub fn arrange_of(settings: &ResolvedSettings) -> (Scalar, bool) {
    (
        number(settings, "spacing_mm").unwrap_or(ArrangeSettings::default().clearance_mm),
        flag(settings, "only_in_the_way").unwrap_or(false),
    )
}

fn method_of(method: &str) -> RepairMethod {
    match method {
        "close-holes" => RepairMethod::Mend(MendMethod::CloseHoles),
        "wrap" => RepairMethod::Wrap,
        "deep" => RepairMethod::Deep,
        _ => RepairMethod::Mend(MendMethod::Repair),
    }
}

fn hollow_of(settings: &ResolvedSettings) -> HollowState {
    let default = HollowState::default();
    let (mode, open_floor) = match text(settings, "mode") {
        Some("outside") => (HollowMode::External, false),
        Some("open-floor") => (HollowMode::Internal, true),
        _ => (default.mode, false),
    };
    HollowState {
        thickness_mm: number(settings, "wall_mm").unwrap_or(default.thickness_mm),
        mode,
        open_floor,
        infill_on: flag(settings, "infill").unwrap_or(false),
        solid_under_mm: number(settings, "keep_solid_under_mm"),
        ..default
    }
}

fn drain_of(
    settings: &ResolvedSettings,
    (object, placed): (ObjectId, Aabb),
    (area_mm, plug_drift_mm): (([Scalar; 2], [Scalar; 2]), Scalar),
) -> Vec<Edit> {
    let default = HoleSize::default();
    let size = HoleSize {
        diameter_mm: number(settings, "diameter_mm").unwrap_or(default.diameter_mm),
        depth_mm: number(settings, "depth_mm").unwrap_or(default.depth_mm),
        ..default
    };
    let by_points = text(settings, "at") == Some("points");
    let drain = Edit::Drain {
        object,
        at_mm: if by_points {
            points(settings, "points")
                .iter()
                .map(|[x, y, z]| placed.mins + (placed.maxs - placed.mins) * Vec3::new(*x, *y, *z))
                .collect()
        } else {
            Vec::new()
        },
        size,
        kind: HoleKind::AtDepth,
        auto: !by_points,
        vent: !by_points && flag(settings, "vent").unwrap_or(true),
    };
    // Left out, a recipe lays no plug, as the command line lays none without the flag.
    let plugs = flag(settings, "plugs").unwrap_or(false);
    let mut edits = vec![drain];
    if plugs {
        edits.push(Edit::Plugs {
            object,
            gap_mm: PLUG_GAP_MM - plug_drift_mm,
            plate_mm: Some(area_mm.1),
            low_mm: area_mm.0,
        });
    }
    edits
}

#[cfg(test)]
mod tests;
