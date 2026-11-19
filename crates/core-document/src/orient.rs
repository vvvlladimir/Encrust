//! Standing an object the way it prints best, judged as it stands and unproven by a trial
//! print (`docs/design/document.md`).

use std::sync::Arc;

use core_edit::{ObjectId, OrientAim};
use core_engine::project::{ModelMeshes, ObjectState, hollow_of};
use core_engine::{EngineError, orient_for_print};
use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_plate::{OrientGoal, OrientPreset, OrientSettings, PlateError, Printed, Watch};
use core_volume::{Cancel, ModelHollow};
use printer_profiles::SupportProfile;

use crate::apply::{Applied, Report};
use crate::document::Document;
use crate::error::ApplyError;
use crate::placement::{degrees_of, oriented_to};
use crate::supports::active_profile;

/// What an `Orient` edit asks of an object: the aim, the tilt allowed, whether a clean surface
/// is held to, and the pose picked.
pub(crate) type OrientAsked = (OrientAim, Option<Scalar>, bool, usize);

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
    OrientSettings {
        max_overhang_deg: profile.max_overhang_deg,
        contact_spacing_mm: profile.contact_spacing_mm,
        on_supports: true,
        lift_mm: profile.lift_mm(),
        rest_on_plate: true,
        ..OrientSettings::default()
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

/// Stands object `id` at the `pick`th pose the search offers.
pub(crate) fn apply_orient(
    document: &mut Document,
    id: ObjectId,
    (goal, max_tilt_deg, clean_surface, pick): OrientAsked,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let on_supports = active_profile(document, state)
        .map_or_else(OrientSettings::default, |p| supported_settings(&p));
    let settings = OrientSettings {
        preset: preset_of(goal),
        clean_surface,
        max_tilt_deg,
        down_now: state.transform.rotation.inverse() * Vec3::NEG_Z,
        ..on_supports
    };
    let body = Body::of(state, meshes).sized(state.transform.sizing());
    let told = |_, _, _| !cancel.asked();
    let found = orient_for_print(
        &body.printed(&state.shown_painted),
        &settings,
        None,
        Watch::by(&told),
    );
    let found = match found {
        Err(EngineError::Orient(PlateError::Cancelled)) => return Err(ApplyError::Cancelled),
        Err(error) => {
            let reason = error.to_string();
            return Err(ApplyError::Failed { object: id, reason });
        }
        Ok(found) => found,
    };
    let choice = found.get(pick).ok_or(ApplyError::NoChoice {
        object: id,
        pick,
        found: found.len(),
    })?;
    let rotation = choice.oriented.rotation;
    let transform = oriented_to(state.transform, &meshes.source, rotation);
    document.set_transform(id, transform);
    Ok(Applied {
        removed: Vec::new(),
        added: Vec::new(),
        report: Report::Oriented {
            rotate_deg: degrees_of(rotation),
        },
    })
}

#[cfg(test)]
mod tests;
