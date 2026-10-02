use crate::config::SampleConfig;
use crate::field::{Field, Grid};

/// What one layer has on it that nothing under it holds up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unsupported {
    /// Pieces with nothing whatever beneath them. Held however small they are: an island
    /// cures against the film alone, and the peel takes it away.
    pub islands: Vec<Field>,
    /// Pieces that lean further out than the surface below them can carry. Held where
    /// they are big enough to be worth a column.
    pub overhangs: Vec<Field>,
}

impl Unsupported {
    /// Whether the layer needs nothing at all. Only the tests ask: placement walks both
    /// lists, and walking two empty lists is already nothing.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.islands.is_empty() && self.overhangs.is_empty()
    }
}

/// What on `layer` needs holding up, given the layer under it and the layer a measured
/// rise below it.
///
/// Both questions are asked of the material that first appears on this layer — what
/// `below` does not already carry — because that is the only new underside the layer has.
/// An **island** is a piece of it that touches nothing the layer below holds up. An
/// **overhang** is the rest of it, wherever it has moved further sideways than `reach`
/// over the rise between `layer` and `reference`, which is the same thing as leaning
/// further than the profile's angle, measured over enough rise for the grid to resolve
/// it. Neither question is the overhang angle of a face; see `docs/decisions/0031`.
pub fn unsupported(
    layer: &Field,
    below: &Field,
    reference: &Field,
    grid: &Grid,
    config: &SampleConfig,
    rise_mm: f32,
) -> Unsupported {
    // A wall standing on itself adds no underside, which is most of most models and
    // costs one pass over the spans.
    let fresh = layer.without(below);
    if fresh.is_empty() {
        return Unsupported::default();
    }
    let carried = layer.without(&fresh);

    let mut islands = Vec::new();
    let mut ledges = Vec::new();
    for piece in fresh.pieces() {
        if piece.adjoins(&carried) {
            ledges.push(piece);
        } else if across_mm(&piece, grid) >= crate::config::MIN_ISLAND_MM {
            islands.push(piece);
        }
    }

    // How far the surface is allowed to have moved over this rise, which is the angle.
    let reach = grid.cells_of(config.reach_mm * rise_mm / crate::config::REFERENCE_RISE_MM);
    // What one layer may step out by on its own: a lean is measured over a rise, but a
    // ledge that appears in a single layer is no lean, it is a shelf over nothing.
    let step = grid.cells_of(config.min_overhang_mm);
    let overhangs = ledges
        .iter()
        .flat_map(|ledge| {
            ledge
                .uncarried_by(reference, reach)
                .with(&ledge.uncarried_by(below, step))
                .pieces()
        })
        .filter(|piece| across_mm(piece, grid) >= config.min_overhang_mm)
        .collect();

    Unsupported { islands, overhangs }
}

