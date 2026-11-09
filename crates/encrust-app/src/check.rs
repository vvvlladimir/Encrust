//! The issues the Check tool last went to: ringed on the plate at the height of their
//! layer while the view stays as it was left, and the camera sent to them once.

use core_analysis::{Risk, RiskKind};
use core_engine::{CupVent, cup_vents, island_floors, lever_holds};
use core_geometry::{Scalar, Vec2, Vec3};
use core_raster::RasterSettings;
use core_slicer::LayerPlan;
use egui::Color32;
use printer_profiles::SupportProfile;

use crate::drain::DrainTool;
use crate::highlight::{EdgeMark, Highlight, LIFT_MM, Spot, ring};
use crate::i18n::tr;
use crate::preview::{as_seen, on_plate};
use crate::scene::{ObjectId, Scene};
use crate::status::Status;
use crate::supports::{ISLANDS_GROUP, SupportTool, hanging_models, island_holds};

mod answers;

pub use answers::{Answer, Answers, IssueKind};

/// The least radius an issue is ringed at, mm: an island of a few pixels still shows, and
/// an issue with no area of its own is ringed at this.
const LEAST_RING_MM: f32 = 0.5;

/// One issue on a layer: where it lies on the plate, mm, and the area it covers, mm².
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub at_mm: Vec2,
    pub area_mm2: Option<f32>,
}

impl Place {
    /// Where `risk` lies on the plate, read off a stack rasterised under `settings`.
    pub fn of(risk: &Risk, settings: &RasterSettings) -> Self {
        Self {
            at_mm: on_plate(settings, risk.at_mm),
            area_mm2: covered_mm2(&risk.kind),
        }
    }

    /// The circle of the issue's own area, or the least one.
    fn radius_mm(self) -> f32 {
        let disc = self
            .area_mm2
            .map_or(0.0, |area| (area / std::f32::consts::PI).sqrt());
        disc.max(LEAST_RING_MM)
    }
}

/// The area an issue covers on its layer, mm², or `None` for one read off no area.
pub fn covered_mm2(kind: &RiskKind) -> Option<f32> {
    match *kind {
        RiskKind::Island { area_mm2 }
        | RiskKind::Floating { area_mm2 }
        | RiskKind::SuctionCup { area_mm2, .. } => Some(area_mm2),
        RiskKind::Lever { .. } | RiskKind::Peel { .. } | RiskKind::Thin { .. } => None,
    }
}

/// A recipe's run asking for islands supported: the models, and the profile each stands on.
pub struct ForRun<'a> {
    pub ids: &'a [ObjectId],
    pub profiles: &'a [(ObjectId, SupportProfile)],
}

/// Check's Support them: a support under every island of `risks`, in the islands' group, on
/// the panel's profile and any model, or for a run on its models and their own profiles;
/// says how many stood.
pub fn support_islands(
    (scene, supports): (&mut Scene, &mut SupportTool),
    (plan, settings): (&LayerPlan, &RasterSettings),
    risks: &[Risk],
    run: Option<ForRun>,
) -> Status {
    let layer_mm = plan.nominal_thickness();
    let floors = island_floors(risks, plan, &as_seen(settings));
    let found: Vec<(ObjectId, Vec3)> = island_holds(scene, &floors, layer_mm)
        .into_iter()
        .flatten()
        .filter(|(id, _)| run.as_ref().is_none_or(|run| run.ids.contains(id)))
        .collect();
    let stood = stand(scene, supports, &found, run.as_ref());
    Status::Info(tr!("check-stood", count = stood, group = ISLANDS_GROUP))
}

/// Check's Support them for weak necks: a support under the piece pulling on each, as
/// [`support_islands`] stands them; says how many stood and how many had nowhere to.
pub fn support_levers(
    (scene, supports): (&mut Scene, &mut SupportTool),
    stack: (&LayerPlan, &RasterSettings),
    risks: &[Risk],
    run: Option<ForRun>,
) -> Status {
    let holds = lever_owners(scene, stack, risks);
    let found: Vec<(ObjectId, Vec3)> = holds
        .iter()
        .flatten()
        .filter(|(id, _)| run.as_ref().is_none_or(|run| run.ids.contains(id)))
        .copied()
        .collect();
    let left = holds.iter().filter(|hold| hold.is_none()).count();
    let stood = stand(scene, supports, &found, run.as_ref());
    told(
        tr!("check-stood-levers", count = stood, group = ISLANDS_GROUP),
        left,
    )
}

