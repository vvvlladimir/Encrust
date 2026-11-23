//! Standing an object the way it prints best, judged as it stands and unproven by a trial
//! print (`docs/design/document.md`).

use std::sync::Arc;

use core_edit::{Edit, ObjectId, OrientAim};
use core_engine::project::{ModelMeshes, ObjectState, hollow_of};
use core_engine::{
    Choice, EngineError, KEEP_CLEAR_RADIUS_MM, KeepClear, SupportPlan, Trial, orient_for_print,
};
use core_geometry::{Mesh, Scalar, Transform, Vec2, Vec3, transform_mesh};
use core_plate::{OrientGoal, OrientPreset, OrientSettings, Phase, PlateError, Printed, Watch};
use core_volume::{Cancel, ModelHollow, Step};
use printer_profiles::{
    Catalogue, MaterialProfile, PrinterProfile, ProfileError, Raft, SupportProfile, machine_named,
};

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::placement::{Footing, degrees_of, euler_degrees, stood_turned};
use crate::supports::orient_plan_of;

/// The pieces a search's share is counted in, so a bar moves in steps of a thousandth.
const SHARE_PIECES: usize = 1000;

/// What an `Orient` edit asks of an object: the aim, the tilt allowed, whether a clean surface
/// is held to, the pose picked, the usable area it must stay on and the machine it is proved on.
pub(crate) struct OrientAsked {
    pub goal: OrientAim,
    pub max_tilt_deg: Option<Scalar>,
    pub clean_surface: bool,
    pub pick: usize,
    pub area_mm: Option<([Scalar; 2], [Scalar; 2])>,
    pub printer: Option<String>,
    pub resin: Option<String>,
    /// Places kept clear, plate mm; how near still touches, mm; the turn they were seen at.
    pub keep_clear: (Vec<Vec3>, Option<Scalar>, Option<Vec3>),
}

impl OrientAsked {
    /// What `edit` asks, if it is an `Orient`.
    pub(crate) fn of(edit: &Edit) -> Option<Self> {
        let Edit::Orient {
            goal,
            max_tilt_deg,
            clean_surface,
            pick,
            plate_mm,
            low_mm,
            printer,
            resin,
            keep_clear_mm,
            keep_clear_radius_mm,
            keep_clear_seen_deg,
            ..
        } = edit
        else {
            return None;
        };
        Some(Self {
            goal: *goal,
            max_tilt_deg: *max_tilt_deg,
            clean_surface: *clean_surface,
            pick: *pick,
            area_mm: plate_mm.map(|plate| (*low_mm, plate)),
            printer: printer.clone(),
            resin: resin.clone(),
            keep_clear: (
                keep_clear_mm.clone(),
                *keep_clear_radius_mm,
                *keep_clear_seen_deg,
            ),
        })
    }
}

/// The preset an orient step's goal `word` names; any other word is `auto`.
pub fn preset_named(word: &str) -> OrientPreset {
    preset_of(OrientAim::named(word))
}

/// The search preset `aim` names.
pub fn preset_of(aim: OrientAim) -> OrientPreset {
    match aim {
        OrientAim::Auto => OrientPreset::Auto,
        OrientAim::FewestSupports => OrientGoal::FewestSupports.into(),
        OrientAim::LeastPeel => OrientGoal::LeastPeel.into(),
        OrientAim::ShortestPrint => OrientGoal::ShortestPrint.into(),
    }
}

/// How the search judges a model that stands on supports built to `profile`.
pub fn supported_settings(profile: &SupportProfile) -> OrientSettings {
    placed_by(supported_base(), &SupportPlan::of(profile.clone()))
}

fn supported_base() -> OrientSettings {
    OrientSettings {
        on_supports: true,
        rest_on_plate: true,
        ..OrientSettings::default()
    }
}

/// The plan orienting weighs supports by: `sized` where a plate keeps an automatic profile,
/// else the `active` group's profile, every shape on `raft`.
pub fn orient_plan(sized: Option<SupportPlan>, active: &SupportProfile, raft: Raft) -> SupportPlan {
    let mut plan = sized.unwrap_or_else(|| SupportPlan::of(active.clone()));
    for shape in plan.profiles_mut() {
        shape.raft = raft;
    }
    plan
}

/// `plan` standing a model `lift_mm` clear of the plate, where the user asked for that.
pub fn lifted(mut plan: SupportPlan, lift_mm: Option<Scalar>) -> SupportPlan {
    if let Some(lift_mm) = lift_mm {
        for shape in plan.profiles_mut() {
            shape.z_lift_mm = lift_mm;
        }
    }
    plan
}

