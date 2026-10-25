use serde::{Deserialize, Serialize};

/// What a print comes out as against what was sliced, and the corrections that close the gap.
/// Every field is neutral by default; see `docs/design/compensation.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Compensation {
    /// Whether the shrinkage percentages are applied; off keeps them for later.
    #[serde(default = "on")]
    pub shrinkage_on: bool,
    /// Whether the wall offsets and the parity offset move any layer above the bottom block.
    #[serde(default = "on")]
    pub tolerance_on: bool,
    /// Whether the bottom block's wall offsets and the parity offset move its layers.
    #[serde(default = "on")]
    pub bottom_tolerance_on: bool,

    /// Percentage the geometry is scaled by along the plate's X before slicing. 100 is
    /// no correction; above it prints larger, to come out right after the resin shrinks.
    pub shrink_x_pct: f32,
    pub shrink_y_pct: f32,
    /// Percentage along Z. Normally left at 100: a layer's shrinkage along Z is taken up
    /// by the layer above it, so the error lands in XY.
    pub shrink_z_pct: f32,

    /// Millimetres every hole's wall moves inwards, so a positive value makes holes
    /// smaller and the body larger. A vendor profile calls it `a`.
    pub hole_offset_mm: f32,
    /// Millimetres every outer wall moves outwards, so a positive value makes the body
    /// larger. A vendor profile calls it `b`.
    pub outer_offset_mm: f32,
    /// `hole_offset_mm` for the bottom block, which cures wider under its long exposure.
    pub bottom_hole_offset_mm: f32,
    /// `outer_offset_mm` for the bottom block: negative is what takes an elephant foot off.
    pub bottom_outer_offset_mm: f32,
    /// Millimetres added to the offsets of even layers only. Alternating layers is how an
    /// offset finer than one pixel is reached at all.
    pub parity_offset_mm: f32,

    /// Seconds the machine spends on a layer beyond what the settings account for, added
    /// to every layer of the estimate. Measured, never guessed: see `layer_time_between`.
    pub layer_time_s: f32,
}

impl Default for Compensation {
    fn default() -> Self {
        Self {
            shrinkage_on: true,
            tolerance_on: true,
            bottom_tolerance_on: true,
            shrink_x_pct: 100.0,
            shrink_y_pct: 100.0,
            shrink_z_pct: 100.0,
            hole_offset_mm: 0.0,
            outer_offset_mm: 0.0,
            bottom_hole_offset_mm: 0.0,
            bottom_outer_offset_mm: 0.0,
            parity_offset_mm: 0.0,
            layer_time_s: 0.0,
        }
    }
}

impl Compensation {
    /// The factors the printed geometry is scaled by, X, Y and Z, one each.
    pub fn scale(&self) -> [f32; 3] {
        if !self.shrinkage_on {
            return [1.0; 3];
        }
        [
            factor(self.shrink_x_pct),
            factor(self.shrink_y_pct),
            factor(self.shrink_z_pct),
        ]
    }

    /// Whether the geometry is printed at the size it was sliced at.
    pub fn scales_nothing(&self) -> bool {
        self.scale()
            .iter()
            .all(|factor| (factor - 1.0).abs() < f32::EPSILON)
    }

    /// The scale and translation applying this shrinkage to a part centred on `centre_x_mm`,
    /// `centre_y_mm`: XY about that centre, Z about the plate, so the part does not move.
    pub fn placement(&self, centre_x_mm: f32, centre_y_mm: f32) -> ([f32; 3], [f32; 3]) {
        let scale = self.scale();
        let anchor = [centre_x_mm, centre_y_mm, 0.0];
        let translation = [
            anchor[0] - scale[0] * anchor[0],
            anchor[1] - scale[1] * anchor[1],
            anchor[2] - scale[2] * anchor[2],
        ];
        (scale, translation)
    }

    /// The offsets layer `index` takes: the hole wall and the outer wall, mm.
    /// The parity offset lands on even layers counting from one, so layers at an odd `index`.
    pub fn offsets_of_layer_mm(&self, index: u32, is_bottom: bool) -> [f32; 2] {
        let (on, hole, outer) = if is_bottom {
            (
                self.bottom_tolerance_on,
                self.bottom_hole_offset_mm,
                self.bottom_outer_offset_mm,
            )
        } else {
            (self.tolerance_on, self.hole_offset_mm, self.outer_offset_mm)
        };
        if !on {
            return [0.0; 2];
        }
        let parity = match index % 2 {
            1 => self.parity_offset_mm,
            _ => 0.0,
        };
        [hole + parity, outer + parity]
    }

    /// How much wider, mm a side, a hole-and-part fit prints than drawn, by this resin's
    /// offsets while its tolerance is off; nothing while on, as the slicer moves them (ADR 0313).
    pub fn fit_drift_mm(&self) -> f32 {
        if self.tolerance_on {
            return 0.0;
        }
        self.hole_offset_mm + self.outer_offset_mm
    }

    /// Whether any layer's walls move at all.
    pub fn offsets_nothing(&self) -> bool {
        let still = |offsets: &[f32]| offsets.iter().all(|offset| offset.abs() < f32::EPSILON);
        let parity = self.parity_offset_mm;
        (!self.tolerance_on || still(&[self.hole_offset_mm, self.outer_offset_mm, parity]))
            && (!self.bottom_tolerance_on
                || still(&[
                    self.bottom_hole_offset_mm,
                    self.bottom_outer_offset_mm,
                    parity,
                ]))
    }
}

