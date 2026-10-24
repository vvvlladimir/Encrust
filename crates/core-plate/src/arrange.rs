use crate::mask::{Footprint, Mask};
use core_geometry::{Scalar, Vec2};

/// What packing the plate is allowed to assume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrangeSettings {
    /// How wide one cell of the packing grid is, millimetres. Packing is exact to this,
    /// and the cost of the search follows its square.
    pub cell_mm: Scalar,
    /// How much room is left between one model and the next, millimetres.
    pub clearance_mm: Scalar,
    /// How far in from the edge of the plate a model has to stay, millimetres.
    pub margin_mm: Scalar,
}

impl Default for ArrangeSettings {
    fn default() -> Self {
        Self {
            cell_mm: 1.0,
            clearance_mm: 3.0,
            margin_mm: 2.0,
        }
    }
}

/// Where one model ends up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Which of the footprints given to [`arrange`] this is.
    pub index: usize,
    /// How far the model moves across the plate, millimetres. Height is not touched.
    pub offset_mm: Vec2,
}

/// The plate as packing left it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Arranged {
    pub placed: Vec<Placed>,
    /// Footprints that found no room, in the order they were given.
    pub left_out: Vec<usize>,
}

/// Footprints are bitmaps so concave models nest without a polygon boolean (ADR 0088).
pub fn arrange(footprints: &[Footprint], plate_mm: Vec2, settings: &ArrangeSettings) -> Arranged {
    let cell_mm = settings.cell_mm.max(Scalar::EPSILON);
    let margin = (settings.margin_mm / cell_mm).ceil().max(0.0) as usize;
    let width = (plate_mm.x / cell_mm).floor().max(0.0) as usize;
    let height = (plate_mm.y / cell_mm).floor().max(0.0) as usize;
    if width <= 2 * margin || height <= 2 * margin {
        return Arranged {
            left_out: (0..footprints.len()).collect(),
            ..Arranged::default()
        };
    }

    let inner = (width - 2 * margin, height - 2 * margin);
    let gap = (settings.clearance_mm / cell_mm).ceil().max(0.0) as usize;
    // The plate carries a border of `gap` cells so that a clearance painted around a part
    // at the edge has somewhere to land.
    let mut plate = Mask::empty(inner.0 + 2 * gap, inner.1 + 2 * gap);

    let mut order: Vec<usize> = (0..footprints.len()).collect();
    order.sort_by_key(|index| std::cmp::Reverse(footprints[*index].area_cells()));

    let mut packed: Vec<(usize, usize, usize)> = Vec::new();
    let mut left_out = Vec::new();
    for index in order {
        let footprint = &footprints[index];
        match fit(&footprint.mask, &plate, inner, gap) {
            Some((x, y)) => {
                footprint.mask.grown(gap).paint(&mut plate, x, y);
                packed.push((index, x, y));
            }
            None => left_out.push(index),
        }
    }

    let shift = centring(&packed, footprints, inner);
    let mut arranged = Arranged {
        placed: packed
            .into_iter()
            .map(|(index, x, y)| {
                let corner = Vec2::new(
                    ((x + margin + shift.0) as Scalar) * cell_mm,
                    ((y + margin + shift.1) as Scalar) * cell_mm,
                );
                Placed {
                    index,
                    offset_mm: corner - footprints[index].corner_mm,
                }
            })
            .collect(),
        left_out,
    };

    arranged.placed.sort_by_key(|placed| placed.index);
    arranged.left_out.sort_unstable();
    arranged
}

/// How far the packed block moves to sit in the middle of the plate, in cells. Packing runs into
/// the corner, since a middle part splits the plate in two halves too small.
fn centring(
    packed: &[(usize, usize, usize)],
    footprints: &[Footprint],
    inner: (usize, usize),
) -> (usize, usize) {
    let used = packed.iter().fold((0, 0), |(w, h), (index, x, y)| {
        let mask = &footprints[*index].mask;
        ((x + mask.width()).max(w), (y + mask.height()).max(h))
    });
    (
        inner.0.saturating_sub(used.0) / 2,
        inner.1.saturating_sub(used.1) / 2,
    )
}

/// The free position nearest the near left corner, in cells, or `None` when the part does not fit.
/// `inner` is the plate without its margin; `gap` is the border the clearance is painted into.
fn fit(part: &Mask, plate: &Mask, inner: (usize, usize), gap: usize) -> Option<(usize, usize)> {
    if part.width() > inner.0 || part.height() > inner.1 {
        return None;
    }

    (0..=inner.1 - part.height())
        .flat_map(|y| (0..=inner.0 - part.width()).map(move |x| (x, y)))
        .find(|(x, y)| !part.hits(plate, x + gap, y + gap))
}

#[cfg(test)]
mod tests;
