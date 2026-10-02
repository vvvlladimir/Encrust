use crate::LayerRuns;

/// One rasterised layer, and what did not fit on the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Rastered {
    pub runs: LayerRuns,
    /// How far the layer reaches past the edge of the display, pixels. Zero when it fits.
    pub overflow_px: f32,
}

impl Rastered {
    /// True when the whole layer landed on the panel.
    pub fn fits(&self) -> bool {
        self.overflow_px <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> LayerRuns {
        LayerRuns::builder(2, 2).finish()
    }

    #[test]
    fn a_layer_inside_the_panel_fits() {
        let rastered = Rastered {
            runs: blank(),
            overflow_px: 0.0,
        };
        assert!(rastered.fits());
    }

    #[test]
    fn any_overflow_means_it_does_not_fit() {
        let rastered = Rastered {
            runs: blank(),
            overflow_px: 0.5,
        };
        assert!(!rastered.fits());
    }
}
