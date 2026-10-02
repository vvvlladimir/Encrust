use std::num::NonZeroU8;

use core_geometry::Scalar;

/// Geometric parameters of a slicing run. Exposure and lift belong to the printer profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliceSettings {
    /// Layer thickness in millimetres.
    pub layer_height: Scalar,
    /// How many planes to sample inside each layer's band. One is the middle of the band
    /// and nothing else, which loses a feature thinner than a layer on whichever side of
    /// that plane it falls; see `docs/design/slicing.md`.
    pub samples: NonZeroU8,
}

/// One plane a layer: the middle of the band, which is what every layer was until
/// step 16d.
pub const ONE_SAMPLE: NonZeroU8 = NonZeroU8::new(1).expect("one is not zero");

impl Default for SliceSettings {
    fn default() -> Self {
        Self {
            layer_height: 0.05,
            samples: ONE_SAMPLE,
        }
    }
}

impl SliceSettings {
    /// How many layers of `layer_height` it takes to cover `[z_min, z_max]`.
    pub fn layer_count(&self, z_min: Scalar, z_max: Scalar) -> usize {
        if self.layer_height <= 0.0 || z_max <= z_min {
            return 0;
        }
        let quotient = (z_max - z_min) / self.layer_height;
        // A model whose height is a whole number of layers must not gain an almost empty
        // extra one just because the division landed a few ulps above the integer.
        let rounded = quotient.round();
        let count = if (quotient - rounded).abs() < 1e-3 {
            rounded
        } else {
            quotient.ceil()
        };
        count as usize
    }

    /// Z of every sampling plane: the middle of each layer's band of material.
    ///
    /// Sampling the middle rather than an edge is what keeps the top and bottom layers of
    /// a model whose faces sit exactly on a layer boundary; see `docs/design/slicing.md`.
    /// The topmost band is clipped to the mesh, so a partial layer is still sampled
    /// inside the material instead of just above it.
    pub fn plane_heights(&self, z_min: Scalar, z_max: Scalar) -> Vec<Scalar> {
        (0..self.layer_count(z_min, z_max))
            .map(|layer| {
                let bottom = (layer as Scalar).mul_add(self.layer_height, z_min);
                let top = (bottom + self.layer_height).min(z_max);
                Scalar::midpoint(bottom, top)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_millimetres_at_one_millimetre_layers_gives_ten_planes() {
        let settings = SliceSettings {
            layer_height: 1.0,
            ..SliceSettings::default()
        };
        let heights = settings.plane_heights(0.0, 10.0);

        assert_eq!(heights.len(), 10);
        assert!((heights[0] - 0.5).abs() < 1e-6);
        assert!((heights[9] - 9.5).abs() < 1e-6);
    }

    #[test]
    fn a_height_that_is_a_whole_number_of_layers_gains_no_extra_layer() {
        // 0.05 has no exact binary representation, so 10.0 / 0.05 does not land on 200.
        let settings = SliceSettings {
            layer_height: 0.05,
            ..SliceSettings::default()
        };
        assert_eq!(settings.layer_count(0.0, 10.0), 200);
    }

    #[test]
    fn a_partial_top_layer_is_sampled_inside_the_material() {
        let settings = SliceSettings {
            layer_height: 1.0,
            ..SliceSettings::default()
        };
        let heights = settings.plane_heights(0.0, 10.4);

        assert_eq!(heights.len(), 11);
        assert!(
            (heights[10] - 10.2).abs() < 1e-6,
            "the last band is 10.0 to 10.4, so its middle is 10.2"
        );
    }

    #[test]
    fn a_model_thinner_than_one_layer_is_sampled_at_its_own_middle() {
        let settings = SliceSettings {
            layer_height: 1.0,
            ..SliceSettings::default()
        };
        let heights = settings.plane_heights(0.0, 0.3);

        assert_eq!(heights.len(), 1);
        assert!((heights[0] - 0.15).abs() < 1e-6);
    }

    #[test]
    fn planes_start_from_the_bottom_of_the_mesh_wherever_it_sits() {
        let settings = SliceSettings {
            layer_height: 2.0,
            ..SliceSettings::default()
        };
        let heights = settings.plane_heights(-5.0, -1.0);
        assert_eq!(heights.len(), 2);
        assert!((heights[0] - -4.0).abs() < 1e-6);
    }

    #[test]
    fn non_positive_layer_height_yields_no_planes() {
        let settings = SliceSettings {
            layer_height: 0.0,
            ..SliceSettings::default()
        };
        assert!(settings.plane_heights(0.0, 10.0).is_empty());
    }

    #[test]
    fn a_flat_mesh_yields_no_planes() {
        let settings = SliceSettings {
            layer_height: 1.0,
            ..SliceSettings::default()
        };
        assert!(settings.plane_heights(3.0, 3.0).is_empty());
    }
}