/// `settings` judging by what `plan` places: its overhang, contact spacing and lift.
pub fn placed_by(settings: OrientSettings, plan: &SupportPlan) -> OrientSettings {
    let placing = plan.placing();
    OrientSettings {
        max_overhang_deg: placing.max_overhang_deg,
        contact_spacing_mm: placing.contact_spacing_mm,
        lift_mm: placing.lift_mm(),
        ..settings
    }
}

/// The way out of every hole and of both ends of every channel cut into a model.
pub fn vents_of(hollow: &ModelHollow) -> Vec<Vec3> {
    let (holes, channels) = hollow.cut();
    let ends = channels.iter().flat_map(|channel| {
        let points = &channel.points;
        let first = points
            .first()
            .zip(points.get(1))
            .map(|(end, next)| *end - *next);
        let last = points
            .last()
            .zip(points.iter().rev().nth(1))
            .map(|(end, next)| *end - *next);
        first.into_iter().chain(last)
    });
    holes.iter().map(|hole| -hole.axis).chain(ends).collect()
}

/// A model as it prints, in its own space: its outside, its shell where hollowed, what is cut
/// into it and the way out of each hole.
#[derive(Debug, Clone)]
pub struct Body {
    pub outer: Arc<Mesh>,
    pub shell: Option<Arc<Mesh>>,
    pub cuts: Option<Arc<Mesh>>,
    pub vents: Vec<Vec3>,
}

impl Body {
    /// The object as it prints: its holes and channels cut as its shell stands.
    pub fn of(state: &ObjectState, meshes: &ModelMeshes) -> Self {
        let hollow = hollow_of(&state.hollow, meshes);
        Self {
            outer: Arc::clone(&meshes.source),
            shell: meshes.shell.clone(),
            cuts: hollow.cut_bodies().cloned(),
            vents: vents_of(&hollow),
        }
    }

    /// The body as it measures on the plate, scaled and stretched by `sizing` with its turn
    /// taken out; a model nobody resized is not copied.
    #[must_use]
    pub fn sized(&self, sizing: Transform) -> Self {
        if sizing == Transform::default() {
            return self.clone();
        }
        let size = |mesh: &Arc<Mesh>| Arc::new(transform_mesh(mesh, sizing));
        let linear = sizing.linear();
        Self {
            outer: size(&self.outer),
            shell: self.shell.as_ref().map(size),
            cuts: self.cuts.as_ref().map(size),
            vents: self.vents.iter().map(|out| linear * *out).collect(),
        }
    }

    /// The body for the search, `shown` the faces painted as what shows.
    pub fn printed<'a>(&'a self, shown: &'a core_geometry::Region) -> Printed<'a> {
        Printed {
            outer: &self.outer,
            shell: self.shell.as_deref(),
            cuts: self.cuts.as_deref(),
            vents: &self.vents,
            shown: Some(shown),
            shows: None,
        }
    }
}

/// The printer and resin an orient proves its poses on, the standard ones where unnamed, read
/// from `catalogue`.
fn machine_in(
    catalogue: Result<Catalogue, ProfileError>,
    (printer, resin): (Option<&str>, Option<&str>),
) -> Result<(PrinterProfile, MaterialProfile), ApplyError> {
    let catalogue = catalogue.map_err(|error| ApplyError::BadCatalogue(error.to_string()))?;
    machine_named(&catalogue, printer, resin).map_err(|error| match error {
        ProfileError::Unknown { kind: "resin", id } => ApplyError::NoResin(id),
        ProfileError::Unknown { id, .. } => ApplyError::NoPrinter(id),
        other => ApplyError::BadCatalogue(other.to_string()),
    })
}

/// The places `points` name on a model whose sized own mesh is `sized`, in that mesh's frame:
/// seen turned `seen_deg` and centred on a plate `plate_mm` across, else standing by `transform`.
pub fn kept_clear(
    sized: &Mesh,
    transform: Transform,
    (points, radius_mm, seen_deg): (&[Vec3], Option<Scalar>, Option<Vec3>),
    plate_mm: Vec2,
) -> Option<KeepClear> {
    if points.is_empty() {
        return None;
    }
    let radius_mm = radius_mm.unwrap_or(KEEP_CLEAR_RADIUS_MM);
    if let Some(seen) = seen_deg {
        return KeepClear::seen_turned(sized, euler_degrees(seen), plate_mm, points, radius_mm);
    }
    let back = transform.rotation.inverse();
    let at_mm = (points.iter())
        .map(|point| back * (*point - transform.translation))
        .collect();
    Some(KeepClear { at_mm, radius_mm })
}