/// The percentage a part came out at, for a feature of `nominal_mm` measured as `printed_mm`.
/// Printing larger than nominal wants a figure under 100, which shrinks the next slice.
pub fn shrink_pct_between(nominal_mm: f32, printed_mm: f32) -> Option<f32> {
    (printed_mm > 0.0 && nominal_mm > 0.0).then(|| 100.0 * nominal_mm / printed_mm)
}

/// The seconds a layer really costs beyond the estimate, from one print's predicted and
/// actual times over a known layer count.
pub fn layer_time_between(predicted_s: f32, actual_s: f32, layers: u32) -> Option<f32> {
    (layers > 0).then(|| (actual_s - predicted_s) / layers as f32)
}

fn on() -> bool {
    true
}

fn factor(percent: f32) -> f32 {
    if percent > 0.0 { percent / 100.0 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fit_drifts_by_the_offsets_only_while_the_slicer_does_not_apply_them() {
        let measured = Compensation {
            hole_offset_mm: 0.04,
            outer_offset_mm: 0.03,
            ..Compensation::default()
        };
        assert_eq!(measured.fit_drift_mm(), 0.0, "the slicer takes both off");
        let off = Compensation {
            tolerance_on: false,
            ..measured
        };
        assert!(
            (off.fit_drift_mm() - 0.07).abs() < 1e-6,
            "the hole prints 0.04 wider and the part 0.03 thinner"
        );
    }

    #[test]
    fn a_fresh_compensation_changes_nothing() {
        let neutral = Compensation::default();
        assert!(neutral.scales_nothing());
        assert!(neutral.offsets_nothing());
        assert!(
            neutral
                .offsets_of_layer_mm(0, false)
                .iter()
                .all(|offset| *offset == 0.0)
        );
    }

    #[test]
    fn a_part_that_came_out_small_is_printed_larger_next_time() {
        // A 20 mm bar measuring 19.90 mm shrank by half a percent.
        let pct = shrink_pct_between(20.0, 19.9).expect("both measurements are positive");
        assert!((pct - 100.5025).abs() < 1e-3, "got {pct}");
        let scaled = 19.9 * (pct / 100.0);
        assert!(
            (scaled - 20.0).abs() < 1e-3,
            "the correction closes the gap"
        );
    }

    #[test]
    fn a_zero_measurement_has_no_percentage() {
        assert_eq!(shrink_pct_between(20.0, 0.0), None);
    }

    #[test]
    fn the_layer_time_is_the_shortfall_spread_over_the_layers() {
        // 35 m 49 s predicted against 60 m 50 s over 200 layers, as a vendor slicer reads it.
        let seconds = layer_time_between(2149.0, 3650.0, 200).expect("there are layers");
        assert!((seconds - 7.505).abs() < 1e-3, "got {seconds}");
    }

    #[test]
    fn an_empty_stack_has_no_layer_time() {
        assert_eq!(layer_time_between(2149.0, 3650.0, 0), None);
    }

    #[test]
    fn a_placement_holds_the_footprint_centre_and_the_plate() {
        let compensation = Compensation {
            shrink_x_pct: 101.0,
            shrink_z_pct: 102.0,
            ..Compensation::default()
        };
        let (scale, translation) = compensation.placement(30.0, 40.0);
        let moved = |axis: usize, at: f32| scale[axis] * at + translation[axis];
        assert!(
            (moved(0, 30.0) - 30.0).abs() < 1e-4,
            "the centre stands still"
        );
        assert!((moved(1, 40.0) - 40.0).abs() < 1e-4, "on both axes");
        assert!(
            (moved(2, 0.0)).abs() < 1e-6,
            "and the plate stays the plate"
        );
    }

    #[test]
    fn the_parity_offset_lands_on_every_other_layer() {
        let compensation = Compensation {
            outer_offset_mm: -0.02,
            parity_offset_mm: -0.01,
            ..Compensation::default()
        };
        assert!((compensation.offsets_of_layer_mm(0, false)[1] + 0.02).abs() < 1e-6);
        assert!((compensation.offsets_of_layer_mm(1, false)[1] + 0.03).abs() < 1e-6);
    }

    #[test]
    fn the_bottom_block_takes_its_own_offsets() {
        let compensation = Compensation {
            outer_offset_mm: -0.02,
            bottom_outer_offset_mm: -0.08,
            ..Compensation::default()
        };
        assert!((compensation.offsets_of_layer_mm(0, true)[1] + 0.08).abs() < 1e-6);
    }

    #[test]
    fn a_switched_off_group_keeps_its_numbers_and_moves_nothing() {
        let compensation = Compensation {
            shrinkage_on: false,
            bottom_tolerance_on: false,
            shrink_x_pct: 101.0,
            outer_offset_mm: -0.02,
            bottom_outer_offset_mm: -0.08,
            ..Compensation::default()
        };
        assert!(compensation.scales_nothing(), "shrinkage is off");
        assert_eq!(compensation.offsets_of_layer_mm(0, true), [0.0; 2]);
        assert!((compensation.offsets_of_layer_mm(0, false)[1] + 0.02).abs() < 1e-6);
        assert!(
            !compensation.offsets_nothing(),
            "the upper layers still move"
        );
        assert_eq!(compensation.shrink_x_pct, 101.0);
    }

    #[test]
    fn a_compensation_saved_before_the_switches_is_switched_on() {
        let read: Compensation =
            toml::from_str("outer_offset_mm = -0.02").expect("an older table parses");
        assert!(read.shrinkage_on && read.tolerance_on && read.bottom_tolerance_on);
    }
}