/// Check's Drill them: a hole through the floor of every suction cup of `risks`, on `only` if
/// named; says how many went in and how many have no floor to drill.
pub fn drill_cups(
    (scene, drain): (&mut Scene, &mut DrainTool),
    stack: (&LayerPlan, &RasterSettings),
    risks: &[Risk],
    only: Option<&[ObjectId]>,
) -> Status {
    let vents = cup_owners(scene, stack, risks);
    let found: Vec<(ObjectId, CupVent)> = vents
        .iter()
        .flatten()
        .filter(|(id, _)| only.is_none_or(|ids| ids.contains(id)))
        .copied()
        .collect();
    let left = vents.iter().filter(|vent| vent.is_none()).count();
    let drilled = drain.drill_vents(scene, &found);
    told(tr!("check-drilled-cups", count = drilled), left)
}

/// The one model all of `owners` name, or `None` where they name several or cannot tell: what
/// Auto-orient is asked to turn.
pub fn only_owner(owners: impl IntoIterator<Item = Option<ObjectId>>) -> Option<ObjectId> {
    let mut owners = owners.into_iter();
    let first = owners.next()??;
    owners.all(|owner| owner == Some(first)).then_some(first)
}

/// For each suction cup of `risks`, the model it is in and the hole that vents it.
pub fn cup_owners(
    scene: &Scene,
    (plan, settings): (&LayerPlan, &RasterSettings),
    risks: &[Risk],
) -> Vec<Option<(ObjectId, CupVent)>> {
    let (objects, models) = hanging_models(scene);
    cup_vents(risks, plan, &as_seen(settings), &models)
        .into_iter()
        .map(|vent| vent.map(|vent| (objects[vent.model].id, vent)))
        .collect()
}

/// For each weak neck of `risks`, the model it is in and where a support holds it.
pub fn lever_owners(
    scene: &Scene,
    (plan, settings): (&LayerPlan, &RasterSettings),
    risks: &[Risk],
) -> Vec<Option<(ObjectId, Vec3)>> {
    let (objects, models) = hanging_models(scene);
    lever_holds(risks, plan, &as_seen(settings), &models)
        .into_iter()
        .map(|hold| hold.map(|(at, contact)| (objects[at].id, contact)))
        .collect()
}

/// Stands a support at each of `found`, on a run's profiles where there is one.
fn stand(
    scene: &mut Scene,
    supports: &mut SupportTool,
    found: &[(ObjectId, Vec3)],
    run: Option<&ForRun>,
) -> usize {
    match run {
        None => supports.support_islands(scene, found),
        Some(run) => supports.support_islands_to(scene, found, |id| {
            let own = run.profiles.iter().find(|(owner, _)| *owner == id);
            own.map(|(_, profile)| profile.clone()).unwrap_or_default()
        }),
    }
}

/// `said`, with how many of the issues had nowhere to be fixed where any had none.
fn told(said: String, left: usize) -> Status {
    match left {
        0 => Status::Info(said),
        left => Status::Noted(said, vec![tr!("check-left-to-you", count = left)]),
    }
}

/// What the rings were laid in: the plate's `preview::plate_fingerprint` and the height
/// the view cut at, mm, or `None` for no cut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stage {
    pub plate: u64,
    pub cut_mm: Option<Scalar>,
}

#[derive(Debug, Clone, PartialEq)]
struct Visited {
    stage: Stage,
    rings: Vec<[Vec3; 2]>,
    colour: Color32,
}

/// The issues gone to, where the camera is still to fly, and what the fixes did.
#[derive(Debug, Default)]
pub struct Visit {
    visited: Option<Visited>,
    flight: Option<Spot>,
    /// The two models of the plate's row gone to, the same one twice for a row of one.
    row: Option<[ObjectId; 2]>,
    /// The kind whose card is to open, once.
    reveal: Option<IssueKind>,
    pub answers: Answers,
}

impl Visit {
    /// Goes to `places` on the layer whose section plane is `layer_mm`, ringed in `colour`
    /// while `stage` holds; the camera looks down on them through the cut.
    pub fn go(&mut self, places: &[Place], layer_mm: Scalar, stage: Stage, colour: Color32) {
        let rings: Vec<[Vec3; 2]> = places
            .iter()
            .flat_map(|place| ring(place.at_mm.extend(layer_mm + LIFT_MM), place.radius_mm()))
            .collect();
        self.flight = Spot::around(rings.iter().flatten().copied(), Vec3::Z);
        self.visited = Some(Visited {
            stage,
            rings,
            colour,
        });
    }

