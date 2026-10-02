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

/// Packs models onto a plate `plate_mm` across, biggest first, and centres what it
/// packed.
///
/// Footprints are bitmaps rather than polygons, so a concave model nests inside another
/// one's bay without a polygon boolean anywhere in the workspace; see
/// `docs/decisions/0088`.
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

/// How far the packed block moves to sit in the middle of the plate, in cells.
///
/// Packing runs into the corner, because a part put in the middle first splits the plate
/// into two halves too small for the next one. What the user sees centred is the block,
/// moved once at the end.
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

/// The free position nearest the near left corner, in cells, or `None` when the part does
/// not fit anywhere.
///
/// Positions run along the rows from the corner outwards, so a part drops as low and as
/// far left as it will go, which is what leaves the rest of the plate in one piece.
/// `inner` is the plate without its margin; `gap` is the border the clearance is painted
/// into, and is what the part's own position is offset by when it is tested.
fn fit(part: &Mask, plate: &Mask, inner: (usize, usize), gap: usize) -> Option<(usize, usize)> {
    if part.width() > inner.0 || part.height() > inner.1 {
        return None;
    }

    (0..=inner.1 - part.height())
        .flat_map(|y| (0..=inner.0 - part.width()).map(move |x| (x, y)))
        .find(|(x, y)| !part.hits(plate, x + gap, y + gap))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Mesh, Transform, Vec3};

    /// Axis-aligned box, twelve triangles, spanning 0..size on every axis.
    fn cuboid(size: Vec3) -> Mesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(size.x, size.y, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(0.0, 0.0, size.z),
            Vec3::new(size.x, 0.0, size.z),
            Vec3::new(size.x, size.y, size.z),
            Vec3::new(0.0, size.y, size.z),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::new(vertices, faces)
    }

    /// `count` identical boxes, all stacked at the origin, which is the worst plate the
    /// arranger can be handed.
    fn heap(count: usize, size: Vec3, cell_mm: Scalar) -> Vec<Footprint> {
        let mesh = cuboid(size);
        (0..count)
            .map(|_| {
                Footprint::of(&mesh, Transform::default(), cell_mm).expect("a box has a footprint")
            })
            .collect()
    }

    /// Where a footprint ends up, as the rectangle it then covers in millimetres.
    fn rect(footprint: &Footprint, offset: Vec2) -> (Vec2, Vec2) {
        let min = footprint.corner_mm + offset;
        let size = Vec2::new(
            footprint.mask.width() as Scalar * footprint.cell_mm,
            footprint.mask.height() as Scalar * footprint.cell_mm,
        );
        (min, min + size)
    }

    fn overlap(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> bool {
        a.0.x < b.1.x && b.0.x < a.1.x && a.0.y < b.1.y && b.0.y < a.1.y
    }

    #[test]
    fn six_copies_are_spread_over_the_plate_without_touching() {
        let footprints = heap(6, Vec3::new(20.0, 20.0, 10.0), 1.0);
        let arranged = arrange(
            &footprints,
            Vec2::new(150.0, 80.0),
            &ArrangeSettings::default(),
        );

        assert_eq!(
            arranged.placed.len(),
            6,
            "six 20 mm boxes fit a 150 by 80 plate"
        );
        assert!(arranged.left_out.is_empty());

        for (first, placed) in arranged.placed.iter().enumerate() {
            let a = rect(&footprints[placed.index], placed.offset_mm);
            for other in &arranged.placed[first + 1..] {
                let b = rect(&footprints[other.index], other.offset_mm);
                assert!(!overlap(a, b), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn everything_lands_inside_the_plate_and_its_margin() {
        let footprints = heap(4, Vec3::new(30.0, 25.0, 10.0), 1.0);
        let settings = ArrangeSettings::default();
        let arranged = arrange(&footprints, Vec2::new(150.0, 80.0), &settings);

        for placed in &arranged.placed {
            let (min, max) = rect(&footprints[placed.index], placed.offset_mm);
            assert!(
                min.x >= settings.margin_mm - 1e-3 && min.y >= settings.margin_mm - 1e-3,
                "{min} is inside the margin"
            );
            assert!(
                max.x <= 150.0 - settings.margin_mm + 1.0
                    && max.y <= 80.0 - settings.margin_mm + 1.0,
                "{max} is inside the plate"
            );
        }
    }

    #[test]
    fn the_clearance_is_kept_between_neighbours() {
        let settings = ArrangeSettings {
            clearance_mm: 5.0,
            ..ArrangeSettings::default()
        };
        let footprints = heap(2, Vec3::new(20.0, 20.0, 10.0), 1.0);
        let arranged = arrange(&footprints, Vec2::new(150.0, 80.0), &settings);
        assert_eq!(arranged.placed.len(), 2);

        let a = rect(&footprints[0], arranged.placed[0].offset_mm);
        let b = rect(&footprints[1], arranged.placed[1].offset_mm);
        let gap_x = (b.0.x - a.1.x).max(a.0.x - b.1.x);
        let gap_y = (b.0.y - a.1.y).max(a.0.y - b.1.y);
        assert!(
            gap_x.max(gap_y) >= settings.clearance_mm - 1e-3,
            "expected {} mm between them, got {}",
            settings.clearance_mm,
            gap_x.max(gap_y)
        );
    }

    #[test]
    fn what_does_not_fit_is_named_rather_than_stacked() {
        let footprints = heap(3, Vec3::new(60.0, 60.0, 10.0), 1.0);
        let arranged = arrange(
            &footprints,
            Vec2::new(150.0, 80.0),
            &ArrangeSettings::default(),
        );

        assert_eq!(
            arranged.placed.len(),
            2,
            "two 60 mm boxes fit across 150 mm"
        );
        assert_eq!(arranged.left_out.len(), 1);
    }

    #[test]
    fn a_model_wider_than_the_plate_is_left_where_it_is() {
        let footprints = heap(1, Vec3::new(200.0, 20.0, 10.0), 1.0);
        let arranged = arrange(
            &footprints,
            Vec2::new(150.0, 80.0),
            &ArrangeSettings::default(),
        );
        assert!(arranged.placed.is_empty());
        assert_eq!(arranged.left_out, vec![0]);
    }

    #[test]
    fn a_plate_smaller_than_its_own_margin_takes_nothing() {
        let footprints = heap(1, Vec3::splat(2.0), 1.0);
        let arranged = arrange(
            &footprints,
            Vec2::new(3.0, 3.0),
            &ArrangeSettings::default(),
        );
        assert_eq!(arranged.left_out, vec![0]);
    }

    #[test]
    fn a_single_model_lands_in_the_middle() {
        let footprints = heap(1, Vec3::new(20.0, 20.0, 10.0), 1.0);
        let arranged = arrange(
            &footprints,
            Vec2::new(150.0, 80.0),
            &ArrangeSettings::default(),
        );

        let (min, max) = rect(&footprints[0], arranged.placed[0].offset_mm);
        let centre = (min + max) / 2.0;
        assert!(
            (centre - Vec2::new(75.0, 40.0)).length() < 2.0,
            "expected the middle of the plate, got {centre}"
        );
    }
}
