use std::fmt;

use core_geometry::{Scalar, Vec2};
use core_slicer::{Layer, LayerPlan, ONE_SAMPLE, SliceSettings, Sliced};
use core_supports::{TrapScan, Trapped};

/// What the slicing run produced, ready to print: counters folded in a window at a time,
/// so a report costs the same whatever the stack (ADR 0066).
#[derive(Debug)]
pub struct SliceReport {
    pub settings: SliceSettings,
    plan: LayerPlan,
    layers: usize,
    contours: usize,
    widest_layer: usize,
    empty_layers: usize,
    first_z: Option<Scalar>,
    last_z: Option<Scalar>,
    /// Resin folded in a layer at a time, because a layer is priced at its own thickness
    /// and that is not one number on an adaptive plan.
    volume_mm3: Scalar,
    open_contours: usize,
    degenerate_contours: usize,
    unlinked_segments: usize,
    /// The drainage scan, while it is still reading layers, and what it found once it is
    /// finished. Neither holds a stack; see ADR 0072.
    drainage: Option<TrapScan>,
    trapped: Vec<Trapped>,
}

impl SliceReport {
    pub fn new(settings: SliceSettings, plan: LayerPlan) -> Self {
        Self {
            settings,
            plan,
            layers: 0,
            contours: 0,
            widest_layer: 0,
            empty_layers: 0,
            first_z: None,
            last_z: None,
            volume_mm3: 0.0,
            open_contours: 0,
            degenerate_contours: 0,
            unlinked_segments: 0,
            drainage: None,
            trapped: Vec::new(),
        }
    }

    /// Also watch the stack for resin that cannot get out of a model spanning `min`..`max`
    /// in plate millimetres.
    pub fn watch_drainage(&mut self, min: Vec2, max: Vec2) {
        self.drainage = Some(TrapScan::new(min, max, self.settings.layer_height));
    }

    /// Closes the drainage scan, after which the report knows what is trapped.
    pub fn finish_drainage(&mut self) {
        if let Some(scan) = self.drainage.take() {
            self.trapped = scan.finish();
        }
    }

    /// Folds one window of the stack in. Windows arrive in print order.
    pub fn absorb(&mut self, sliced: &Sliced) {
        self.volume_mm3 += sliced.resin_volume_mm3(&self.plan, self.layers);
        for layer in &sliced.layers {
            if let Some(scan) = self.drainage.as_mut() {
                scan.push(layer);
            }
            self.layers += 1;
            // Every plane's contours are counted: the work of sampling is what the figure
            // is for, and only the rasteriser sees the extra planes.
            let contours = layer.contours.len() + layer.extra.iter().map(Vec::len).sum::<usize>();
            self.contours += contours;
            self.widest_layer = self.widest_layer.max(contours);
            if Layer::is_empty(layer) {
                self.empty_layers += 1;
            }
            self.first_z.get_or_insert(layer.z);
            self.last_z = Some(layer.z);
        }
        self.open_contours += sliced.open_contours;
        self.degenerate_contours += sliced.degenerate_contours;
        self.unlinked_segments += sliced.unlinked_segments;
    }

    /// True when every contour closed on the mesh's own topology.
    pub fn is_clean(&self) -> bool {
        self.open_contours == 0
            && self.degenerate_contours == 0
            && self.unlinked_segments == 0
            && self.trapped.is_empty()
    }

    pub fn layer_count(&self) -> usize {
        self.layers
    }

    pub fn open_contours(&self) -> usize {
        self.open_contours
    }

    /// The pockets the drainage scan found with no way out, empty when none was asked for.
    pub fn trapped(&self) -> &[Trapped] {
        &self.trapped
    }

    /// Resin the part consumes, cubic millimetres.
    ///
    /// Contour areas are signed, so a hole runs clockwise and subtracts itself.
    pub fn resin_volume_mm3(&self) -> Scalar {
        self.volume_mm3.max(0.0)
    }
}

