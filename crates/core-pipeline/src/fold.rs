use std::borrow::Cow;

use core_analysis::{Measured, Stretch, cure};
use core_raster::{LayerRuns, RasterSettings, Rasterizer, ScanlineRasterizer};
use core_slicer::{Layer, LayerPlan, offset_contours};
use printer_profiles::{Compensation, MaterialProfile};
use rayon::prelude::*;

use crate::error::PipelineError;

/// How far a layer's walls move before it is rasterised, and where the bottom block that
/// moves differently ends; see `docs/design/compensation.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Tolerance {
    pub compensation: Compensation,
    pub bottom_layers: u32,
}

impl Tolerance {
    /// What the resin in hand asks for.
    pub fn of(material: &MaterialProfile) -> Self {
        Self {
            compensation: material.compensation,
            bottom_layers: material.bottom_layers,
        }
    }

    /// Whether every wall stays where the slicer put it.
    pub fn moves_nothing(&self) -> bool {
        self.compensation.offsets_nothing()
    }

    /// `layer` with the offsets of its index applied, or the layer itself when none move.
    fn applied<'a>(&self, layer: &'a Layer, index: u32) -> Cow<'a, Layer> {
        if self.moves_nothing() {
            return Cow::Borrowed(layer);
        }
        let [hole_mm, outer_mm] = self
            .compensation
            .offsets_of_layer_mm(index, index < self.bottom_layers);
        Cow::Owned(Layer {
            z: layer.z,
            contours: offset_contours(&layer.contours, hole_mm, outer_mm),
            extra: layer
                .extra
                .iter()
                .map(|plane| offset_contours(plane, hole_mm, outer_mm))
                .collect(),
        })
    }
}

/// One rasterised layer as it leaves the fold, ready to be written.
pub struct Folded {
    pub runs: LayerRuns,
    /// What the fold decided has to come out of this layer before it is written: the
    /// islands, when the run was asked to take them.
    pub taken: Vec<Stretch>,
    /// How far past the edge of the panel this layer reached, pixels.
    pub overflow_px: f32,
}

impl Folded {
    /// The layer as it goes into the file, with whatever the fold took out of it gone.
    pub fn written(&self) -> std::borrow::Cow<'_, LayerRuns> {
        if self.taken.is_empty() {
            std::borrow::Cow::Borrowed(&self.runs)
        } else {
            std::borrow::Cow::Owned(core_analysis::erase(&self.runs, &self.taken))
        }
    }
}

/// Rasterises `layers` in parallel and folds them into `measured` in print order, which
/// is where an island is taken out.
///
/// Print order is why the fold is not parallel: what a layer stands on is only known once
/// the layer under it has been folded. See `docs/design/analysis.md`.
pub fn fold_group(
    layers: &[Layer],
    settings: &RasterSettings,
    plan: &LayerPlan,
    tolerance: &Tolerance,
    measured: &mut Measured,
) -> Result<Vec<Folded>, PipelineError> {
    // The group is the next layers in print order, so their indices follow the fold's.
    let first = measured.layer_count();
    let rastered: Vec<_> = layers
        .par_iter()
        .enumerate()
        .map(|(step, layer)| {
            let offset = tolerance.applied(layer, (first + step) as u32);
            let rastered = ScanlineRasterizer
                .rasterize(offset.as_ref(), settings)
                .map_err(|source| PipelineError::Raster { z: layer.z, source })?;
            let cured = cure(&rastered.runs, settings.pitch);
            Ok((rastered, cured))
        })
        .collect::<Result<_, PipelineError>>()?;

    Ok(rastered
        .into_iter()
        .map(|(rastered, cured)| {
            let thickness_mm = plan.thickness_of(measured.layer_count()).unwrap_or(0.0);
            let taken = measured.push(cured, thickness_mm);
            Folded {
                runs: rastered.runs,
                taken,
                overflow_px: rastered.overflow_px,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec2;
    use core_raster::{Grey, PixelPitch, Shading};
    use core_slicer::{Contour, Winding};
    use printer_profiles::Compensation;

    /// A 1 mm panel at 100 microns a pixel, so a pixel is easy to count.
    fn panel() -> RasterSettings {
        RasterSettings {
            width_px: 100,
            height_px: 100,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            shading: Shading::Binary,
            grey: Grey::default(),
            blur_px: 0,
            mirror_x: false,
            mirror_y: false,
        }
    }

    /// One layer holding a 4 mm square in the middle of that panel.
    fn square_layer() -> Layer {
        let (low, high) = (3.0, 7.0);
        Layer::new(
            0.05,
            vec![Contour::new(
                vec![
                    Vec2::new(low, low),
                    Vec2::new(high, low),
                    Vec2::new(high, high),
                    Vec2::new(low, high),
                ],
                Winding::Outer,
            )],
        )
    }

    /// Pixels the fold's first layer lit.
    fn lit(tolerance: &Tolerance, index: u32) -> usize {
        let plan = LayerPlan::of_count(0.05, index as usize + 1);
        let mut measured = Measured::new(0);
        for _ in 0..index {
            fold_group(&[square_layer()], &panel(), &plan, tolerance, &mut measured)
                .expect("a square rasterises");
        }
        let folded = fold_group(&[square_layer()], &panel(), &plan, tolerance, &mut measured)
            .expect("a square rasterises");
        folded[0]
            .runs
            .runs()
            .iter()
            .filter(|run| run.value > 0)
            .map(|run| run.length as usize)
            .sum()
    }

    #[test]
    fn an_outer_offset_lights_more_of_the_panel() {
        let plain = lit(&Tolerance::default(), 0);
        let grown = lit(
            &Tolerance {
                compensation: Compensation {
                    outer_offset_mm: 0.5,
                    ..Compensation::default()
                },
                bottom_layers: 0,
            },
            0,
        );
        // A 4 mm square grown half a millimetre a side is 5 mm across, less the four
        // corners a bevel join cuts off: 50 by 50 pixels minus four 5-pixel triangles.
        assert_eq!(plain, 40 * 40);
        assert_eq!(grown, 50 * 50 - 4 * 5 * 5 / 2);
    }

    #[test]
    fn the_bottom_block_takes_its_own_offset() {
        let tolerance = Tolerance {
            compensation: Compensation {
                outer_offset_mm: 0.0,
                bottom_outer_offset_mm: -0.5,
                ..Compensation::default()
            },
            bottom_layers: 2,
        };
        assert_eq!(lit(&tolerance, 0), 30 * 30, "layer one is a bottom layer");
        assert_eq!(lit(&tolerance, 2), 40 * 40, "layer three is not");
    }

    #[test]
    fn the_parity_offset_lands_on_every_other_layer() {
        let tolerance = Tolerance {
            compensation: Compensation {
                parity_offset_mm: 0.5,
                ..Compensation::default()
            },
            bottom_layers: 0,
        };
        assert_eq!(
            lit(&tolerance, 0),
            40 * 40,
            "the first layer takes no parity"
        );
        assert_eq!(
            lit(&tolerance, 1),
            50 * 50 - 4 * 5 * 5 / 2,
            "the second does"
        );
    }
}