/// The plan an orient weighs where the plate has no support group: the stock profile, as the
/// window's default group builds it.
pub fn default_plan() -> SupportPlan {
    let stock = SupportProfile::default();
    orient_plan(None, &stock, stock.raft)
}

/// Every pose the search ranks for object `id`, best first, proved on the machine `asked` names,
/// and the lift a pose set on supports stands at, mm.
pub(crate) fn ranked_poses(
    document: &Document,
    id: ObjectId,
    asked: &OrientAsked,
    cancel: Cancel<'_>,
) -> Result<(Vec<Choice>, Scalar), ApplyError> {
    let named = (asked.printer.as_deref(), asked.resin.as_deref());
    let (printer, material) = machine_in(Catalogue::bundled(), named)?;
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let plan = orient_plan_of(document, state)?.unwrap_or_else(default_plan);
    let z_lift_mm = plan.placing().z_lift_mm;
    let on_supports = placed_by(supported_base(), &plan);
    let volume = &printer.build_volume;
    let settings = OrientSettings {
        preset: preset_of(asked.goal),
        clean_surface: asked.clean_surface,
        max_tilt_deg: asked.max_tilt_deg,
        down_now: state.transform.rotation.inverse() * Vec3::NEG_Z,
        build_volume_mm: Some(Vec3::new(volume.x, volume.y, volume.z)),
        ..on_supports
    };
    let body = Body::of(state, meshes).sized(state.transform.sizing());
    let (points, radius_mm, seen_deg) = &asked.keep_clear;
    let plate_mm = Vec2::new(volume.x, volume.y);
    let asked_clear = (points.as_slice(), *radius_mm, *seen_deg);
    let keep_clear = kept_clear(&body.outer, state.transform, asked_clear, plate_mm);
    let trial = Trial {
        printer: &printer,
        material: &material,
        support: &plan,
        raster_window: rayon::current_num_threads(),
        keep_clear: keep_clear.as_ref(),
    };
    let told = |phase: Phase, done, total| {
        let share = phase.share(done, total);
        cancel.reached(
            Step::Searching,
            (share * SHARE_PIECES as f32) as usize,
            SHARE_PIECES,
        );
        !cancel.asked()
    };
    let found = orient_for_print(
        &body.printed(&state.shown_painted),
        &settings,
        Some(&trial),
        Watch::by(&told),
    );
    match found {
        Err(EngineError::Orient(PlateError::Cancelled)) => Err(ApplyError::Cancelled),
        Err(error) => Err(ApplyError::Failed {
            object: id,
            reason: error.to_string(),
        }),
        Ok(found) => Ok((found, z_lift_mm)),
    }
}

/// One pose the search ranked, as an orient's report lists it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ranked {
    pub rotation: core_geometry::Quat,
    /// Whether it sets the model on the plate, needing no supports this way up.
    pub on_plate: bool,
    /// Whether it is worth offering beside the first.
    pub offered: bool,
    pub printout: Option<core_engine::Printout>,
}

impl Ranked {
    fn of(choice: &Choice) -> Self {
        Self {
            rotation: choice.oriented.rotation,
            on_plate: choice.oriented.on_plate,
            offered: choice.ruling.is_offered(),
            printout: choice.printout,
        }
    }
}

/// Stands object `id` at the `pick`th pose the search offers, proved on a printer.
pub(crate) fn apply_orient(
    document: &mut Document,
    id: ObjectId,
    asked: &OrientAsked,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (found, lift_mm) = ranked_poses(document, id, asked, cancel)?;
    let pick = asked.pick;
    let choice = found.get(pick).ok_or(ApplyError::NoChoice {
        object: id,
        pick,
        found: found.len(),
    })?;
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let rotation = choice.oriented.rotation;
    let footing = Footing {
        area_mm: asked.area_mm,
        lift_mm,
    };
    let at = (footing, choice.oriented.on_plate);
    let transform = stood_turned(state.transform, &meshes.source, rotation, at);
    document.set_transform(id, transform);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Oriented {
            rotate_deg: degrees_of(rotation),
            on_plate: choice.oriented.on_plate,
            peak_mm2: choice.oriented.score.peak_mm2.unwrap_or_default(),
            overhang_mm2: choice.oriented.score.overhang_mm2,
            ranked: found.iter().map(Ranked::of).collect(),
        },
    })
}

#[cfg(test)]
mod tests;
