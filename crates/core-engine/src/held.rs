//! What a measured stack shows, answered: where a support stands under an island or a weak
//! neck, and where a hole lets the air into a suction cup. See `docs/design/check-fixes.md`.

use core_analysis::{Risk, RiskKind, holds_for_lever};
use core_geometry::{
    Bvh, Mat4, Mesh, PlacedHit, Ray, Scalar, Transform, Vec3, raycast_placed_within,
};
use core_raster::RasterSettings;
use core_slicer::LayerPlan;
use printer_profiles::SupportProfile;

mod vent;

pub use vent::{CupVent, Unvented, cup_vents};

/// How many layers over an island's floor its underside may first be found.
const UNDERSIDE_LAYERS: Scalar = 2.0;

/// A model with its placement as a matrix and that matrix's inverse.
type Placed<'a> = (&'a Mesh, &'a Bvh, Mat4, Mat4);

fn placed<'a>(models: &[Hanging<'a>]) -> Vec<(usize, Placed<'a>)> {
    models
        .iter()
        .enumerate()
        .filter_map(|(id, (mesh, bvh, transform))| {
            let inverse = transform.inverse_matrix()?;
            Some((id, (*mesh, *bvh, transform.to_matrix(), inverse)))
        })
        .collect()
}

/// The nearest surface of `placed` straight below `from`, and the index of its model.
fn first_below(placed: &[(usize, Placed)], from: Vec3) -> Option<(usize, PlacedHit)> {
    first_along(placed, from, -Vec3::Z)
}

/// The nearest surface of `placed` from `from` along `direction`, and the index of its model.
fn first_along(
    placed: &[(usize, Placed)],
    from: Vec3,
    direction: Vec3,
) -> Option<(usize, PlacedHit)> {
    let ray = Ray::new(from, direction);
    placed
        .iter()
        .filter_map(|(id, (mesh, bvh, matrix, inverse))| {
            let hit = raycast_placed_within(mesh, bvh, *matrix, *inverse, &ray, Scalar::INFINITY)?;
            Some((*id, hit))
        })
        .min_by(|(_, one), (_, other)| one.distance.total_cmp(&other.distance))
}

/// Where each island of `risks` stands a support: on the plate under it, at the top of the
/// layer below, mm. The stack was measured on `panel` and cut by `plan`.
pub fn island_floors(risks: &[Risk], plan: &LayerPlan, panel: &RasterSettings) -> Vec<Vec3> {
    risks
        .iter()
        .filter(|risk| {
            matches!(
                risk.kind,
                RiskKind::Island { .. } | RiskKind::Floating { .. }
            )
        })
        .filter_map(|risk| {
            let floor_mm = plan.top_of(risk.layer.checked_sub(1)?)?;
            Some(panel.plate_mm(risk.at_mm).extend(floor_mm))
        })
        .collect()
}

/// A model an island may hang from: its mesh, that mesh's bvh and where it is placed.
pub type Hanging<'a> = (&'a Mesh, &'a Bvh, Transform);

/// For each of `floors`, the model of `models` its island hangs from and the point of that
/// model's underside a support there touches: the first surface straight over a point a layer
/// under the floor, when it is no more than two layers up; `None` for an island of none.
pub fn island_holds(
    floors: &[Vec3],
    layer_mm: Scalar,
    models: &[Hanging],
) -> Vec<Option<(usize, Vec3)>> {
    let placed = placed(models);
    floors
        .iter()
        .map(|floor| {
            let ray = Ray::new(*floor - Vec3::Z * layer_mm, Vec3::Z);
            let within = UNDERSIDE_LAYERS * layer_mm;
            placed
                .iter()
                .filter_map(|(id, (mesh, bvh, matrix, inverse))| {
                    let hit = raycast_placed_within(mesh, bvh, *matrix, *inverse, &ray, within)?;
                    Some((*id, hit))
                })
                .min_by(|(_, one), (_, other)| one.distance.total_cmp(&other.distance))
                .map(|(id, hit)| (id, hit.point))
        })
        .collect()
}

