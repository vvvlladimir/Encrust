//! The test prints the start page offers: resin tests of the project's own, built in
//! `core-samples` and pictured from the same mesh; see `docs/decisions/0232` and `0234`.
//! Those too large to bundle are [`remote`] (ADR 0394).

pub mod remote;

use std::sync::Arc;

use core_geometry::Mesh;
use core_samples::TestPrint;

use crate::files::Handed;
use crate::i18n::tr;

/// One test print, opened as if its file had been dropped on the window.
pub struct Sample {
    print: TestPrint,
    name: fn() -> String,
    about: fn() -> String,
}

pub const SAMPLES: [Sample; 4] = [
    Sample {
        print: TestPrint::Detail,
        name: || tr!("sample-detail"),
        about: || tr!("sample-detail-about"),
    },
    Sample {
        print: TestPrint::Exposure,
        name: || tr!("sample-exposure"),
        about: || tr!("sample-exposure-about"),
    },
    Sample {
        print: TestPrint::Surface,
        name: || tr!("sample-surface"),
        about: || tr!("sample-surface-about"),
    },
    Sample {
        print: TestPrint::Fit,
        name: || tr!("sample-fit"),
        about: || tr!("sample-fit-about"),
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
        self.print.build()
    }

    /// The model as an STL handed to the window.
    pub fn handed(&self) -> Handed {
        let bytes = core_samples::stl(&self.print.build());
        Handed::bytes(self.print.file(), Arc::from(bytes))
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
}
