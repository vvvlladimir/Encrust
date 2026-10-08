use crate::Layer;

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
}

#[cfg(test)]
mod tests {
    use super::*;

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
