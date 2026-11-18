use core_geometry::Mesh;

/// One of the test prints, by what it tells about a resin and a printer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestPrint {
    /// Spikes, ledges and pillars: the finest detail that prints.
    Detail,
    /// Pins, fins, slots and holes from fine to coarse: whether a layer is under- or
    /// overexposed.
    Exposure,
    /// Faces at each angle: the finish a surface takes.
    Surface,
    /// A hole and pins at each clearance: what fits.
    Fit,
}

impl TestPrint {
    pub const ALL: [Self; 4] = [Self::Detail, Self::Exposure, Self::Surface, Self::Fit];

    /// Its name on a command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Detail => "detail",
            Self::Exposure => "exposure",
            Self::Surface => "surface",
            Self::Fit => "fit",
        }
    }

    /// Its STL's name, the same in every language and safe on every file system.
    pub fn file(self) -> &'static str {
        match self {
            Self::Detail => "detail-test.stl",
            Self::Exposure => "exposure-check.stl",
            Self::Surface => "surface-finish.stl",
            Self::Fit => "fit-gauge.stl",
        }
    }

    /// The model, built afresh, standing on z = 0.
    pub fn build(self) -> Mesh {
        match self {
            Self::Detail => crate::detail::build(),
            Self::Exposure => crate::exposure::build(),
            Self::Surface => crate::surface::build(),
            Self::Fit => crate::fit::build(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fit::{CLEARANCES_MM, HOLE_MM, PIN_PITCH};

    #[test]
    fn every_test_print_stays_small() {
        for print in TestPrint::ALL {
            let faces = print.build().faces.len();
            assert!(faces < 40_000, "{} has {faces} faces", print.name());
        }
    }

    #[test]
    fn the_fit_gauge_pins_differ_from_the_hole_by_their_clearances() {
        let mesh = TestPrint::Fit.build();
        let [low, high] = mesh
            .vertices
            .iter()
            .filter(|v| v.y < -1.0)
            .fold([f32::INFINITY, f32::NEG_INFINITY], |[low, high], v| {
                [low.min(v.x), high.max(v.x)]
            });
        let last = CLEARANCES_MM.len() - 1;
        // The first pin's left edge to the last one's right: the pitches between, both radii.
        let radius = |clearance: f32| f32::midpoint(HOLE_MM, clearance);
        let expected =
            PIN_PITCH * last as f32 + radius(CLEARANCES_MM[0]) + radius(CLEARANCES_MM[last]);
        let span = high - low;
        assert!(
            (span - expected).abs() < 1e-3,
            "{span} mm against {expected} mm"
        );
    }
}
