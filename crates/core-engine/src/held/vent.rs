//! Where a hole lets the air into a suction cup: the shortest way out of its lowest point.
//! See `docs/design/check-fixes.md`.

use core_analysis::{Risk, RiskKind};
use core_geometry::{PlacedHit, Scalar, Vec3};
use core_raster::RasterSettings;
use core_slicer::LayerPlan;

use super::{Hanging, Placed, first_along, placed};

/// How far past a face a ray starts so as not to meet that face again, mm.
const THROUGH_FACE_MM: Scalar = 1e-3;
/// How far into a cup a vent's tip reaches past the wall, mm: enough to be open whatever the
/// wall's facets do around its tip.
const VENT_PAST_WALL_MM: Scalar = 1.0;
/// The longest vent drilled, mm: past it the hole is a tunnel resin hardly flows through.
const LONGEST_VENT_MM: Scalar = 15.0;
/// The most faces a way out crosses before it is given up: overlapping shells nest a few.
const MOST_CROSSINGS: usize = 8;

/// A way for the air into a suction cup: a hole from the outside into its lowest point, in
/// plate millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CupVent {
    /// Index into the models given to [`cup_vents`].
    pub model: usize,
    pub mouth_mm: Vec3,
    /// Outward along the hole: it is drilled from the mouth the other way.
    pub outward: Vec3,
    pub depth_mm: Scalar,
}

/// Why a suction cup gets no vent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unvented {
    /// Its floor is the plate, or stands on it, and no wall beside it is thin enough: the
    /// model is lifted onto supports, which let the air in from below, or turned.
    OnThePlate,
    /// No wall around its lowest point is thinner than `LONGEST_VENT_MM`, or no floor under
    /// the cup's middle was found to drill from, as in a ring.
    TooDeep,
}

/// For each suction cup of `risks`, the shortest hole of `hole_mm` across from the outside
/// into its lowest point, or why there is none.
pub fn cup_vents(
    risks: &[Risk],
    plan: &LayerPlan,
    panel: &RasterSettings,
    models: &[Hanging],
    hole_mm: Scalar,
) -> Vec<Result<CupVent, Unvented>> {
    let placed = placed(models);
    let layer_mm = plan.nominal_thickness();
    let cups = risks
        .iter()
        .filter(|risk| matches!(risk.kind, RiskKind::SuctionCup { .. }));
    cups.map(|risk| {
        let top_mm = plan.top_of(risk.layer).ok_or(Unvented::TooDeep)?;
        let in_cup = panel.plate_mm(risk.at_mm).extend(top_mm);
        let floor = first_along(&placed, in_cup, -Vec3::Z);
        if floor.is_some_and(|(_, hit)| hit.normal.z <= 0.0) {
            // The middle of a ring-shaped cup lies in the material around it.
            return Err(Unvented::TooDeep);
        }
        let floor_mm = floor.map_or(0.0, |(_, hit)| hit.point.z);
        let clear = Clearance { hole_mm, layer_mm };
        // Half the hole sunk into the floor: a vent above it leaves a cup under its rim.
        let lowest = in_cup.with_z(floor_mm + hole_mm / 4.0);
        shortest_way_out(&placed, lowest, clear).ok_or_else(|| {
            let down = through_wall(&placed, lowest, -Vec3::Z);
            let on_plate = floor.is_none()
                || down.is_some_and(|(_, _, exit)| !clear.holds(exit.point, -Vec3::Z));
            if on_plate {
                Unvented::OnThePlate
            } else {
                Unvented::TooDeep
            }
        })
    })
    .collect()
}

/// What a vent's mouth keeps off the plate.
#[derive(Debug, Clone, Copy)]
struct Clearance {
    hole_mm: Scalar,
    layer_mm: Scalar,
}

impl Clearance {
    /// Whether a hole going out along `outward` with its mouth at `mouth` stays open: a level
    /// one is a notch along the plate, one going down is sealed by a plate under its rim.
    fn holds(self, mouth: Vec3, outward: Vec3) -> bool {
        let rim_drop = self.hole_mm / 2.0 * (1.0 - outward.z * outward.z).max(0.0).sqrt();
        outward.z >= 0.0 || mouth.z - rim_drop >= self.layer_mm - THROUGH_FACE_MM
    }
}

/// The ways out of a cup a vent is tried along: straight down, then down at 45 degrees and
/// level, each eight ways round.
fn ways_out() -> impl Iterator<Item = Vec3> {
    let round = (0..8).map(|step| {
        let (sin, cos) = (step as Scalar * std::f32::consts::FRAC_PI_4).sin_cos();
        Vec3::new(cos, sin, 0.0)
    });
    let slanted = round.clone().map(|level| (level - Vec3::Z).normalize());
    std::iter::once(-Vec3::Z).chain(slanted).chain(round)
}

/// The vent through the thinnest wall around `lowest` whose mouth `clear` allows.
fn shortest_way_out(placed: &[(usize, Placed)], lowest: Vec3, clear: Clearance) -> Option<CupVent> {
    ways_out()
        .filter_map(|outward| {
            let (model, wall_mm, exit) = through_wall(placed, lowest, outward)?;
            (wall_mm <= LONGEST_VENT_MM && clear.holds(exit.point, outward)).then_some(CupVent {
                model,
                mouth_mm: exit.point,
                outward,
                depth_mm: wall_mm + VENT_PAST_WALL_MM,
            })
        })
        .min_by(|one, other| one.depth_mm.total_cmp(&other.depth_mm))
}

/// The model whose wall `from` meets first along `outward`, how thick it is there, mm, and
/// where the ray leaves it: inside overlapping shells it is out once it has left them all.
fn through_wall(
    placed: &[(usize, Placed)],
    from: Vec3,
    outward: Vec3,
) -> Option<(usize, Scalar, PlacedHit)> {
    let (model, entry) = first_along(placed, from, outward)?;
    if entry.normal.dot(outward) >= 0.0 {
        return None;
    }
    let own = placed.iter().position(|(id, _)| *id == model)?;
    let own = &placed[own..=own];
    let mut inside = 1_usize;
    let mut at = entry.point;
    for _ in 0..MOST_CROSSINGS {
        let (_, hit) = first_along(own, at + outward * THROUGH_FACE_MM, outward)?;
        inside = if hit.normal.dot(outward) < 0.0 {
            inside + 1
        } else {
            inside - 1
        };
        if inside == 0 {
            return Some((model, entry.point.distance(hit.point), hit));
        }
        at = hit.point;
    }
    None
}
