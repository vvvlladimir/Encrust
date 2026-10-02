use serde::{Deserialize, Serialize};

/// What a print comes out as against what was sliced, and the corrections that close the
/// gap: size, tolerance and the estimate's own clock.
///
/// Every field is neutral by default, so a resin that states nothing prints exactly what
/// it did before; see `docs/design/compensation.md`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Compensation {
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

    /// The scale and the translation that apply this shrinkage to a part whose footprint
    /// is centred on `centre_x_mm`, `centre_y_mm`.
    ///
    /// XY is taken about that centre and Z about the plate, so a correction resizes the
    /// part without moving it off where it stands or off the plate it is held to.
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

    /// The offsets layer `index` takes: the hole wall and the outer wall, millimetres.
    ///
    /// The parity offset lands on even layers, counting the first layer off the plate as
    /// layer one, so it is the layers at an odd `index` that take it.
    pub fn offsets_of_layer_mm(&self, index: u32, is_bottom: bool) -> [f32; 2] {
        let (hole, outer) = if is_bottom {
            (self.bottom_hole_offset_mm, self.bottom_outer_offset_mm)
        } else {
            (self.hole_offset_mm, self.outer_offset_mm)
        };
        let parity = match index % 2 {
            1 => self.parity_offset_mm,
            _ => 0.0,
        };
        [hole + parity, outer + parity]
    }

    /// Whether any layer's walls move at all.
    pub fn offsets_nothing(&self) -> bool {
        [
            self.hole_offset_mm,
            self.outer_offset_mm,
            self.bottom_hole_offset_mm,
            self.bottom_outer_offset_mm,
            self.parity_offset_mm,
        ]
        .iter()
        .all(|offset| offset.abs() < f32::EPSILON)
    }
}

/// The percentage a part came out at, for a feature that should have measured
/// `nominal_mm` and measured `printed_mm` instead.
///
/// Printing larger than nominal wants a figure under 100, which shrinks the next slice.
pub fn shrink_pct_between(nominal_mm: f32, printed_mm: f32) -> Option<f32> {
    (printed_mm > 0.0 && nominal_mm > 0.0).then(|| 100.0 * nominal_mm / printed_mm)
}

/// The seconds a layer really costs beyond the estimate, from one print's predicted and
/// actual times over a known layer count.
pub fn layer_time_between(predicted_s: f32, actual_s: f32, layers: u32) -> Option<f32> {
    (layers > 0).then(|| (actual_s - predicted_s) / layers as f32)
}

fn factor(percent: f32) -> f32 {
    if percent > 0.0 { percent / 100.0 } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