impl fmt::Display for SliceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.plan.is_uniform() {
            writeln!(f, "  layer height  {:.3} mm", self.settings.layer_height)?;
        } else {
            writeln!(
                f,
                "  layer height  {:.3} mm at most, adaptive",
                self.settings.layer_height
            )?;
        }
        writeln!(f, "  layers        {}", self.layers)?;
        if self.settings.samples != ONE_SAMPLE {
            writeln!(
                f,
                "  sampling      {} planes a layer, united",
                self.settings.samples
            )?;
        }
        writeln!(
            f,
            "  contours      {} (up to {} per layer)",
            self.contours, self.widest_layer
        )?;

        if let (Some(first), Some(last)) = (self.first_z, self.last_z) {
            writeln!(f, "  sliced from   {first:.3} to {last:.3} mm")?;
        }
        writeln!(f, "  sliced volume {:.3} mm^3", self.resin_volume_mm3())?;

        for trapped in &self.trapped {
            writeln!(
                f,
                "  trapped resin {:.1} mm^3 at {:.1}, {:.1}, {:.1} mm",
                trapped.volume_mm3, trapped.at.x, trapped.at.y, trapped.at.z
            )?;
        }
        for problem in self.problems() {
            writeln!(f, "  slice defect  {problem}")?;
        }
        Ok(())
    }
}

impl SliceReport {
    fn problems(&self) -> Vec<String> {
        let counts = [
            (self.open_contours, "contours closed over a gap"),
            (self.degenerate_contours, "contours with no area"),
            (self.unlinked_segments, "segments on a branching edge"),
            (self.empty_layers, "empty layers"),
        ];
        counts
            .into_iter()
            .filter(|(count, _)| *count > 0)
            .map(|(count, label)| format!("{count} {label}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec2;
    use core_slicer::{Contour, Winding};

    fn square_layer(z: Scalar) -> Layer {
        Layer::new(
            z,
            vec![Contour::new(
                vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(2.0, 0.0),
                    Vec2::new(2.0, 2.0),
                    Vec2::new(0.0, 2.0),
                ],
                Winding::Outer,
            )],
        )
    }

    fn report(sliced: Sliced) -> SliceReport {
        let mut report = SliceReport::new(
            SliceSettings {
                layer_height: 0.5,
                ..SliceSettings::default()
            },
            LayerPlan::of_count(0.5, 8),
        );
        report.absorb(&sliced);
        report
    }

    #[test]
    fn a_clean_run_reports_no_defects() {
        let text = report(Sliced {
            layers: vec![square_layer(0.25), square_layer(0.75)],
            ..Sliced::default()
        })
        .to_string();

        assert!(text.contains("layers        2"));
        // Two 2x2 mm squares 0.5 mm apart enclose 4 mm^3.
        assert!(text.contains("sliced volume 4.000 mm^3"), "{text}");
        assert!(!text.contains("slice defect"));
    }

    #[test]
    fn windows_add_up_to_what_one_stack_would_have_said() {
        let whole = report(Sliced {
            layers: vec![square_layer(0.25), square_layer(0.75), square_layer(1.25)],
            open_contours: 2,
            ..Sliced::default()
        });

        let mut windowed = SliceReport::new(
            SliceSettings {
                layer_height: 0.5,
                ..SliceSettings::default()
            },
            LayerPlan::of_count(0.5, 8),
        );
        windowed.absorb(&Sliced {
            layers: vec![square_layer(0.25), square_layer(0.75)],
            open_contours: 1,
            ..Sliced::default()
        });
        windowed.absorb(&Sliced {
            layers: vec![square_layer(1.25)],
            open_contours: 1,
            ..Sliced::default()
        });

        assert_eq!(windowed.to_string(), whole.to_string());
    }

    #[test]
    fn a_hole_is_subtracted_from_the_volume() {
        let mut layer = square_layer(0.25);
        // A 1x1 mm hole inside the 2x2 mm square, wound clockwise.
        layer.contours.push(Contour::new(
            vec![
                Vec2::new(0.5, 0.5),
                Vec2::new(0.5, 1.5),
                Vec2::new(1.5, 1.5),
                Vec2::new(1.5, 0.5),
            ],
            Winding::Inner,
        ));

        let report = report(Sliced {
            layers: vec![layer],
            ..Sliced::default()
        });
        // (4 - 1) mm^2 over one 0.5 mm layer.
        assert!((report.resin_volume_mm3() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn every_kind_of_defect_is_named() {
        let text = report(Sliced {
            layers: vec![square_layer(0.25), Layer::empty(0.75)],
            open_contours: 2,
            degenerate_contours: 3,
            unlinked_segments: 4,
        })
        .to_string();

        assert!(text.contains("2 contours closed over a gap"));
        assert!(text.contains("3 contours with no area"));
        assert!(text.contains("4 segments on a branching edge"));
        assert!(text.contains("1 empty layers"));
    }

    #[test]
    fn a_run_with_a_papered_over_contour_is_not_clean() {
        assert!(
            !report(Sliced {
                open_contours: 1,
                ..Sliced::default()
            })
            .is_clean()
        );
    }
}
