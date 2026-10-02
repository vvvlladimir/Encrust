use core_geometry::Scalar;

use crate::Contour;

/// Every contour found on one slicing plane.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layer {
    /// Height of the sampling plane, the middle of the layer, millimetres above the
    /// build plate.
    pub z: Scalar,
    pub contours: Vec<Contour>,
    /// Contours of the other planes sampled inside this layer's band, when more than one
    /// was asked for. Only a rasteriser reads them, and it takes their union with
    /// `contours`: a feature thinner than a layer stands on one plane and not the next.
    /// See `docs/design/slicing.md`.
    pub extra: Vec<Vec<Contour>>,
}

impl Layer {
    pub fn new(z: Scalar, contours: Vec<Contour>) -> Self {
        Self {
            z,
            contours,
            extra: Vec::new(),
        }
    }

    pub fn empty(z: Scalar) -> Self {
        Self::new(z, Vec::new())
    }

    /// Whether no plane sampled inside this layer's band found anything.
    pub fn is_empty(&self) -> bool {
        self.contours.is_empty() && self.extra.iter().all(Vec::is_empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_layer_keeps_its_height() {
        let layer = Layer::empty(1.25);
        assert!(layer.is_empty());
        assert!((layer.z - 1.25).abs() < Scalar::EPSILON);
    }
}