    /// Sends the camera to `spot` with nothing ringed: an issue of the plate, not of a layer.
    pub fn fly(&mut self, spot: Spot) {
        self.visited = None;
        self.flight = Some(spot);
    }

    /// Marks the plate's row of `models` as the one gone to.
    pub fn mark_row(&mut self, models: [ObjectId; 2]) {
        self.row = Some(models);
    }

    /// Asks for the card of `kind` to open.
    pub fn reveal(&mut self, kind: IssueKind) {
        self.reveal = Some(kind);
    }

    /// The kind whose card is to open, once.
    pub fn take_reveal(&mut self) -> Option<IssueKind> {
        self.reveal.take()
    }

    /// The plate's row gone to last.
    pub fn marked_row(&self) -> Option<[ObjectId; 2]> {
        self.row
    }

    /// Forgets the issues gone to; what the fixes did stands.
    pub fn clear(&mut self) {
        (self.visited, self.flight, self.row) = (None, None, None);
    }

    /// Where the camera is to go, once.
    pub fn take_flight(&mut self) -> Option<Spot> {
        self.flight.take()
    }

    /// The rings, while the plate and the cut are still the `stage` they were laid in.
    pub fn highlight(&self, stage: Stage) -> Highlight {
        let Some(visited) = self
            .visited
            .as_ref()
            .filter(|visited| visited.stage == stage)
        else {
            return Highlight::default();
        };
        Highlight {
            faces: Vec::new(),
            edges: vec![EdgeMark {
                edges: visited.rings.clone(),
                colour: visited.colour,
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUT: Stage = Stage {
        plate: 7,
        cut_mm: Some(3.0),
    };

    fn island(x: f32, area_mm2: f32) -> Place {
        Place {
            at_mm: Vec2::new(x, 0.0),
            area_mm2: Some(area_mm2),
        }
    }

    #[test]
    fn auto_orient_is_aimed_at_a_model_only_when_every_issue_is_on_it() {
        let unit = (Vec3::ZERO, Vec3::ONE);
        let scene = crate::scene::fixtures::scene_of_boxes(&[unit, unit]);
        let (one, other) = (scene.objects()[0].id, scene.objects()[1].id);
        assert_eq!(only_owner([Some(one), Some(one)]), Some(one));
        assert_eq!(only_owner([Some(one), Some(other)]), None);
        assert_eq!(only_owner([Some(one), None]), None);
        assert_eq!(only_owner([]), None);
    }

    #[test]
    fn an_island_is_ringed_on_the_cut_at_the_radius_of_its_area() {
        let mut visit = Visit::default();
        visit.go(
            &[island(10.0, std::f32::consts::PI * 4.0)],
            3.0,
            CUT,
            Color32::RED,
        );
        let highlight = visit.highlight(CUT);
        let centre = Vec3::new(10.0, 0.0, 3.0 + LIFT_MM);
        for end in highlight.edges[0].edges.iter().flatten() {
            assert!(
                (end.distance(centre) - 2.0).abs() < 1e-4,
                "4π mm² rings at 2 mm; got {end}"
            );
        }
        assert_eq!(highlight.edges[0].colour, Color32::RED);
    }

    #[test]
    fn the_camera_flies_once_to_every_issue_gone_to_seen_from_above() {
        let mut visit = Visit::default();
        visit.go(
            &[island(0.0, 0.0), island(10.0, 0.0)],
            3.0,
            CUT,
            Color32::RED,
        );
        let spot = visit.take_flight().expect("two islands");
        assert!((spot.center - Vec3::new(5.0, 0.0, 3.0 + LIFT_MM)).length() < 1e-4);
        assert!(
            (spot.radius_mm - (5.0 + LEAST_RING_MM)).abs() < 1e-3,
            "both rings in; got {}",
            spot.radius_mm
        );
        assert_eq!(spot.normal, Vec3::Z);
        assert!(visit.take_flight().is_none());
    }

    #[test]
    fn the_rings_go_once_the_cut_or_the_plate_moves_off_them() {
        let mut visit = Visit::default();
        visit.go(&[island(0.0, 1.0)], 3.0, CUT, Color32::RED);
        let moved_cut = |cut_mm| Stage { cut_mm, ..CUT };
        assert!(visit.highlight(moved_cut(Some(3.05))).edges.is_empty());
        assert!(visit.highlight(moved_cut(None)).edges.is_empty());
        let moved_model = Stage { plate: 8, ..CUT };
        assert!(visit.highlight(moved_model).edges.is_empty());
        assert!(Visit::default().highlight(CUT).edges.is_empty());
    }
}
