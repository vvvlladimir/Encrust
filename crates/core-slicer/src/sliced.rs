use core_geometry::Scalar;

use crate::{Contour, Layer, LayerPlan};

/// The outcome of a slicing run: the layers, and what the mesh forced the slicer to
/// paper over. A sound mesh produces zeroes in every counter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sliced {
    /// Layers ordered by height, lowest first.
    pub layers: Vec<Layer>,
    /// Contours closed by a straight jump because the chain of faces ran out.
    pub open_contours: usize,
    /// Contours dropped because they enclosed no area.
    pub degenerate_contours: usize,
    /// Crossings dropped because their edge already carried one: the surface branches.
    pub unlinked_segments: usize,
}

impl Sliced {
    /// True when every contour closed on mesh topology alone.
    pub fn is_clean(&self) -> bool {
        self.open_contours == 0 && self.degenerate_contours == 0 && self.unlinked_segments == 0
    }

    pub fn contour_count(&self) -> usize {
        self.layers.iter().map(|layer| layer.contours.len()).sum()
    }

    /// Resin the sliced part consumes, cubic millimetres, where these layers are the
    /// `first` and the ones after it of `plan`.
    ///
    /// Contour areas are signed, so a hole runs clockwise and subtracts itself. Each
    /// layer is priced at its own thickness, which is not one number on an adaptive plan.
    pub fn resin_volume_mm3(&self, plan: &LayerPlan, first: usize) -> Scalar {
        let volume: Scalar = self
            .layers
            .iter()
            .enumerate()
            .map(|(offset, layer)| {
                let area = layer
                    .contours
                    .iter()
                    .map(Contour::signed_double_area)
                    .sum::<Scalar>()
                    / 2.0;
                area * plan.thickness_of(first + offset).unwrap_or(0.0)
            })
            .sum();
        volume.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec2;

    /// Axis-aligned square of side `side`, wound counter-clockwise, at the origin.
    fn square(side: Scalar) -> Contour {
        let points = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(side, 0.0),
            Vec2::new(side, side),
            Vec2::new(0.0, side),
        ];
        Contour::from_points(points).expect("a square encloses an area")
    }

    #[test]
    fn a_run_without_defects_is_clean() {
        let sliced = Sliced {
            layers: vec![Layer::empty(0.1)],
            ..Sliced::default()
        };
        assert!(sliced.is_clean());
        assert_eq!(sliced.contour_count(), 0);
    }

    #[test]
    fn the_volume_is_the_layer_areas_times_the_layer_height() {
        let sliced = Sliced {
            layers: vec![
                Layer::new(0.25, vec![square(2.0)]),
                Layer::new(0.75, vec![square(2.0)]),
            ],
            ..Sliced::default()
        };
        // Two layers of a 2 x 2 mm square, half a millimetre thick: 2 * 2 * 0.5 * 2.
        assert!((sliced.resin_volume_mm3(&LayerPlan::of_count(0.5, 2), 0) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn a_hole_subtracts_itself_from_the_volume() {
        let mut points = square(1.0).points;
        points.reverse();
        let hole = Contour::new(points, crate::Winding::Inner);
        let sliced = Sliced {
            layers: vec![Layer::new(0.5, vec![square(2.0), hole])],
            ..Sliced::default()
        };
        // A 2 x 2 mm square minus a 1 x 1 mm hole, one millimetre thick.
        assert!((sliced.resin_volume_mm3(&LayerPlan::of_count(1.0, 1), 0) - 3.0).abs() < 1e-4);
    }

    #[test]
    fn any_papered_over_defect_makes_the_run_unclean() {
        for sliced in [
            Sliced {
                open_contours: 1,
                ..Sliced::default()
            },
            Sliced {
                degenerate_contours: 1,
                ..Sliced::default()
            },
            Sliced {
                unlinked_segments: 1,
                ..Sliced::default()
            },
        ] {
            assert!(!sliced.is_clean());
        }
    }
}
