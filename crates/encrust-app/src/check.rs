//! The issues the Check tool last went to: ringed on the plate at the height of their
//! layer while the view stays as it was left, and the camera sent to them once.

use core_analysis::{Risk, RiskKind};
use core_geometry::{Scalar, Vec2, Vec3};
use core_raster::RasterSettings;
use egui::Color32;

use crate::highlight::{EdgeMark, Highlight, LIFT_MM, Spot, ring};
use crate::preview::on_plate;

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

/// The issues gone to, and where the camera is still to fly.
#[derive(Debug, Default)]
pub struct Visit {
    visited: Option<Visited>,
    flight: Option<Spot>,
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

    /// Forgets the issues gone to.
    pub fn clear(&mut self) {
        *self = Self::default();
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