/// The fewest supports a weak neck's tip is drawn at, mm across, whatever the profile: a
/// tip of 0.3 mm, as Auto draws, adds next to nothing to a neck.
const NECK_TIP_MM: f32 = 0.6;

/// `profile` as a support holding a weak neck stands to, its tip and any small pillar in its
/// place thick enough to carry part of the pull.
pub fn neck_profile(profile: &SupportProfile) -> SupportProfile {
    let mut sturdy = profile.clone();
    sturdy.tip.contact_diameter_mm = sturdy.tip.contact_diameter_mm.max(NECK_TIP_MM);
    sturdy.small_pillar.diameter_mm = sturdy.small_pillar.diameter_mm.max(NECK_TIP_MM);
    sturdy
}

/// The section `profile` adds to a neck per support, mm²: the narrower of a tip and a small
/// pillar, whichever it stands as.
fn neck_tip_mm2(profile: &SupportProfile) -> f32 {
    let across = profile
        .tip
        .contact_diameter_mm
        .min(profile.small_pillar.diameter_mm);
    std::f32::consts::PI * (across / 2.0).powi(2)
}

/// Around a piece's centre, in units of its radius of gyration: the centre, then opposite
/// points first, so that a few supports already stand balanced.
const NECK_RINGS: [(f32, f32); 17] = {
    const R: f32 = 1.0;
    const H: f32 = 0.5;
    const D: f32 = 0.707;
    [
        (0.0, 0.0),
        (R, 0.0),
        (-R, 0.0),
        (0.0, R),
        (0.0, -R),
        (D, D),
        (-D, -D),
        (D, -D),
        (-D, D),
        (H, 0.0),
        (-H, 0.0),
        (0.0, H),
        (0.0, -H),
        (0.354, 0.354),
        (-0.354, -0.354),
        (0.354, -0.354),
        (-0.354, 0.354),
    ]
};

/// For each weak neck of `risks`, as many underside points of the piece that pulls on it as
/// [`holds_for_lever`] asks of `profile`'s tips, each with its model; none for a piece over
/// no underside, or whose underside lies on the plate.
pub fn lever_holds(
    risks: &[Risk],
    plan: &LayerPlan,
    panel: &RasterSettings,
    models: &[Hanging],
    profile: &SupportProfile,
) -> Vec<Vec<(usize, Vec3)>> {
    let placed = placed(models);
    let layer_mm = plan.nominal_thickness();
    let tip_mm2 = neck_tip_mm2(profile);
    let spacing_mm = 2.0 * profile.contact_radius_mm().max(NECK_TIP_MM / 2.0);
    let levers = risks
        .iter()
        .filter(|risk| matches!(risk.kind, RiskKind::Lever { .. }));
    levers
        .map(|risk| {
            let Some(floor_mm) = risk.layer.checked_sub(1).and_then(|at| plan.top_of(at)) else {
                return Vec::new();
            };
            let centre = panel.plate_mm(risk.at_mm).extend(floor_mm + layer_mm / 2.0);
            let RiskKind::Lever { gyration_mm, .. } = risk.kind else {
                return Vec::new();
            };
            let wanted = holds_for_lever(&risk.kind, tip_mm2);
            undersides(&placed, centre, gyration_mm, layer_mm)
                .into_iter()
                .fold(Vec::new(), |mut held: Vec<(usize, Vec3)>, hit| {
                    let clear = held.iter().all(|(_, at)| at.distance(hit.1) >= spacing_mm);
                    if clear && held.len() < wanted {
                        held.push(hit);
                    }
                    held
                })
        })
        .collect()
}

/// The undersides straight below `centre` and the points of [`NECK_RINGS`] around it.
fn undersides(
    placed: &[(usize, Placed)],
    centre: Vec3,
    gyration_mm: Scalar,
    layer_mm: Scalar,
) -> Vec<(usize, Vec3)> {
    NECK_RINGS
        .iter()
        .filter_map(|(x, y)| {
            let from = centre + Vec3::new(x * gyration_mm, y * gyration_mm, 0.0);
            let (model, under) = first_below(placed, from)?;
            (under.normal.z < 0.0 && under.point.z >= layer_mm).then_some((model, under.point))
        })
        .collect()
}

#[cfg(test)]
mod tests;