/// The longer side of a piece's bounding box, millimetres. What "smaller than" means to
/// the island filter.
fn across_mm(piece: &Field, grid: &Grid) -> f32 {
    piece.bounds().map_or(0.0, |(min, max)| {
        let cells = (max[0] - min[0] + 1).max(max[1] - min[1] + 1);
        grid.millimetres_of(cells)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GRID_PITCH_MM, REFERENCE_RISE_MM, SampleConfig};
    use core_geometry::{Scalar, Vec2};
    use core_slicer::{Contour, Layer};
    use printer_profiles::SupportProfile;

    fn grid() -> Grid {
        Grid::covering(Vec2::ZERO, Vec2::splat(60.0), GRID_PITCH_MM)
    }

    fn config() -> SampleConfig {
        SampleConfig::for_profile(&crate::tests::profile())
    }

    /// One axis-aligned rectangle as a layer, read onto the grid.
    fn rectangle(x: Scalar, y: Scalar, width: Scalar, height: Scalar) -> Field {
        let points = vec![
            Vec2::new(x, y),
            Vec2::new(x + width, y),
            Vec2::new(x + width, y + height),
            Vec2::new(x, y + height),
        ];
        let layer = Layer {
            z: 1.0,
            contours: vec![Contour::from_points(points).expect("a rectangle encloses area")],
            extra: Vec::new(),
        };
        Field::of_layer(&layer, &grid())
    }

    fn square(x: Scalar, y: Scalar, side: Scalar) -> Field {
        rectangle(x, y, side, side)
    }

    /// The default rise, so that the reach is the profile's angle at face value.
    fn found(layer: &Field, below: &Field, reference: &Field) -> Unsupported {
        unsupported(
            layer,
            below,
            reference,
            &grid(),
            &config(),
            REFERENCE_RISE_MM,
        )
    }

    #[test]
    fn a_wall_standing_on_itself_needs_nothing() {
        let wall = square(10.0, 10.0, 10.0);
        assert!(found(&wall, &wall, &wall).is_empty());
    }

    /// A wall that leans out by `step` millimetres over the reference rise, as the three
    /// fields placement reads: the layer, the layer under it, and the reference. A lean
    /// moves under a cell per layer, so the layer below is one cell back — which is the
    /// whole point of measuring the angle over a rise; see `docs/decisions/0031`.
    fn leaning(step: Scalar) -> (Field, Field, Field) {
        (
            square(10.0, 10.0, 10.0 + step),
            square(10.0, 10.0, 10.0 + step - GRID_PITCH_MM),
            square(10.0, 10.0, 10.0),
        )
    }

    #[test]
    fn a_surface_leaning_less_than_the_angle_holds_itself_up() {
        // A millimetre of rise moves the wall 0.9 mm, inside the millimetre a 45 degree
        // lean is allowed.
        let (layer, below, reference) = leaning(0.9);
        assert!(found(&layer, &below, &reference).overhangs.is_empty());
    }

    #[test]
    fn a_step_too_wide_to_bridge_is_an_overhang_however_gentle_the_lean() {
        // The same 0.9 mm, taken in one layer: over a millimetre of rise that is a lean
        // the profile allows, but in one layer it is a shelf over nothing.
        let below = square(10.0, 10.0, 10.0);
        let layer = square(10.0, 10.0, 10.9);
        assert!(!found(&layer, &below, &below).overhangs.is_empty());
    }

    #[test]
    fn a_surface_leaning_further_than_the_angle_is_an_overhang() {
        let below = square(10.0, 10.0, 10.0);
        let layer = square(10.0, 10.0, 14.0);
        let overhangs = found(&layer, &below, &below).overhangs;

        assert_eq!(overhangs.len(), 1, "the ledge is one piece");
        let held = overhangs[0].area_mm2(&grid());
        // Everything past the millimetre of reach: a 14 mm square less an 11 mm one.
        let expected = 14.0 * 14.0 - 11.0 * 11.0;
        assert!(
            (held - expected).abs() / expected < 0.05,
            "expected about {expected} mm2 of ledge, got {held}"
        );
    }

    #[test]
    fn a_part_with_nothing_under_it_is_an_island() {
        let below = square(10.0, 10.0, 10.0);
        let layer = square(10.0, 10.0, 10.0).with(&square(40.0, 40.0, 5.0));

        let found = found(&layer, &below, &below);
        assert_eq!(found.islands.len(), 1, "one piece stands on nothing");
        let island = found.islands[0].area_mm2(&grid());
        assert!(
            (island - 25.0).abs() < 1.0,
            "the whole 5 mm island, got {island}"
        );
    }

    #[test]
    fn the_first_layer_of_a_stack_is_all_island() {
        let layer = square(10.0, 10.0, 10.0);
        let found = found(&layer, &Field::default(), &Field::default());
        assert_eq!(found.islands.len(), 1);
    }

    #[test]
    fn a_speck_too_small_to_print_is_not_an_island_worth_holding() {
        let below = square(10.0, 10.0, 10.0);
        let speck = square(40.0, 40.0, 0.1);
        let layer = below.with(&speck);
        assert!(found(&layer, &below, &below).islands.is_empty());
    }

    #[test]
    fn a_ceiling_closing_over_a_bore_is_an_overhang() {
        let mut bore = square(12.0, 12.0, 6.0);
        // A ring: the 20 mm block with a 6 mm hole through it.
        bore = square(10.0, 10.0, 20.0).without(&bore);
        let layer = square(10.0, 10.0, 20.0);

        let overhangs = found(&layer, &bore, &bore).overhangs;
        assert_eq!(overhangs.len(), 1, "the roof over the bore");
        let roof = overhangs[0].area_mm2(&grid());
        // The whole 6 mm hole less the rim the wall around it carries, which is the step
        // the resin bridges rather than the millimetre of reach.
        assert!(
            (16.0..36.0).contains(&roof),
            "expected most of a 36 mm2 hole, got {roof}"
        );
    }

    #[test]
    fn a_shallower_angle_finds_more_to_hold() {
        let (layer, below, reference) = leaning(0.9);
        let strict = SampleConfig::for_profile(&SupportProfile {
            max_overhang_deg: 20.0,
            ..crate::tests::profile()
        });

        assert!(
            found(&layer, &below, &reference).overhangs.is_empty(),
            "45 degrees lets a 0.9 mm lean over a millimetre of rise pass"
        );
        assert!(
            !unsupported(
                &layer,
                &below,
                &reference,
                &grid(),
                &strict,
                REFERENCE_RISE_MM
            )
            .overhangs
            .is_empty(),
            "20 degrees does not"
        );
    }

    #[test]
    fn a_shorter_rise_scales_the_reach_it_allows() {
        // Half the rise, so half the ground a lean may cover.
        let below = square(10.0, 10.0, 10.0);
        let layer = square(10.0, 10.0, 10.7);
        assert!(
            found(&layer, &below, &below).overhangs.is_empty(),
            "0.7 mm over a millimetre of rise is under 45 degrees"
        );
        assert!(
            !unsupported(
                &layer,
                &below,
                &below,
                &grid(),
                &config(),
                REFERENCE_RISE_MM / 2.0
            )
            .overhangs
            .is_empty(),
            "the same step over half the rise is over it"
        );
    }

    #[test]
    fn a_layer_of_nothing_needs_nothing() {
        let below = square(10.0, 10.0, 10.0);
        assert!(found(&Field::default(), &below, &below).is_empty());
    }
}
