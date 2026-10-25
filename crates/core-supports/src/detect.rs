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
    /// Pieces with nothing beneath them that are too small to be islands yet: held where
    /// they began once what grows out of them is more than dust.
    pub specks: Vec<Field>,
    /// Shelves put out in this one layer, apart from `overhangs` only when steps are asked
    /// for: what a step is held by and sized by. See `docs/decisions/0326`.
    pub steps: Vec<Field>,
    /// Pieces leaning past the step's gentler angle and not past the profile's: held only
    /// where a step under them reaches. Found only when steps are asked for.
    pub gentle: Vec<Field>,
    /// Pieces leaning past the steadying angle and not past the step's: a slope a part is steadied
    /// on, sparsely; found only when steps are asked for (ADR 0326).
    pub slopes: Vec<Field>,
}

impl Unsupported {
    /// Whether the layer needs nothing at all.
    pub fn is_empty(&self) -> bool {
        self.islands.is_empty()
            && self.overhangs.is_empty()
            && self.specks.is_empty()
            && self.steps.is_empty()
            && self.gentle.is_empty()
            && self.slopes.is_empty()
    }
}

/// What on `layer` needs holding up given the layers below: islands touch nothing, overhangs
/// moved past `reach` over the rise (ADR 0031); `leans_deg` adds steps and steadying slopes.
pub fn unsupported(
    layer: &Field,
    below: &Field,
    reference: &Field,
    grid: &Grid,
    config: &SampleConfig,
    rise_mm: f32,
    leans_deg: Option<[f32; 2]>,
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
    let mut specks = Vec::new();
    for piece in fresh.pieces() {
        if piece.adjoins(&carried) {
            ledges.push(piece);
        } else if across_mm(&piece, grid) >= crate::config::MIN_ISLAND_MM {
            islands.push(piece);
        } else {
            specks.push(piece);
        }
    }

    // How far the surface is allowed to have moved over this rise, which is the angle.
    let reach = grid.cells_of(config.reach_mm * rise_mm / crate::config::REFERENCE_RISE_MM);
    // What one layer may step out by on its own: a lean is measured over a rise, but a
    // ledge that appears in a single layer is no lean, it is a shelf over nothing.
    let step = grid.cells_of(config.min_overhang_mm);
    let worth = |piece: &Field| across_mm(piece, grid) >= config.min_overhang_mm;
    let Some([lean_deg, steady_deg]) = leans_deg else {
        let overhangs = ledges
            .iter()
            .flat_map(|ledge| {
                ledge
                    .uncarried_by(reference, reach)
                    .with(&ledge.uncarried_by(below, step))
                    .pieces()
            })
            .filter(worth)
            .collect();
        return Unsupported {
            islands,
            overhangs,
            specks,
            ..Unsupported::default()
        };
    };

    let reach_of = |deg: f32| grid.cells_of(rise_mm * deg.to_radians().tan());
    let (gentle_reach, steady_reach) = (reach_of(lean_deg), reach_of(steady_deg));
    let (mut overhangs, mut steps, mut gentle) = (Vec::new(), Vec::new(), Vec::new());
    let mut slopes = Vec::new();
    for ledge in &ledges {
        let shelf = ledge.uncarried_by(below, step);
        let leaning = ledge.uncarried_by(reference, reach);
        let held = leaning.with(&shelf);
        steps.extend(shelf.pieces().into_iter().filter(worth));
        overhangs.extend(leaning.without(&shelf).pieces().into_iter().filter(worth));
        // The foot of a shelf, which the wall under it carries, is the shelf's own.
        let near = ledge
            .uncarried_by(reference, gentle_reach)
            .without(&held)
            .uncarried_by(&shelf, step);
        let sloped = ledge
            .uncarried_by(reference, steady_reach)
            .without(&near)
            .without(&held)
            .uncarried_by(&shelf, step);
        gentle.extend(near.pieces().into_iter().filter(worth));
        slopes.extend(sloped.pieces().into_iter().filter(worth));
    }
    Unsupported {
        islands,
        overhangs,
        specks,
        steps,
        gentle,
        slopes,
    }
}

/// The longer side of a piece's bounding box, millimetres. What "smaller than" means to
/// the island filter.
pub(crate) fn across_mm(piece: &Field, grid: &Grid) -> f32 {
    piece.bounds().map_or(0.0, |(min, max)| {
        let cells = (max[0] - min[0] + 1).max(max[1] - min[1] + 1);
        grid.millimetres_of(cells)
    })
}

#[cfg(test)]
mod tests;
