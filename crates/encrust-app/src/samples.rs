//! The test prints the start page offers: resin tests of the project's own, built in code
//! with the mark raised on each, and pictured from the same mesh; see `docs/decisions/0232` and `0234`.

pub(crate) mod detail;
mod exposure;
mod fit;
mod mark;
pub(crate) mod solid;
mod surface;

use std::sync::Arc;

use core_geometry::Mesh;

use crate::files::Handed;
use crate::i18n::tr;

/// One test print, opened as if its file had been dropped on the window.
pub struct Sample {
    /// The STL's name, the same in every language and safe on every file system.
    file: &'static str,
    name: fn() -> String,
    about: fn() -> String,
    build: fn() -> Mesh,
}

pub const SAMPLES: [Sample; 4] = [
    Sample {
        file: "detail-test.stl",
        name: || tr!("sample-detail"),
        about: || tr!("sample-detail-about"),
        build: detail::build,
    },
    Sample {
        file: "exposure-check.stl",
        name: || tr!("sample-exposure"),
        about: || tr!("sample-exposure-about"),
        build: exposure::build,
    },
    Sample {
        file: "surface-finish.stl",
        name: || tr!("sample-surface"),
        about: || tr!("sample-surface-about"),
        build: surface::build,
    },
    Sample {
        file: "fit-gauge.stl",
        name: || tr!("sample-fit"),
        about: || tr!("sample-fit-about"),
        build: fit::build,
    },
];

impl Sample {
    /// What the test print is called, in the window's language.
    pub fn name(&self) -> String {
        (self.name)()
    }

    /// What printing it tells, shown when the pointer rests on its name.
    pub fn about(&self) -> String {
        (self.about)()
    }

    /// The model, built afresh.
    pub fn mesh(&self) -> Mesh {
        (self.build)()
    }

    /// The model as an STL handed to the window.
    pub fn handed(&self) -> Handed {
        let bytes = solid::stl(&(self.build)());
        Handed::bytes(self.file, Arc::from(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_test_print_opens_sound() {
        for sample in &SAMPLES {
            let plate = crate::plate::BuildPlate::default();
            let imported = crate::import::prepare(&sample.handed(), &plate, &mut |_| {})
                .expect("a generated STL reads");
            let summary = &imported.summary;
            assert!(summary.is_sound(), "{}: {summary:?}", sample.name());
        }
    }

    #[test]
    fn every_test_print_stays_small() {
        for sample in &SAMPLES {
            let faces = (sample.build)().faces.len();
            assert!(faces < 40_000, "{} has {faces} faces", sample.name());
        }
    }

    #[test]
    fn the_fit_gauge_pins_differ_from_the_hole_by_their_clearances() {
        let mesh = fit::build();
        let [low, high] = mesh
            .vertices
            .iter()
            .filter(|v| v.y < -1.0)
            .fold([f32::INFINITY, f32::NEG_INFINITY], |[low, high], v| {
                [low.min(v.x), high.max(v.x)]
            });
        let last = fit::CLEARANCES_MM.len() - 1;
        // The first pin's left edge to the last one's right: the pitches between, both radii.
        let expected = fit::PIN_PITCH * last as f32
            + (fit::HOLE_MM + fit::CLEARANCES_MM[0]) / 2.0
            + (fit::HOLE_MM + fit::CLEARANCES_MM[last]) / 2.0;
        let span = high - low;
        assert!(
            (span - expected).abs() < 1e-3,
            "{span} mm against {expected} mm"
        );
    }
}
